use crate::{
    backups,
    net::{enc, Net},
    store::Store,
    util::*,
};
use anyhow::{bail, Context, Result};
use serde_json::{json, Value};
use std::{collections::HashSet, fs, io::Read, path::Path};
pub const API: &str = "https://api.modrinth.com/v2";
pub fn folder(kind: &str) -> Result<&'static str> {
    match kind {
        "mod" => Ok("mods"),
        "shader" => Ok("shaderpacks"),
        "resourcepack" => Ok("resourcepacks"),
        _ => bail!("Неизвестный тип контента"),
    }
}
fn index(root: &Path) -> Result<Value> {
    let p = root.join(".nova-content.json");
    if p.exists() {
        read_json(&p)
    } else {
        let mut result = json!({});
        if let Ok(legacy) = read_json(&root.join("nova-content.json")) {
            for entry in legacy.as_array().unwrap_or(&vec![]) {
                if let Ok(folder) = folder(s(entry, "type")) {
                    result[format!("{}/{}", folder, s(entry, "filename"))] =
                        json!({"project_id":s(entry,"project"),"version_id":s(entry,"version")});
                }
            }
        }
        Ok(result)
    }
}
fn primary(v: &Value) -> Result<&Value> {
    let files = list(v, "files");
    files
        .iter()
        .find(|f| flag(f, "primary", false))
        .or_else(|| files.first())
        .context("У версии нет файлов")
}
fn includes(v: &Value, key: &str, name: &str) -> bool {
    list(v, key).iter().any(|e| e.as_str() == Some(name))
}
pub fn search(net: &Net, options: &Value) -> Result<Value> {
    let kind = s(options, "type");
    if kind == "mod" && !["Fabric", "Quilt", "Forge", "NeoForge"].contains(&s(options, "loader")) {
        return Ok(json!([]));
    }
    let mut facets = vec![json!([format!("project_type:{kind}")])];
    if !s(options, "ver").is_empty() {
        facets.push(json!([format!("versions:{}", s(options, "ver"))]));
    }
    if kind == "mod" {
        if s(options, "loader") == "Quilt" {
            facets.push(json!(["categories:quilt", "categories:fabric"]));
        } else {
            facets.push(json!([format!(
                "categories:{}",
                s(options, "loader").to_lowercase()
            )]));
        }
        facets.push(json!(["client_side:required", "client_side:optional"]));
    }
    let r = net.json(&format!(
        "{API}/search?query={}&facets={}&limit=30",
        enc(s(options, "q")),
        enc(&json!(facets).to_string())
    ))?;
    Ok(r["hits"].clone())
}
pub fn versions(net: &Net, project: &str, mc: &str, loader: &str, kind: &str) -> Result<Value> {
    let mut url = format!(
        "{API}/project/{}/version?game_versions={}",
        enc(project),
        enc(&json!([mc]).to_string())
    );
    if kind == "mod" && loader != "Vanilla" {
        url.push_str(&format!(
            "&loaders={}",
            enc(&if loader == "Quilt" {
                json!(["quilt", "fabric"]).to_string()
            } else {
                json!([loader.to_lowercase()]).to_string()
            })
        ));
    }
    net.json(&url)
}
pub fn files(root: &Path, kind: &str) -> Result<Value> {
    let dir = root.join(folder(kind)?);
    let meta = index(root)?;
    let mut files = vec![];
    if dir.exists() {
        for entry in fs::read_dir(&dir)? {
            let p = entry?.path();
            if !p.is_file() || p.is_symlink() {
                continue;
            }
            let name = p.file_name().unwrap().to_string_lossy().to_string();
            let bare = name.trim_end_matches(".disabled");
            if !(if kind == "mod" {
                bare.ends_with(".jar")
            } else {
                bare.ends_with(".zip")
            }) {
                continue;
            }
            let m = &meta[format!("{}/{bare}", folder(kind)?)];
            files.push(json!({"name":name,"type":kind,"active":if kind=="mod"{!name.ends_with(".disabled")}else{active(root,kind,bare)},"project":s(m,"project_id"),"metadata":m}));
        }
    }
    files.sort_by(|a, b| s(a, "name").cmp(s(b, "name")));
    Ok(json!(files))
}
pub fn install(
    store: &Store,
    net: &Net,
    inst: &Value,
    project: &str,
    kind: &str,
    log: &dyn Fn(String),
) -> Result<String> {
    if s(inst, "loader") == "Rust" {
        bail!("Java-контент не подходит для Horizon");
    }
    if kind == "mod" && s(inst, "loader") == "Vanilla" {
        bail!("Для модов выберите Fabric, Quilt, Forge или NeoForge");
    }
    let found = versions(net, project, s(inst, "version"), s(inst, "loader"), kind)?;
    let latest = found
        .as_array()
        .and_then(|a| {
            a.iter()
                .find(|v| s(v, "version_type") == "release")
                .or_else(|| if kind == "mod" { None } else { a.first() })
        })
        .context("Нет совместимой стабильной версии контента")?;
    let root = store.dir(s(inst, "id"))?;
    let backup = backups::snapshot(&root, "mods", "Перед установкой контента")?;
    match install_version(&root, net, inst, latest, kind, &mut HashSet::new(), log) {
        Ok(name) => Ok(name),
        Err(e) => {
            let _ = backups::restore(&root, "mods", s(&backup, "id"));
            Err(e)
        }
    }
}
fn install_version(
    root: &Path,
    net: &Net,
    inst: &Value,
    version: &Value,
    kind: &str,
    seen: &mut HashSet<String>,
    log: &dyn Fn(String),
) -> Result<String> {
    if !seen.insert(s(version, "id").into()) {
        return Ok(String::new());
    }
    if seen.len() > 200 {
        bail!("Слишком длинная цепочка зависимостей");
    }
    let project_key = format!("project:{}:", s(version, "project_id"));
    let selected = format!("{}{}", project_key, s(version, "id"));
    if seen
        .iter()
        .any(|v| v.starts_with(&project_key) && v != &selected)
    {
        bail!("Зависимости требуют разные версии одного проекта");
    }
    seen.insert(selected);
    for dep in list(version, "dependencies") {
        if s(dep, "dependency_type") != "required" {
            continue;
        }
        let exact = if s(dep, "version_id").is_empty() {
            None
        } else {
            Some(net.json(&format!("{API}/version/{}", s(dep, "version_id")))?)
        };
        let id = if !s(dep, "project_id").is_empty() {
            s(dep, "project_id").to_owned()
        } else {
            exact
                .as_ref()
                .map(|v| s(v, "project_id").to_owned())
                .context("Нужна ручная установка зависимости")?
        };
        let project = net.json(&format!("{API}/project/{}", enc(&id)))?;
        let dep_type = s(&project, "project_type");
        if !["mod", "shader", "resourcepack"].contains(&dep_type) {
            bail!("Неподдерживаемая зависимость контента");
        }
        if dep_type == "mod" && s(inst, "loader") == "Vanilla" {
            bail!("Для зависимости нужен загрузчик модов");
        }
        let next = if let Some(v) = exact {
            v
        } else {
            let found = versions(net, &id, s(inst, "version"), s(inst, "loader"), dep_type)?;
            found
                .as_array()
                .and_then(|a| {
                    a.iter()
                        .find(|v| s(v, "version_type") == "release")
                        .or_else(|| if dep_type == "mod" { None } else { a.first() })
                })
                .context("Нет обязательной зависимости")?
                .clone()
        };
        if !includes(&next, "game_versions", s(inst, "version"))
            || (dep_type == "mod"
                && !includes(&next, "loaders", &s(inst, "loader").to_lowercase())
                && !(s(inst, "loader") == "Quilt" && includes(&next, "loaders", "fabric")))
        {
            bail!("Зависимость несовместима со сборкой");
        }
        install_version(root, net, inst, &next, dep_type, seen, log)?;
    }
    let file = primary(version)?;
    let name = leaf(s(file, "filename"))?;
    if (kind == "mod" && !name.ends_with(".jar")) || (kind != "mod" && !name.ends_with(".zip")) {
        bail!("Неподходящий файл контента");
    }
    let sha = s(&file["hashes"], "sha512");
    if sha.len() != 128 {
        bail!("Нет SHA-512 файла");
    }
    let project = s(version, "project_id");
    let mut meta = index(root)?;
    let mut previous = vec![];
    let mut disabled = false;
    for (k, v) in meta.as_object().unwrap() {
        if k.starts_with(&format!("{}/", folder(kind)?)) && s(v, "project_id") == project {
            previous.push(k.clone());
            disabled |= safe(root, &format!("{k}.disabled"))?.exists();
        }
    }
    let dest = root
        .join(folder(kind)?)
        .join(format!("{name}{}", if disabled { ".disabled" } else { "" }));
    let temp = root
        .join(".nova-downloads")
        .join(format!("{}-{name}", uuid::Uuid::new_v4()));
    log(format!("Установка {name}"));
    net.download(
        s(file, "url"),
        &temp,
        "sha512",
        sha,
        number(file, "size", 0),
    )?;
    fs::create_dir_all(dest.parent().unwrap())?;
    fs::rename(temp, &dest)?;
    for key in previous {
        let p = safe(root, &key)?;
        let off = safe(root, &format!("{key}.disabled"))?;
        for old in [p, off] {
            if old != dest && old.exists() {
                fs::remove_file(old)?;
            }
        }
        meta.as_object_mut().unwrap().remove(&key);
    }
    meta[format!("{}/{name}", folder(kind)?)] = json!({"project_id":project,"version_id":s(version,"id"),"version_number":s(version,"version_number"),"url":s(file,"url"),"hashes":file["hashes"],"size":file["size"]});
    write_json(&root.join(".nova-content.json"), &meta)?;
    if kind == "resourcepack" && !disabled {
        resource(root, name, true)?;
    }
    if kind == "shader" && !disabled {
        shader(root, name, true)?;
    }
    Ok(name.to_string())
}
pub fn active(root: &Path, kind: &str, name: &str) -> bool {
    if kind == "resourcepack" {
        let text = fs::read_to_string(root.join("options.txt")).unwrap_or_default();
        return text
            .lines()
            .find_map(|l| l.strip_prefix("resourcePacks:"))
            .and_then(|v| serde_json::from_str::<Vec<String>>(v).ok())
            .is_some_and(|v| v.iter().any(|v| v == name || v == &format!("file/{name}")));
    }
    [
        ("config/iris.properties", true),
        ("optionsshaders.txt", false),
    ]
    .iter()
    .any(|(file, iris)| {
        let text = fs::read_to_string(root.join(file)).unwrap_or_default();
        text.lines().any(|l| l == format!("shaderPack={name}"))
            && (!iris || !text.lines().any(|l| l == "enableShaders=false"))
    })
}
pub fn resource(root: &Path, name: &str, enabled: bool) -> Result<()> {
    let p = root.join("options.txt");
    let original = fs::read_to_string(&p).unwrap_or_default();
    let mut packs = original
        .lines()
        .find_map(|l| l.strip_prefix("resourcePacks:"))
        .and_then(|v| serde_json::from_str::<Vec<String>>(v).ok())
        .unwrap_or_default();
    let token = format!("file/{name}");
    packs.retain(|p| p != &token && p != name);
    if enabled {
        packs.push(token);
    }
    let mut lines = original
        .lines()
        .filter(|l| !l.starts_with("resourcePacks:"))
        .map(str::to_owned)
        .collect::<Vec<_>>();
    lines.push(format!("resourcePacks:{}", json!(packs)));
    atomic(&p, lines.join("\n").as_bytes())
}
pub fn shader(root: &Path, name: &str, enabled: bool) -> Result<()> {
    for (path, iris) in [
        (root.join("config/iris.properties"), true),
        (root.join("optionsshaders.txt"), false),
    ] {
        let original = fs::read_to_string(&path).unwrap_or_default();
        let selected = original
            .lines()
            .find_map(|l| l.strip_prefix("shaderPack="))
            .unwrap_or("");
        if !enabled && selected != name {
            continue;
        }
        let mut lines = original
            .lines()
            .filter(|l| {
                !l.starts_with("shaderPack=") && (!iris || !l.starts_with("enableShaders="))
            })
            .map(str::to_owned)
            .collect::<Vec<_>>();
        lines.push(format!("shaderPack={}", if enabled { name } else { "" }));
        if iris {
            lines.push(format!("enableShaders={enabled}"));
        }
        atomic(&path, lines.join("\n").as_bytes())?;
    }
    Ok(())
}
pub fn toggle(root: &Path, kind: &str, name: &str) -> Result<Value> {
    if kind != "mod" {
        let enabled = !active(root, kind, name);
        safe(&root.join(folder(kind)?), leaf(name)?)?;
        if kind == "resourcepack" {
            resource(root, name, enabled)?;
        } else {
            shader(root, name, enabled)?;
        }
        return Ok(json!(true));
    }
    leaf(name)?;
    let source = safe(&root.join(folder(kind)?), name)?;
    let bare = name.trim_end_matches(".disabled");
    let enabled = name.ends_with(".disabled");
    let dest = source.with_file_name(if enabled {
        bare.to_string()
    } else {
        format!("{name}.disabled")
    });
    if dest.exists() {
        bail!("Файл уже существует");
    }
    fs::rename(source, dest)?;
    if kind == "resourcepack" {
        resource(root, bare, enabled)?;
    }
    if kind == "shader" {
        shader(root, bare, enabled)?;
    }
    Ok(json!(true))
}
pub fn remove(root: &Path, kind: &str, name: &str) -> Result<Value> {
    backups::snapshot(root, "mods", "Перед удалением контента")?;
    let p = safe(&root.join(folder(kind)?), leaf(name)?)?;
    fs::remove_file(p)?;
    if kind == "resourcepack" {
        resource(root, name.trim_end_matches(".disabled"), false)?;
    }
    if kind == "shader" {
        shader(root, name.trim_end_matches(".disabled"), false)?;
    }
    Ok(json!(true))
}
fn identify(root: &Path, net: &Net, item: &Value) -> Result<Value> {
    let meta = &item["metadata"];
    if !s(meta, "version_id").is_empty() {
        return net.json(&format!("{API}/version/{}", s(meta, "version_id")));
    }
    let hash = sha(
        &root.join(folder(s(item, "type"))?).join(s(item, "name")),
        "sha512",
    )?;
    net.json(&format!("{API}/version_file/{hash}?algorithm=sha512"))
}
pub fn updates(root: &Path, net: &Net, inst: &Value) -> Result<Value> {
    let items = files(root, "mod")?;
    let mut records = vec![];
    for item in items.as_array().unwrap() {
        records.push((
            s(item, "name").to_owned(),
            sha(&root.join("mods").join(s(item, "name")), "sha512")?,
        ));
    }
    if records.is_empty() {
        return Ok(json!({"updates":[],"unknown":[]}));
    }
    let hashes = records.iter().map(|(_, h)| h).collect::<Vec<_>>();
    let known = net.post(
        &format!("{API}/version_files"),
        &json!({"hashes":hashes,"algorithm":"sha512"}),
    )?;
    let loaders = if s(inst, "loader") == "Quilt" {
        json!(["quilt", "fabric"])
    } else {
        json!([s(inst, "loader").to_lowercase()])
    };
    let latest=net.post(&format!("{API}/version_files/update"),&json!({"hashes":hashes,"algorithm":"sha512","loaders":loaders,"game_versions":[s(inst,"version")],"version_types":["release"]}))?;
    let mut updates = vec![];
    let mut unknown = vec![];
    for (name, hash) in records {
        let old = &known[&hash];
        let next = &latest[&hash];
        if old.is_null() {
            unknown.push(name);
            continue;
        }
        if next.is_null()
            || s(old, "id") == s(next, "id")
            || s(old, "project_id") != s(next, "project_id")
            || s(next, "date_published") <= s(old, "date_published")
        {
            continue;
        }
        let file = primary(next)?;
        let filename = leaf(s(file, "filename"))?;
        if !filename.ends_with(".jar") || s(&file["hashes"], "sha512") == hash {
            continue;
        }
        updates.push(json!({"name":name,"to":format!("{filename}{}",if name.ends_with(".disabled"){".disabled"}else{""}),"version":s(next,"version_number"),"fromVersion":s(old,"version_number"),"_version":next,"_old":old}));
    }
    Ok(json!({"updates":updates,"unknown":unknown}))
}
pub fn apply_updates(
    root: &Path,
    net: &Net,
    inst: &Value,
    names: &Value,
    log: &dyn Fn(String),
) -> Result<Value> {
    let plan = updates(root, net, inst)?;
    let names = names.as_array().context("Некорректный список модов")?;
    let selected = list(&plan, "updates")
        .iter()
        .filter(|u| names.iter().any(|n| n.as_str() == Some(s(u, "name"))))
        .collect::<Vec<_>>();
    if selected.len() != names.len() {
        bail!("Список модов изменился. Проверьте обновления снова.");
    }
    let backup = backups::snapshot(root, "mods", "Перед обновлением модов")?;
    for item in &selected {
        let name = s(item, "name").trim_end_matches(".disabled");
        let mut meta = index(root)?;
        meta[format!("mods/{name}")] =
            json!({"project_id":s(&item["_old"],"project_id"),"version_id":s(&item["_old"],"id")});
        write_json(&root.join(".nova-content.json"), &meta)?;
        if let Err(e) = install_version(
            root,
            net,
            inst,
            &item["_version"],
            "mod",
            &mut HashSet::new(),
            log,
        ) {
            let _ = backups::restore(root, "mods", s(&backup, "id"));
            return Err(e);
        }
    }
    Ok(json!({"count":selected.len()}))
}
pub fn manage(
    root: &Path,
    net: &Net,
    inst: &Value,
    kind: &str,
    name: &str,
    action: &str,
    log: &dyn Fn(String),
) -> Result<Value> {
    match action {
        "toggle" => toggle(root, kind, name),
        "remove" => remove(root, kind, name),
        "update" => {
            let current = files(root, kind)?
                .as_array()
                .unwrap()
                .iter()
                .find(|i| s(i, "name") == name)
                .cloned()
                .context("Файл не найден")?;
            let old = identify(root, net, &current)?;
            let found = versions(
                net,
                s(&old, "project_id"),
                s(inst, "version"),
                s(inst, "loader"),
                kind,
            )?;
            let latest = found
                .as_array()
                .and_then(|v| v.first())
                .context("Нет совместимой версии")?;
            if s(latest, "id") == s(&old, "id") {
                return Ok(json!({"changed":false}));
            }
            backups::snapshot(root, "mods", "Перед обновлением контента")?;
            install_version(root, net, inst, latest, kind, &mut HashSet::new(), log)?;
            Ok(json!({"changed":true}))
        }
        _ => bail!("Неизвестное действие"),
    }
}
pub fn import_pack(store: &Store, net: &Net, file: &Path, log: &dyn Fn(String)) -> Result<Value> {
    let mut zip = zip::ZipArchive::new(fs::File::open(file)?)?;
    let mut text = String::new();
    {
        let f = zip.by_name("modrinth.index.json")?;
        if f.size() > 16 * 1024 * 1024 {
            bail!("Слишком большой индекс сборки");
        }
        f.take(16 * 1024 * 1024).read_to_string(&mut text)?;
    }
    let manifest: Value = serde_json::from_str(&text)?;
    if s(&manifest, "game") != "minecraft" || number(&manifest, "formatVersion", 0) != 1 {
        bail!("Неправильный .mrpack");
    }
    let deps = &manifest["dependencies"];
    let (loader, key) = if !s(deps, "fabric-loader").is_empty() {
        ("Fabric", "fabric-loader")
    } else if !s(deps, "quilt-loader").is_empty() {
        ("Quilt", "quilt-loader")
    } else if !s(deps, "forge").is_empty() {
        ("Forge", "forge")
    } else if !s(deps, "neoforge").is_empty() {
        ("NeoForge", "neoforge")
    } else {
        ("Vanilla", "")
    };
    let inst = json!({"id":uuid::Uuid::new_v4().to_string(),"name":s(&manifest,"name"),"version":s(deps,"minecraft"),"loader":loader,"loaderVersion":s(deps,key)});
    let root = store.root.join("instances").join(s(&inst, "id"));
    fs::create_dir_all(&root)?;
    let result = (|| {
        for f in list(&manifest, "files") {
            if s(&f["env"], "client") == "unsupported" {
                continue;
            }
            let p = safe(&root, s(f, "path"))?;
            let url = f["downloads"][0]
                .as_str()
                .context("Нет загрузки файла сборки")?;
            if !url.starts_with("https://") || s(&f["hashes"], "sha512").len() != 128 {
                bail!("Нет защищённой загрузки с SHA-512");
            }
            log(format!(
                "Сборка: {}",
                p.file_name().unwrap().to_string_lossy()
            ));
            net.download(
                url,
                &p,
                "sha512",
                s(&f["hashes"], "sha512"),
                number(f, "fileSize", 0),
            )?;
        }
        drop(zip);
        unzip(file, &root, "overrides/")?;
        unzip(file, &root, "client-overrides/")?;
        store.add(inst.clone())?;
        Ok(inst)
    })();
    if result.is_err() {
        let _ = fs::remove_dir_all(root);
    }
    result
}
pub fn install_pack(
    store: &Store,
    net: &Net,
    project: &str,
    mc: &str,
    log: &dyn Fn(String),
) -> Result<Value> {
    let found = versions(net, project, mc, "Vanilla", "modpack")?;
    let latest = found
        .as_array()
        .and_then(|v| v.first())
        .context("Нет сборки для выбранной версии")?;
    let f = primary(latest)?;
    let p = store
        .root
        .join("downloads")
        .join(format!("{}.mrpack", uuid::Uuid::new_v4()));
    net.download(
        s(f, "url"),
        &p,
        "sha512",
        s(&f["hashes"], "sha512"),
        number(f, "size", 0),
    )?;
    let result = import_pack(store, net, &p, log);
    let _ = fs::remove_file(p);
    result
}
pub fn export_pack(store: &Store, inst: &Value, target: &Path) -> Result<Value> {
    if s(inst, "loader") == "Rust" {
        bail!("Horizon не экспортируется в Java .mrpack");
    }
    let root = store.dir(s(inst, "id"))?;
    let stage = store
        .root
        .join("tmp")
        .join(uuid::Uuid::new_v4().to_string());
    fs::create_dir_all(&stage)?;
    let result = (|| {
        let mut deps = json!({"minecraft":s(inst,"version")});
        if s(inst, "loader") != "Vanilla" {
            let key = match s(inst, "loader") {
                "Fabric" => "fabric-loader",
                "Quilt" => "quilt-loader",
                "Forge" => "forge",
                _ => "neoforge",
            };
            if s(inst, "loaderVersion").is_empty() {
                bail!("Сначала запустите сборку для определения версии загрузчика");
            }
            deps[key] = json!(s(inst, "loaderVersion"));
        }
        let mut files = vec![];
        let mut managed = HashSet::new();
        for (k, m) in index(&root)?.as_object().unwrap() {
            let p = safe(&root, k)?;
            if !p.is_file()
                || !s(m, "url").starts_with("https://")
                || sha(&p, "sha512")? != s(&m["hashes"], "sha512")
            {
                continue;
            }
            files.push(json!({"path":k,"hashes":{"sha512":sha(&p,"sha512")?,"sha1":sha(&p,"sha1")?},"downloads":[s(m,"url")],"fileSize":fs::metadata(&p)?.len(),"env":{"client":"required","server":"optional"}}));
            managed.insert(k.clone());
        }
        for name in [
            "mods",
            "config",
            "shaderpacks",
            "resourcepacks",
            "options.txt",
            "optionsshaders.txt",
            "servers.dat",
        ] {
            copy_tree(&root.join(name), &stage.join("overrides").join(name), &[])?;
        }
        for k in managed {
            let _ = fs::remove_file(safe(&stage.join("overrides"), &k)?);
        }
        write_json(
            &stage.join("modrinth.index.json"),
            &json!({"formatVersion":1,"game":"minecraft","versionId":"1.0.0","name":s(inst,"name"),"dependencies":deps,"files":files}),
        )?;
        zip_tree(&stage, target)?;
        Ok(json!({"count":files.len(),"omitted":[]}))
    })();
    let _ = fs::remove_dir_all(stage);
    result
}
