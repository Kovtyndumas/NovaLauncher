use crate::{backups, minecraft, net::Net, store::Store, util::*};
use anyhow::{bail, Context, Result};
use serde_json::{json, Value};
use std::{
    collections::BTreeMap,
    fs,
    path::Path,
    time::{Duration, SystemTime},
};

const CONTENT: &[&str] = &[
    "mods",
    "config",
    "resourcepacks",
    "shaderpacks",
    "options.txt",
    "optionsshaders.txt",
    ".nova-content.json",
];
fn walk(root: &Path, path: &Path, out: &mut Vec<(String, u64)>) -> Result<()> {
    if !path.exists() {
        return Ok(());
    }
    if path.is_symlink() {
        return Ok(());
    }
    let meta = fs::symlink_metadata(path)?;
    if meta.is_file() {
        out.push((
            path.strip_prefix(root)?
                .to_string_lossy()
                .replace('\\', "/"),
            meta.len(),
        ));
    } else if meta.is_dir() {
        for entry in fs::read_dir(path)? {
            walk(root, &entry?.path(), out)?;
        }
    }
    Ok(())
}
fn size(path: &Path) -> Result<u64> {
    let mut files = vec![];
    walk(path, path, &mut files)?;
    Ok(files.iter().map(|v| v.1).sum())
}
pub fn inventory(root: &Path) -> Result<BTreeMap<String, String>> {
    let mut files = vec![];
    for name in CONTENT {
        walk(root, &safe(root, name)?, &mut files)?;
    }
    files
        .into_iter()
        .map(|(name, _)| Ok((name.clone(), sha(&safe(root, &name)?, "sha256")?)))
        .collect()
}
pub fn signature(store: &Store, inst: &Value) -> Result<Value> {
    Ok(
        json!({"files":inventory(&store.dir(s(inst,"id"))?)?,"version":inst["version"],"loader":inst["loader"],"loaderVersion":inst["loaderVersion"],"launchSettings":inst["launchSettings"]}),
    )
}
fn delta(before: &BTreeMap<String, String>, after: &BTreeMap<String, String>) -> Vec<Value> {
    let mut changes = vec![];
    for (name, hash) in after {
        if before.get(name) != Some(hash) {
            changes.push(
                json!({"name":name,"kind":if before.contains_key(name){"changed"}else{"added"}}),
            );
        }
    }
    for name in before.keys() {
        if !after.contains_key(name) {
            changes.push(json!({"name":name,"kind":"removed"}));
        }
    }
    changes
}
pub fn record(
    root: &Path,
    label: &str,
    before: &BTreeMap<String, String>,
    error: Option<&str>,
) -> Result<()> {
    let after = inventory(root)?;
    let changes = delta(before, &after);
    let path = root.join(".nova-changes.json");
    let mut history = read_json(&path).unwrap_or(json!([]));
    let entries = history
        .as_array_mut()
        .context("Повреждена история изменений")?;
    entries.insert(
        0,
        json!({"at":now(),"label":label,"changes":changes,"error":error}),
    );
    entries.truncate(300);
    write_json(&path, &history)?;
    write_json(&root.join(".nova-observed.json"), &json!(after))?;
    Ok(())
}
pub fn history(store: &Store, id: &str) -> Result<Value> {
    let root = store.dir(id)?;
    let observed = root.join(".nova-observed.json");
    if let Ok(value) = read_json(&observed) {
        if let Ok(before) = serde_json::from_value::<BTreeMap<String, String>>(value) {
            if before != inventory(&root)? {
                record(&root, "Изменения вне Nova", &before, None)?;
            }
        }
    } else {
        write_json(&observed, &json!(inventory(&root)?))?;
    }
    Ok(read_json(&root.join(".nova-changes.json")).unwrap_or(json!([])))
}
pub fn create_trial(store: &Store, id: &str) -> Result<Value> {
    let source = store.get(id)?;
    if s(&source, "loader") == "Rust" {
        bail!("Пробные обновления доступны для Java-сборок");
    }
    let baseline = signature(store, &source)?;
    let copy = store.duplicate(id)?;
    let trial_id = s(&copy["instance"], "id");
    let root = store.dir(trial_id)?;
    if baseline != signature(store, &source)? || baseline["files"] != json!(inventory(&root)?) {
        let _ = store.remove(trial_id);
        bail!("Исходная сборка изменилась во время копирования. Создайте пробную копию ещё раз.");
    }
    store.edit(|db|{let trial=db["instances"].as_array_mut().unwrap().iter_mut().find(|v|s(v,"id")==trial_id).unwrap();trial["name"]=json!(format!("{} · пробная",s(&source,"name")));trial["trial"]=json!({"source":id,"sourceName":source["name"],"base":baseline,"createdAt":now(),"confirmed":null,"promoted":false});Ok(())})?;
    Ok(json!({"db":store.public(),"instance":store.get(trial_id)?}))
}
pub fn confirm_trial(store: &Store, id: &str) -> Result<Value> {
    let trial = store.get(id)?;
    if !trial["trial"].is_object() || flag(&trial["trial"], "promoted", false) {
        bail!("Это не активная пробная копия");
    }
    let history = read_json(&store.root.join("play-history.json")).unwrap_or(json!([]));
    let current = signature(store, &trial)?;
    let played = history
        .as_array()
        .into_iter()
        .flatten()
        .find(|v| s(v, "instanceId") == id)
        .is_some_and(|v| {
            v["code"].as_i64() == Some(0)
                && s(v, "at") >= s(&trial["trial"], "createdAt")
                && v["verifiedContent"] == current
        });
    if !played {
        bail!("Сначала запустите пробную сборку, проверьте её в игре и завершите сеанс без ошибки");
    }
    let confirmed = signature(store, &trial)?;
    store.edit(|db| {
        let inst = db["instances"]
            .as_array_mut()
            .unwrap()
            .iter_mut()
            .find(|v| s(v, "id") == id)
            .unwrap();
        inst["trial"]["confirmed"] = confirmed;
        Ok(())
    })?;
    Ok(store.public())
}
fn remove_path(path: &Path) -> Result<()> {
    if path.is_symlink() || path.is_file() {
        fs::remove_file(path)?;
    } else if path.is_dir() {
        fs::remove_dir_all(path)?;
    }
    Ok(())
}
// A pending journal restores the old content after an interrupted directory swap.
pub fn recover(root: &Path) -> Result<()> {
    let transaction = safe(root, ".nova-promotion")?;
    let journal = transaction.join("pending.json");
    if !journal.is_file() {
        return Ok(());
    }
    let data = read_json(&journal)?;
    if !flag(&data, "committed", false) {
        for key in CONTENT {
            let target = safe(root, key)?;
            let old = safe(&transaction.join("old"), key)?;
            if old.exists() {
                remove_path(&target)?;
                fs::rename(old, target)?;
            } else if data["existed"][*key] == false {
                remove_path(&target)?;
            }
        }
    }
    fs::remove_dir_all(transaction)?;
    Ok(())
}
pub fn promote_trial(store: &Store, id: &str) -> Result<Value> {
    let trial = store.get(id)?;
    let meta = &trial["trial"];
    if !meta.is_object() || flag(meta, "promoted", false) {
        bail!("Это не активная пробная копия");
    }
    let source = store.get(s(meta, "source"))?;
    if meta["confirmed"].is_null() || meta["confirmed"] != signature(store, &trial)? {
        bail!("Проверьте текущее состояние пробной сборки в игре и подтвердите его");
    }
    if meta["base"] != signature(store, &source)? {
        bail!("Основная сборка изменилась после создания копии. Автоматический перенос остановлен; создайте новую пробную копию.");
    }
    if trial["version"] != source["version"]
        || trial["loader"] != source["loader"]
        || trial["launchSettings"] != source["launchSettings"]
    {
        bail!("Перенос поддерживает контент той же версии и профиля запуска. Параметры основной сборки сохраняются.");
    }
    let from = store.dir(id)?;
    let root = store.dir(s(&source, "id"))?;
    recover(&root)?;
    let before = inventory(&root)?;
    backups::snapshot(&root, "mods", "Перед переносом пробного обновления")?;
    let txn = safe(&root, ".nova-promotion")?;
    if txn.exists() {
        fs::remove_dir_all(&txn)?;
    }
    fs::create_dir_all(txn.join("old"))?;
    let mut existed = json!({});
    for key in CONTENT {
        existed[*key] = json!(safe(&root, key)?.exists());
        copy_tree(&safe(&from, key)?, &safe(&txn.join("new"), key)?, &[])?;
    }
    // Compare again after staging: external file editors do not share Nova's operation lock.
    if meta["base"] != signature(store, &source)? || meta["confirmed"] != signature(store, &trial)?
    {
        fs::remove_dir_all(txn)?;
        bail!("Файлы изменились во время подготовки переноса. Повторите проверку.");
    }
    write_json(
        &txn.join("pending.json"),
        &json!({"existed":existed,"committed":false}),
    )?;
    let result = (|| {
        for key in CONTENT {
            let target = safe(&root, key)?;
            if target.exists() {
                fs::rename(&target, safe(&txn.join("old"), key)?)?;
            }
            let staged = safe(&txn.join("new"), key)?;
            if staged.exists() {
                fs::rename(staged, target)?;
            }
        }
        write_json(
            &txn.join("pending.json"),
            &json!({"existed":existed,"committed":true}),
        )?;
        Ok(())
    })();
    if let Err(error) = result {
        recover(&root)?;
        return Err(error);
    }
    recover(&root)?;
    record(&root, "Перенос проверенного обновления", &before, None)?;
    store.edit(|db| {
        if let Some(inst) = db["instances"]
            .as_array_mut()
            .unwrap()
            .iter_mut()
            .find(|v| s(v, "id") == id)
        {
            inst["trial"]["promoted"] = json!(true);
        }
        Ok(())
    })?;
    Ok(json!({"db":store.public(),"source":source["id"],"worldsPreserved":true}))
}
pub fn trash(store: &Store) -> Result<Value> {
    let dir = store.root.join("deleted-instances");
    let mut result = vec![];
    if dir.is_dir() {
        for entry in fs::read_dir(&dir)? {
            let path = entry?.path();
            if !path.is_dir() || path.is_symlink() {
                continue;
            }
            let meta = read_json(&path.join(".nova-deleted.json")).unwrap_or(Value::Null);
            result.push(json!({"key":path.file_name().unwrap().to_string_lossy(),"instance":meta["instance"],"at":meta["at"],"bytes":size(&path)?,"restorable":meta["instance"].is_object()}));
        }
    }
    result.sort_by(|a, b| s(b, "at").cmp(s(a, "at")));
    Ok(json!(result))
}
pub fn restore_trash(store: &Store, key: &str) -> Result<Value> {
    leaf(key)?;
    let source = safe(&store.root.join("deleted-instances"), key)?;
    let meta = read_json(&safe(&source, ".nova-deleted.json")?)?;
    let mut inst = meta["instance"].clone();
    crate::store::validate(&json!({"instances":[inst.clone()],"settings":{}}))?;
    let mut id = s(&inst, "id").to_owned();
    if store.get(&id).is_ok() || store.root.join("instances").join(&id).exists() {
        id = uuid::Uuid::new_v4().to_string();
        inst["id"] = json!(id);
        inst.as_object_mut().unwrap().remove("trial");
    }
    let destination = safe(&store.root.join("instances"), &id)?;
    fs::create_dir_all(destination.parent().unwrap())?;
    fs::rename(&source, &destination)?;
    if let Err(error) = store.add(inst.clone()) {
        let _ = fs::rename(&destination, &source);
        return Err(error);
    }
    let _ = fs::remove_file(destination.join(".nova-deleted.json"));
    Ok(json!({"db":store.public(),"instance":inst}))
}
pub fn storage(store: &Store) -> Result<Value> {
    let mut instances = vec![];
    for inst in list(&store.db(), "instances") {
        let dir = store.dir(s(inst, "id"))?;
        let total = size(&dir)?;
        instances.push(json!({"id":inst["id"],"name":inst["name"],"bytes":total,"worlds":size(&dir.join("saves"))?,"backups":size(&dir.join("backups"))?,"mods":size(&dir.join("mods"))?}));
    }
    instances.sort_by(|a, b| b["bytes"].as_u64().cmp(&a["bytes"].as_u64()));
    let shared = size(&store.root.join("minecraft"))? + size(&store.root.join("runtimes"))?;
    let trash = size(&store.root.join("deleted-instances"))?;
    Ok(
        json!({"instances":instances,"shared":shared,"trash":trash,"downloads":size(&store.root.join("downloads"))?+size(&store.root.join("updates"))?}),
    )
}
pub fn cleanup_preview(store: &Store) -> Result<Value> {
    let root = store.root.join("downloads");
    let mut files = vec![];
    walk(&root, &root, &mut files)?;
    let mut candidates = vec![];
    for (name, bytes) in files {
        if !name.ends_with(".part") {
            continue;
        }
        let path = safe(&root, &name)?;
        let metadata = fs::metadata(&path)?;
        if metadata.modified()?.elapsed().unwrap_or_default() < Duration::from_secs(86400) {
            continue;
        }
        let hash = sha(&path, "sha256")?;
        candidates.push(json!({"id":format!("{name}:{hash}"),"name":name,"bytes":bytes}));
    }
    Ok(json!(candidates))
}
pub fn cleanup(store: &Store, ids: &Value) -> Result<Value> {
    let selected = ids.as_array().context("Выберите файлы из предпросмотра")?;
    let fresh = cleanup_preview(store)?;
    let mut bytes = 0;
    let mut count = 0;
    // Resolve all selections from a fresh allowlist before deleting anything.
    let mut paths = vec![];
    let mut unique = std::collections::HashSet::new();
    for id in selected {
        if !unique.insert(id.to_string()) {
            bail!("Файл выбран повторно");
        }
        let item = fresh
            .as_array()
            .unwrap()
            .iter()
            .find(|v| v["id"] == *id)
            .context("Список устарел. Обновите предпросмотр очистки.")?;
        paths.push((
            safe(&store.root.join("downloads"), s(item, "name"))?,
            number(item, "bytes", 0),
        ));
    }
    for (path, len) in paths {
        fs::remove_file(path)?;
        bytes += len;
        count += 1;
    }
    Ok(json!({"bytes":bytes,"count":count}))
}
pub fn compare(store: &Store, left: &str, right: &str) -> Result<Value> {
    let a = store.get(left)?;
    let b = store.get(right)?;
    Ok(
        json!({"left":a,"right":b,"changes":delta(&inventory(&store.dir(left)?)?,&inventory(&store.dir(right)?)?)}),
    )
}
pub fn offline_ready(store: &Store, id: &str) -> Result<Value> {
    let inst = store.get(id)?;
    if s(&inst, "loader") == "Rust" {
        return Ok(
            json!({"ready":false,"message":"Horizon использует собственную подготовку файлов и вход. Эта проверка предназначена для Java-сборок."}),
        );
    }
    let net = Net::new()?;
    net.1.configure(&json!({"offlineMode":true}));
    match minecraft::prepare(store, &net, &inst, &|_| {}) {
        Ok(_) => Ok(
            json!({"ready":true,"message":"Java, загрузчик, библиотеки и ресурсы доступны локально. Онлайн-серверы требуют подключения."}),
        ),
        Err(error) => Ok(json!({"ready":false,"message":error.to_string()})),
    }
}
pub fn screenshots(store: &Store, id: &str, offset: usize) -> Result<Value> {
    use base64::Engine;
    let root = store.dir(id)?.join("screenshots");
    let mut files = vec![];
    if root.is_dir() {
        for entry in fs::read_dir(&root)? {
            let path = entry?.path();
            if path.is_symlink() || !path.is_file() {
                continue;
            }
            let ext = path
                .extension()
                .and_then(|v| v.to_str())
                .unwrap_or("")
                .to_lowercase();
            if !["png", "jpg", "jpeg"].contains(&ext.as_str()) {
                continue;
            }
            let metadata = fs::metadata(&path)?;
            files.push((
                path,
                metadata.modified().unwrap_or(SystemTime::UNIX_EPOCH),
                metadata.len(),
                ext,
            ));
        }
    }
    files.sort_by(|a, b| b.1.cmp(&a.1));
    let total = files.len();
    let items:Vec<Value>=files.into_iter().skip(offset).take(12).map(|(path,at,bytes,ext)|{
        let mime=if ext=="png"{"image/png"}else{"image/jpeg"};
        let image=if bytes<=6*1024*1024{fs::read(&path).ok().map(|data|format!("data:{mime};base64,{}",base64::engine::general_purpose::STANDARD.encode(data)))}else{None};
        json!({"name":path.file_name().unwrap().to_string_lossy(),"at":chrono::DateTime::<chrono::Utc>::from(at).to_rfc3339(),"bytes":bytes,"image":image})
    }).collect();
    Ok(json!({"total":total,"offset":offset,"items":items}))
}
pub fn open_screenshot(store: &Store, id: &str, name: &str) -> Result<Value> {
    leaf(name)?;
    let ext = Path::new(name)
        .extension()
        .and_then(|v| v.to_str())
        .unwrap_or("")
        .to_lowercase();
    if !["png", "jpg", "jpeg"].contains(&ext.as_str()) {
        bail!("Можно открыть только скриншот PNG или JPEG");
    }
    let path = safe(&store.dir(id)?.join("screenshots"), name)?;
    if !path.is_file() {
        bail!("Скриншот не найден");
    }
    open::that(path)?;
    Ok(Value::Null)
}

#[cfg(test)]
mod tests {
    use super::*;
    fn fixture() -> (tempfile::TempDir, Store) {
        let temp = tempfile::tempdir().unwrap();
        let store = Store::new(temp.path().into()).unwrap();
        store.add(json!({"id":"main","name":"My World","version":"1.21.1","loader":"Fabric","launchSettings":{"ram":4}})).unwrap();
        let root = store.dir("main").unwrap();
        fs::create_dir_all(root.join("mods")).unwrap();
        fs::write(root.join("mods/old.jar"), b"old").unwrap();
        fs::create_dir_all(root.join("saves/World")).unwrap();
        fs::write(root.join("saves/World/level.dat"), b"real world").unwrap();
        (temp, store)
    }
    fn played(store: &Store, id: &str) {
        let inst = store.get(id).unwrap();
        write_json(&store.root.join("play-history.json"),&json!([{"instanceId":id,"code":0,"at":now(),"verifiedContent":signature(store,&inst).unwrap()}])).unwrap();
    }
    #[test]
    fn trial_promotion_keeps_real_worlds_and_source_profile_and_saves_backup() {
        let (_temp, store) = fixture();
        let result = create_trial(&store, "main").unwrap();
        let id = s(&result["instance"], "id");
        let trial = store.dir(id).unwrap();
        fs::remove_file(trial.join("mods/old.jar")).unwrap();
        fs::write(trial.join("mods/new.jar"), b"new").unwrap();
        fs::write(trial.join("saves/World/level.dat"), b"test world").unwrap();
        assert!(promote_trial(&store, id).is_err());
        assert!(confirm_trial(&store, id).is_err());
        played(&store, id);
        confirm_trial(&store, id).unwrap();
        let result = promote_trial(&store, id).unwrap();
        assert_eq!(result["worldsPreserved"], true);
        let root = store.dir("main").unwrap();
        assert_eq!(
            fs::read(root.join("saves/World/level.dat")).unwrap(),
            b"real world"
        );
        assert_eq!(
            fs::read(trial.join("saves/World/level.dat")).unwrap(),
            b"test world"
        );
        assert!(!root.join("mods/old.jar").exists());
        assert_eq!(fs::read(root.join("mods/new.jar")).unwrap(), b"new");
        assert_eq!(store.get("main").unwrap()["launchSettings"]["ram"], 4);
        assert!(!backups::list_backups(&root, "mods")
            .unwrap()
            .as_array()
            .unwrap()
            .is_empty());
        assert!(promote_trial(&store, id).is_err());
    }
    #[test]
    fn modified_source_or_unplayed_trial_cannot_be_promoted() {
        let (_temp, store) = fixture();
        let result = create_trial(&store, "main").unwrap();
        let id = s(&result["instance"], "id");
        played(&store, id);
        fs::write(
            store.dir(id).unwrap().join("mods/new.jar"),
            b"unplayed update",
        )
        .unwrap();
        assert!(confirm_trial(&store, id).is_err());
        played(&store, id);
        confirm_trial(&store, id).unwrap();
        fs::write(
            store.dir("main").unwrap().join("mods/old.jar"),
            b"external source edit",
        )
        .unwrap();
        assert!(promote_trial(&store, id)
            .unwrap_err()
            .to_string()
            .contains("Основная сборка изменилась"));
        assert_eq!(
            fs::read(store.dir("main").unwrap().join("mods/old.jar")).unwrap(),
            b"external source edit"
        );
    }
    #[test]
    fn interrupted_swap_is_rolled_back_when_store_reopens() {
        let (temp, store) = fixture();
        let root = store.dir("main").unwrap();
        let txn = root.join(".nova-promotion");
        fs::create_dir_all(txn.join("old")).unwrap();
        fs::rename(root.join("mods"), txn.join("old/mods")).unwrap();
        fs::create_dir_all(root.join("mods")).unwrap();
        fs::write(root.join("mods/new.jar"), b"interrupted").unwrap();
        fs::create_dir_all(root.join("config")).unwrap();
        fs::write(root.join("config/new.cfg"), b"new config").unwrap();
        let mut existed = json!({});
        for key in CONTENT {
            existed[*key] = json!(*key == "mods");
        }
        write_json(
            &txn.join("pending.json"),
            &json!({"existed":existed,"committed":false}),
        )
        .unwrap();
        drop(store);
        Store::new(temp.path().into()).unwrap();
        assert_eq!(fs::read(root.join("mods/old.jar")).unwrap(), b"old");
        assert!(!root.join("mods/new.jar").exists());
        assert!(!root.join("config").exists());
        assert!(!txn.exists());
    }
    #[test]
    fn trash_roundtrip_restores_metadata_and_never_overwrites_an_existing_instance() {
        let (_temp, store) = fixture();
        store.remove("main").unwrap();
        let items = trash(&store).unwrap();
        assert_eq!(items[0]["instance"]["name"], "My World");
        let key = s(&items[0], "key");
        store
            .add(json!({"id":"main","name":"New main","loader":"Vanilla","version":"1.21.1"}))
            .unwrap();
        fs::write(store.dir("main").unwrap().join("keep.txt"), b"keep").unwrap();
        let restored = restore_trash(&store, key).unwrap();
        let id = s(&restored["instance"], "id");
        assert_ne!(id, "main");
        assert_eq!(restored["instance"]["launchSettings"]["ram"], 4);
        assert_eq!(
            fs::read(store.dir(id).unwrap().join("saves/World/level.dat")).unwrap(),
            b"real world"
        );
        assert_eq!(
            fs::read(store.dir("main").unwrap().join("keep.txt")).unwrap(),
            b"keep"
        );
        assert!(trash(&store).unwrap().as_array().unwrap().is_empty());
        assert!(restore_trash(&store, "../../db.json").is_err());
    }
    #[test]
    fn cleanup_only_removes_reviewed_old_partial_files() {
        let (_temp, store) = fixture();
        let downloads = store.root.join("downloads");
        fs::create_dir_all(&downloads).unwrap();
        let old = downloads.join("old.part");
        fs::write(&old, b"old partial").unwrap();
        fs::write(downloads.join("new.part"), b"active").unwrap();
        fs::write(downloads.join("ready.jar"), b"installed").unwrap();
        let times =
            fs::FileTimes::new().set_modified(SystemTime::now() - Duration::from_secs(172800));
        fs::OpenOptions::new()
            .write(true)
            .open(&old)
            .unwrap()
            .set_times(times)
            .unwrap();
        let plan = cleanup_preview(&store).unwrap();
        assert_eq!(plan.as_array().unwrap().len(), 1);
        assert!(cleanup(&store, &json!([plan[0]["id"], "forged"])).is_err());
        assert!(old.exists());
        let result = cleanup(&store, &json!([plan[0]["id"]])).unwrap();
        assert_eq!(result["count"], 1);
        assert!(!old.exists());
        assert!(downloads.join("new.part").exists());
        assert!(downloads.join("ready.jar").exists());
        assert!(store
            .dir("main")
            .unwrap()
            .join("saves/World/level.dat")
            .exists());
    }
    #[test]
    fn comparison_and_timeline_detect_changes_without_reading_worlds() {
        let (_temp, store) = fixture();
        let root = store.dir("main").unwrap();
        history(&store, "main").unwrap();
        let copy = store.duplicate("main").unwrap();
        let id = s(&copy["instance"], "id");
        fs::write(store.dir(id).unwrap().join("mods/new.jar"), b"new").unwrap();
        let comparison = compare(&store, "main", id).unwrap();
        assert_eq!(comparison["changes"].as_array().unwrap().len(), 1);
        assert_eq!(comparison["changes"][0]["name"], "mods/new.jar");
        fs::write(root.join("mods/old.jar"), b"changed").unwrap();
        let log = history(&store, "main").unwrap();
        assert_eq!(log[0]["label"], "Изменения вне Nova");
        assert_eq!(log[0]["changes"][0]["kind"], "changed");
        assert_eq!(
            history(&store, "main").unwrap().as_array().unwrap().len(),
            1
        );
    }
    #[test]
    fn screenshots_are_scoped_paginated_and_cannot_launch_non_images() {
        let (_temp, store) = fixture();
        let root = store.dir("main").unwrap().join("screenshots");
        fs::create_dir_all(&root).unwrap();
        for index in 0..14 {
            fs::write(root.join(format!("{index}.png")), b"png fixture").unwrap();
        }
        fs::write(root.join("run.command"), b"not an image").unwrap();
        let first = screenshots(&store, "main", 0).unwrap();
        assert_eq!(first["total"], 14);
        assert_eq!(first["items"].as_array().unwrap().len(), 12);
        assert_eq!(
            screenshots(&store, "main", 12).unwrap()["items"]
                .as_array()
                .unwrap()
                .len(),
            2
        );
        assert!(open_screenshot(&store, "main", "../db.json").is_err());
        assert!(open_screenshot(&store, "main", "run.command").is_err());
    }
    #[test]
    fn storage_does_not_double_count_worlds_or_follow_symlinks() {
        let (_temp, store) = fixture();
        let data = storage(&store).unwrap();
        let instance = &data["instances"][0];
        assert_eq!(instance["worlds"], 10);
        assert_eq!(instance["mods"], 3);
        assert_eq!(instance["bytes"], 13);
        #[cfg(unix)]
        {
            std::os::unix::fs::symlink(&store.root, store.dir("main").unwrap().join("link"))
                .unwrap();
            assert_eq!(storage(&store).unwrap()["instances"][0]["bytes"], 13);
        }
    }
}
