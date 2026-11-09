use super::*;
use std::{collections::HashMap, fs, path::Path};
#[test]
fn invalid_database_remains_untouched() {
    let t = tempfile::tempdir().unwrap();
    fs::write(t.path().join("db.json"), "bad json").unwrap();
    assert!(store::Store::new(t.path().into()).is_err());
    assert_eq!(
        fs::read_to_string(t.path().join("db.json")).unwrap(),
        "bad json"
    );
}
fn fixture() -> (tempfile::TempDir, store::Store, Value) {
    let t = tempfile::tempdir().unwrap();
    let store = store::Store::new(t.path().into()).unwrap();
    let i = json!({"id":"test","name":"My World","version":"1.21.1","loader":"Fabric"});
    store.add(i.clone()).unwrap();
    (t, store, i)
}
#[test]
fn validates_instance_identifiers() {
    for id in ["../bad", "/tmp/bad", "bad/name", ""] {
        assert!(store::validate(&json!({"settings":{},"instances":[{"id":id,"name":"X","version":"1.0","loader":"Vanilla"}]})).is_err());
    }
}
#[test]
fn deleting_instance_preserves_worlds() {
    let (_t, s, _) = fixture();
    let root = s.dir("test").unwrap();
    fs::create_dir_all(root.join("saves/World")).unwrap();
    fs::write(root.join("saves/World/level.dat"), "world").unwrap();
    let r = s.remove("test").unwrap();
    assert!(s.public()["instances"].as_array().unwrap().is_empty());
    assert_eq!(
        fs::read_to_string(Path::new(util::s(&r, "recoveryPath")).join("saves/World/level.dat"))
            .unwrap(),
        "world"
    );
}
#[test]
fn duplicate_excludes_recursive_backups() {
    let (_t, store, _) = fixture();
    let root = store.dir("test").unwrap();
    fs::create_dir_all(root.join("backups/mods/x")).unwrap();
    fs::write(root.join("backups/mods/x/foo"), "skip").unwrap();
    fs::write(root.join("options.txt"), "keep").unwrap();
    let result = store.duplicate("test").unwrap();
    let id = util::s(&result["instance"], "id");
    let copy = store.dir(id).unwrap();
    assert!(!copy.join("backups").exists());
    assert_eq!(
        fs::read_to_string(copy.join("options.txt")).unwrap(),
        "keep"
    );
}
#[test]
fn merge_preserves_backend_loader_version() {
    let (_t, store, _) = fixture();
    let base = store.public();
    store.loader_version("test", "0.16.10").unwrap();
    let mut edit = base.clone();
    edit["settings"]["ram"] = json!(8);
    store.merge(edit, &base).unwrap();
    assert_eq!(store.get("test").unwrap()["loaderVersion"], "0.16.10");
    assert_eq!(store.settings()["ram"], 8);
}
#[test]
fn archive_paths_reject_traversal() {
    let t = tempfile::tempdir().unwrap();
    for name in ["../bad", "/bad", "C:/bad", "a/../../bad", "a\\..\\bad"] {
        assert!(safe(t.path(), name).is_err());
    }
}
#[test]
fn zip_slip_is_rejected() {
    let t = tempfile::tempdir().unwrap();
    let file = t.path().join("bad.zip");
    let mut zip = zip::ZipWriter::new(fs::File::create(&file).unwrap());
    zip.start_file("../bad", zip::write::SimpleFileOptions::default())
        .unwrap();
    use std::io::Write;
    zip.write_all(b"bad").unwrap();
    zip.finish().unwrap();
    assert!(util::unzip(&file, &t.path().join("out"), "").is_err());
    assert!(!t.path().join("bad").exists());
}
#[test]
fn world_snapshot_restore_roundtrip() {
    let t = tempfile::tempdir().unwrap();
    fs::create_dir_all(t.path().join("saves/World")).unwrap();
    let p = t.path().join("saves/World/level.dat");
    fs::write(&p, "old").unwrap();
    let backup = backups::snapshot(t.path(), "worlds", "Test").unwrap();
    fs::write(&p, "new").unwrap();
    backups::restore(t.path(), "worlds", s(&backup, "id")).unwrap();
    assert_eq!(fs::read_to_string(p).unwrap(), "old");
    assert_eq!(
        backups::list_backups(t.path(), "worlds")
            .unwrap()
            .as_array()
            .unwrap()
            .len(),
        2
    );
}
#[test]
fn mods_toggle_remove_restore() {
    let (_t, store, _) = fixture();
    let root = store.dir("test").unwrap();
    fs::create_dir_all(root.join("mods")).unwrap();
    fs::write(root.join("mods/mod.jar"), "mod").unwrap();
    content::toggle(&root, "mod", "mod.jar").unwrap();
    assert!(root.join("mods/mod.jar.disabled").exists());
    let removed = backups::remove_mod(&root, "mod.jar.disabled").unwrap();
    assert!(!root.join("mods/mod.jar.disabled").exists());
    backups::restore_removed(&root, s(&removed, "id")).unwrap();
    assert!(root.join("mods/mod.jar.disabled").exists());
}
#[test]
fn options_preserve_other_resourcepacks() {
    let t = tempfile::tempdir().unwrap();
    fs::write(
        t.path().join("options.txt"),
        "volume:1\nresourcePacks:[\"vanilla\",\"file/other.zip\"]",
    )
    .unwrap();
    content::resource(t.path(), "new.zip", true).unwrap();
    content::resource(t.path(), "new.zip", false).unwrap();
    let out = fs::read_to_string(t.path().join("options.txt")).unwrap();
    assert!(out.contains("file/other.zip"));
    assert!(out.contains("volume:1"));
    assert!(!out.contains("file/new.zip"));
}
#[test]
fn shaders_can_be_disabled() {
    let t = tempfile::tempdir().unwrap();
    content::shader(t.path(), "shaders.zip", true).unwrap();
    content::shader(t.path(), "shaders.zip", false).unwrap();
    assert!(fs::read_to_string(t.path().join("config/iris.properties"))
        .unwrap()
        .contains("enableShaders=false"));
}
#[test]
fn manifest_rules_select_os_and_architecture() {
    let rule = json!({"rules":[{"action":"allow","os":{"name":"osx"}},{"action":"disallow","os":{"arch":"x86"}}]});
    assert!(minecraft::allowed(&rule, "osx", "aarch64"));
    assert!(!minecraft::allowed(&rule, "windows", "aarch64"));
    assert!(!minecraft::allowed(&rule, "osx", "x86"));
}
#[test]
fn feature_rules_are_not_enabled_without_feature() {
    let r = json!({"rules":[{"action":"allow","features":{"is_demo_user":true}}]});
    assert!(!minecraft::allowed(&r, "osx", "aarch64"));
}
#[test]
fn maven_coordinates_are_safe() {
    assert_eq!(
        minecraft::maven("org.lwjgl:lwjgl:3.3.3:natives-macos-arm64")
            .unwrap()
            .to_string_lossy()
            .replace('\\', "/"),
        "org/lwjgl/lwjgl/3.3.3/lwjgl-3.3.3-natives-macos-arm64.jar"
    );
    assert!(minecraft::maven("a:../../bad:1").is_err());
}
#[test]
fn jvm_args_do_not_invoke_shell() {
    assert_eq!(
        minecraft::split_args("'-Dname=hello world' -Xmx4G '$(touch /tmp/bad)'").unwrap(),
        vec!["-Dname=hello world", "-Xmx4G", "$(touch /tmp/bad)"]
    );
    assert!(minecraft::split_args("\"bad").is_err());
}
#[test]
fn merging_loader_preserves_jvm_args() {
    let parent = json!({"libraries":[{"name":"a:b:1"}],"arguments":{"jvm":["-cp","${classpath}"],"game":["--username","${name}"]}});
    let child = json!({"libraries":[{"name":"a:b:2"}],"arguments":{"game":["--loader"]}});
    let merged = minecraft::merge(&parent, &child);
    assert_eq!(list(&merged, "libraries").len(), 1);
    assert_eq!(list(&merged["arguments"], "jvm").len(), 2);
    assert_eq!(list(&merged["arguments"], "game").len(), 3);
}
#[test]
fn argument_placeholders_preserve_spaces() {
    let vars = HashMap::from([("game_directory".into(), "/My Folder/Game".into())]);
    assert_eq!(
        minecraft::arguments(
            &json!(["--gameDir", "${game_directory}"])
                .as_array()
                .unwrap(),
            &vars,
            "osx",
            "aarch64"
        ),
        vec!["--gameDir", "/My Folder/Game"]
    );
}
#[test]
fn successful_opengl_log_is_not_a_graphics_error() {
    assert_eq!(
        report("Using graphics backend OpenGL", 143, "x")["title"],
        "Причина не определена"
    );
    assert_eq!(report("GLFW error", 1, "x")["title"], "Ошибка графики");
}
#[test]
fn import_preserves_source_files() {
    let (_t, source, _) = fixture();
    fs::write(source.dir("test").unwrap().join("options.txt"), "keep").unwrap();
    let other = tempfile::tempdir().unwrap();
    let target = store::Store::new(other.path().into()).unwrap();
    assert_eq!(target.import(&source.root).unwrap(), 1);
    let first = target.public()["instances"][0].clone();
    assert_eq!(
        fs::read_to_string(target.dir(s(&first, "id")).unwrap().join("options.txt")).unwrap(),
        "keep"
    );
    assert!(source.dir("test").unwrap().join("options.txt").exists());
}
#[test]
fn pack_roundtrip_preserves_local_overrides() {
    let (_t, store, mut inst) = fixture();
    inst["loader"] = json!("Vanilla");
    store
        .edit(|db| {
            db["instances"][0] = inst.clone();
            Ok(())
        })
        .unwrap();
    let root = store.dir("test").unwrap();
    fs::create_dir_all(root.join("config")).unwrap();
    fs::write(root.join("config/test.txt"), "setting").unwrap();
    let target = store.root.join("out.mrpack");
    content::export_pack(&store, &inst, &target).unwrap();
    let imported =
        content::import_pack(&store, &net::Net::new().unwrap(), &target, &|_| {}).unwrap();
    assert_eq!(
        fs::read_to_string(
            store
                .dir(s(&imported, "id"))
                .unwrap()
                .join("config/test.txt")
        )
        .unwrap(),
        "setting"
    );
}
#[test]
fn horizon_always_pins_engine() {
    assert_eq!(
        horizon::command("play").unwrap(),
        vec!["play", "--version", "v0.2.97"]
    );
    assert!(horizon::command("delete").is_err());
}
#[test]
fn version_ranges_match_electron_behavior() {
    use compatibility::matches;
    for (version, range, expected) in [
        ("1.21.1", ">=1.20 <1.22", Some(true)),
        ("1.19", "1.20.x", Some(false)),
        ("0.16.10", "^0.16.0", Some(true)),
        ("0.17.0", "^0.16.0", Some(false)),
        ("1.3", "~1.2", Some(false)),
        ("1.2.8", "~1.2", Some(true)),
        ("1.21.1-pre1", ">1.20", None),
        ("1.21.1", "1.20.x || 1.21.x", Some(true)),
    ] {
        assert_eq!(
            matches(version, &json!(range)),
            expected,
            "{version} {range}"
        );
    }
}
#[test]
fn local_preflight_detects_wrong_minecraft_version() {
    let (_t, store, inst) = fixture();
    let root = store.dir("test").unwrap();
    fs::create_dir_all(root.join("mods")).unwrap();
    let mut zip = zip::ZipWriter::new(fs::File::create(root.join("mods/test.jar")).unwrap());
    zip.start_file("fabric.mod.json", zip::write::SimpleFileOptions::default())
        .unwrap();
    use std::io::Write;
    zip.write_all(
        json!({"id":"testmod","version":"1.0","depends":{"minecraft":"1.20.x"}})
            .to_string()
            .as_bytes(),
    )
    .unwrap();
    zip.finish().unwrap();
    let r = compatibility::check(&root, &net::Net::new().unwrap(), &inst, false).unwrap();
    assert_eq!(r["errors"], 1);
}
#[test]
fn bundled_fabric_libraries_are_not_top_level_duplicates() {
    use std::io::{Cursor,Write};
    fn jar(id:&str,nested:Option<&[u8]>)->Vec<u8>{
        let mut zip=zip::ZipWriter::new(Cursor::new(Vec::new()));
        let meta=if nested.is_some(){json!({"id":id,"version":"1.0","jars":[{"file":"nested.jar"}]})}else{json!({"id":id,"version":"1.0"})};
        zip.start_file("fabric.mod.json",zip::write::SimpleFileOptions::default()).unwrap();zip.write_all(meta.to_string().as_bytes()).unwrap();
        if let Some(bytes)=nested{zip.start_file("nested.jar",zip::write::SimpleFileOptions::default()).unwrap();zip.write_all(bytes).unwrap();}
        zip.finish().unwrap().into_inner()
    }
    let (_t,store,inst)=fixture();let root=store.dir("test").unwrap();fs::create_dir_all(root.join("mods")).unwrap();
    let library=jar("library",None);
    fs::write(root.join("mods/one.jar"),jar("one",Some(&library))).unwrap();
    fs::write(root.join("mods/two.jar"),jar("two",Some(&library))).unwrap();
    let net=net::Net::new().unwrap();
    assert_eq!(compatibility::check(&root,&net,&inst,false).unwrap()["errors"],0);
    fs::write(root.join("mods/duplicate.jar"),jar("one",None)).unwrap();
    assert_eq!(compatibility::check(&root,&net,&inst,false).unwrap()["errors"],1);
}
#[test]
fn legacy_world_backup_can_be_restored() {
    let t = tempfile::tempdir().unwrap();
    let root = t.path();
    let old = root.join("backups/worlds/1234-abcd");
    fs::create_dir_all(old.join("saves/World")).unwrap();
    fs::write(old.join("saves/World/level.dat"), "old-world").unwrap();
    write_json(
        &old.join("manifest.json"),
        &json!({"id":"1234-abcd","at":"2026-01-01T00:00:00Z","worlds":["World"],"reason":"Legacy"}),
    )
    .unwrap();
    assert_eq!(
        backups::list_backups(root, "worlds")
            .unwrap()
            .as_array()
            .unwrap()
            .len(),
        1
    );
    backups::restore(root, "worlds", "1234-abcd").unwrap();
    assert_eq!(
        fs::read_to_string(root.join("saves/World/level.dat")).unwrap(),
        "old-world"
    );
}
#[test]
fn legacy_removed_mod_gets_its_original_id() {
    let t = tempfile::tempdir().unwrap();
    let old = t.path().join("backups/removed-mods/legacy");
    fs::create_dir_all(&old).unwrap();
    fs::write(old.join("old.jar"), "mod").unwrap();
    write_json(
        &old.join("manifest.json"),
        &json!({"name":"old.jar","at":"2026-01-01T00:00:00Z"}),
    )
    .unwrap();
    let listed = backups::removed(t.path()).unwrap();
    assert_eq!(listed[0]["id"], "legacy");
    backups::restore_removed(t.path(), "legacy").unwrap();
    assert!(t.path().join("mods/old.jar").exists());
}
#[test]
fn resourcepack_toggle_changes_options_without_renaming_zip() {
    let t = tempfile::tempdir().unwrap();
    fs::create_dir_all(t.path().join("resourcepacks")).unwrap();
    fs::write(t.path().join("resourcepacks/pack.zip"), "zip").unwrap();
    content::toggle(t.path(), "resourcepack", "pack.zip").unwrap();
    assert!(content::active(t.path(), "resourcepack", "pack.zip"));
    content::toggle(t.path(), "resourcepack", "pack.zip").unwrap();
    assert!(!content::active(t.path(), "resourcepack", "pack.zip"));
    assert!(t.path().join("resourcepacks/pack.zip").exists());
}
#[test]
fn oauth_only_accepts_exact_redirect_and_matching_state() {
    let valid =
        url::Url::parse("https://login.live.com/oauth20_desktop.srf?state=test-state&code=abc")
            .unwrap();
    assert_eq!(
        auth::redirect_code(&valid, "test-state").unwrap().unwrap(),
        "abc"
    );
    assert!(auth::redirect_code(&valid, "different").unwrap().is_err());
    for url in [
        "https://login.live.com/oauth20_desktop.srf.evil?code=abc&state=test-state",
        "https://evil.example/oauth20_desktop.srf?code=abc&state=test-state",
    ] {
        assert!(auth::redirect_code(&url::Url::parse(url).unwrap(), "test-state").is_none());
    }
    let denied = url::Url::parse(
        "https://login.live.com/oauth20_desktop.srf?state=test-state&error=access_denied",
    )
    .unwrap();
    assert_eq!(
        auth::redirect_code(&denied, "test-state")
            .unwrap()
            .unwrap_err()
            .to_string(),
        "Вход отменён"
    );
}
#[test]
fn changing_loader_does_not_reuse_old_loader_version() {
    let (_t, store, _) = fixture();
    store.loader_version("test", "old-fabric-version").unwrap();
    let base = store.public();
    let mut next = base.clone();
    next["instances"][0]["loader"] = json!("Quilt");
    next["instances"][0]
        .as_object_mut()
        .unwrap()
        .remove("loaderVersion");
    store.merge(next, &base).unwrap();
    assert!(store.get("test").unwrap()["loaderVersion"].is_null());
}
