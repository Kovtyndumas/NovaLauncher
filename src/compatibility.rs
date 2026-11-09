use crate::{content, net::Net, util::*};
use anyhow::{bail, Result};
use serde_json::{json, Value};
use std::{
    collections::HashMap,
    fs,
    io::{Read, Seek},
    path::Path,
};
#[derive(Clone, Default)]
struct Mod {
    file: String,
    id: String,
    version: String,
    provides: Vec<String>,
    depends: Value,
    breaks: Value,
    loader: String,
    server: bool,
}
pub fn matches(version: &str, range: &Value) -> Option<bool> {
    if let Some(ranges) = range.as_array() {
        let values = ranges
            .iter()
            .map(|r| matches(version, r))
            .collect::<Vec<_>>();
        return if values.contains(&Some(true)) {
            Some(true)
        } else if values.contains(&None) {
            None
        } else {
            Some(false)
        };
    }
    let range = range.as_str()?;
    if range.contains("||") {
        return matches(
            version,
            &json!(range.split("||").map(str::trim).collect::<Vec<_>>()),
        );
    }
    if range == "*" || range.is_empty() {
        return Some(true);
    }
    fn parse(s: &str) -> Option<Vec<u32>> {
        if s.is_empty() || !s.chars().all(|c| c.is_ascii_digit() || c == '.') {
            return None;
        }
        s.split('.').map(|p| p.parse().ok()).collect()
    }
    fn cmp(a: &[u32], b: &[u32]) -> std::cmp::Ordering {
        for n in 0..a.len().max(b.len()) {
            let c = a.get(n).unwrap_or(&0).cmp(b.get(n).unwrap_or(&0));
            if !c.is_eq() {
                return c;
            }
        }
        std::cmp::Ordering::Equal
    }
    let a = parse(version)?;
    let normalized = regex::Regex::new(r"(>=|<=|>|<|=|~|\^)\s+")
        .unwrap()
        .replace_all(range, "$1");
    let re = regex::Regex::new(r"^(>=|<=|>|<|=|~|\^)?(\d+(?:\.(?:\d+|[xX*]))*)$").unwrap();
    let mut unknown = false;
    for part in normalized.split_whitespace() {
        let Some(m) = re.captures(part) else {
            unknown = true;
            continue;
        };
        let op = m.get(1).map(|v| v.as_str()).unwrap_or("=");
        let raw = &m[2];
        if raw.contains(['x', 'X', '*']) {
            if op != "=" {
                unknown = true;
                continue;
            }
            for (n, p) in raw.split('.').enumerate() {
                if ["x", "X", "*"].contains(&p) {
                    continue;
                }
                if p.parse::<u32>().ok() != Some(*a.get(n).unwrap_or(&0)) {
                    return Some(false);
                }
            }
            continue;
        }
        let b = parse(raw)?;
        let c = cmp(&a, &b);
        let good = match op {
            "=" => c.is_eq(),
            ">" => c.is_gt(),
            "<" => c.is_lt(),
            ">=" => !c.is_lt(),
            "<=" => !c.is_gt(),
            "~" => {
                let mut upper = b.clone();
                let n = if upper.len() == 1 { 0 } else { 1 };
                upper[n] += 1;
                upper.truncate(n + 1);
                !c.is_lt() && cmp(&a, &upper).is_lt()
            }
            "^" => {
                let mut upper = b.clone();
                let n = upper
                    .iter()
                    .position(|v| *v != 0)
                    .unwrap_or(upper.len() - 1);
                upper[n] += 1;
                upper.truncate(n + 1);
                !c.is_lt() && cmp(&a, &upper).is_lt()
            }
            _ => false,
        };
        if !good {
            return Some(false);
        }
    }
    if unknown {
        None
    } else {
        Some(true)
    }
}
fn inspect<R: Read + Seek>(
    reader: R,
    file: &str,
    preferred: &str,
    depth: u32,
    mods: &mut Vec<Mod>,
) -> Result<()> {
    let mut zip = zip::ZipArchive::new(reader)?;
    let names = zip.file_names().map(str::to_owned).collect::<Vec<_>>();
    if depth == 0
        && ((preferred == "Forge" && names.iter().any(|n| n == "META-INF/mods.toml"))
            || (preferred == "NeoForge"
                && names.iter().any(|n| n == "META-INF/neoforge.mods.toml")))
    {
        mods.push(Mod {
            file: file.into(),
            loader: preferred.into(),
            ..Mod::default()
        });
        return Ok(());
    }
    if names.iter().any(|n| n == "fabric.mod.json") {
        let meta: Value;
        {
            let mut f = zip.by_name("fabric.mod.json")?;
            if f.size() > 1024 * 1024 {
                bail!("Слишком большие метаданные");
            }
            let mut text = String::new();
            f.read_to_string(&mut text)?;
            meta = serde_json::from_str(&text)?;
        }
        mods.push(Mod {
            file: file.into(),
            id: s(&meta, "id").into(),
            version: s(&meta, "version").into(),
            provides: list(&meta, "provides")
                .iter()
                .filter_map(Value::as_str)
                .map(str::to_owned)
                .collect(),
            depends: meta["depends"].clone(),
            breaks: meta["breaks"].clone(),
            loader: "Fabric".into(),
            server: s(&meta, "environment") == "server",
        });
        if depth < 1 {
            for jar in list(&meta, "jars").iter().take(64) {
                if let Ok(mut f) = zip.by_name(s(jar, "file")) {
                    if f.size() < 16 * 1024 * 1024 {
                        let mut bytes = vec![];
                        f.read_to_end(&mut bytes)?;
                        inspect(
                            std::io::Cursor::new(bytes),
                            file,
                            preferred,
                            depth + 1,
                            mods,
                        )?;
                    }
                }
            }
        }
    } else if names.iter().any(|n| n == "quilt.mod.json") {
        let mut text = String::new();
        zip.by_name("quilt.mod.json")?.read_to_string(&mut text)?;
        let meta: Value = serde_json::from_str(&text)?;
        let meta = &meta["quilt_loader"];
        let mut depends = json!({});
        for d in list(meta, "depends") {
            if !s(d, "id").is_empty() && !flag(d, "optional", false) {
                depends[s(d, "id")] = d.get("versions").cloned().unwrap_or(json!("*"));
            }
        }
        mods.push(Mod {
            file: file.into(),
            id: s(meta, "id").into(),
            version: s(meta, "version").into(),
            loader: "Quilt".into(),
            depends,
            provides: list(meta, "provides")
                .iter()
                .filter_map(|v| v.as_str().or_else(|| v["id"].as_str()))
                .map(str::to_owned)
                .collect(),
            ..Mod::default()
        });
    } else {
        for (kind, path) in [
            ("NeoForge", "META-INF/neoforge.mods.toml"),
            ("Forge", "META-INF/mods.toml"),
        ] {
            if names.iter().any(|n| n == path) {
                mods.push(Mod {
                    file: file.into(),
                    loader: kind.into(),
                    ..Mod::default()
                });
                break;
            }
        }
    }
    Ok(())
}
pub fn check(root: &Path, net: &Net, inst: &Value, want_online: bool) -> Result<Value> {
    let files = content::files(root, "mod")?;
    let mut issues = Vec::<Value>::new();
    let mut mods = vec![];
    let mut items = vec![];
    let mut ids = HashMap::<String, (String, String)>::new();
    let mut checked = 0;
    let mut disabled = 0;
    fn issue(issues: &mut Vec<Value>, severity: &str, file: &str, message: String) {
        if !issues
            .iter()
            .any(|v| s(v, "file") == file && s(v, "message") == message)
        {
            issues.push(json!({"severity":severity,"file":file,"message":message}));
        }
    }
    for item in files.as_array().unwrap() {
        if !flag(item, "active", true) {
            disabled += 1;
            continue;
        }
        checked += 1;
        let name = s(item, "name");
        let p = root.join("mods").join(name);
        let mut local = vec![];
        match inspect(fs::File::open(&p)?, name, s(inst, "loader"), 0, &mut local) {
            Ok(()) => {}
            Err(e) => issue(
                &mut issues,
                "error",
                name,
                format!("Не удалось прочитать JAR: {e}"),
            ),
        };
        let hash = sha(&p, "sha512")?;
        items.push((name.to_owned(), hash, local.clone()));
        for m in &local {
            for id in std::iter::once(&m.id)
                .chain(&m.provides)
                .filter(|id| !id.is_empty())
            {
                if let Some((_, file)) = ids.get(id) {
                    if file != name {
                        issue(
                            &mut issues,
                            "error",
                            name,
                            format!("Дубликат мода {id} в {file}"),
                        );
                    }
                } else {
                    ids.insert(id.clone(), (m.version.clone(), name.into()));
                }
            }
            if m.loader != s(inst, "loader")
                && !(s(inst, "loader") == "Quilt" && m.loader == "Fabric")
            {
                issue(
                    &mut issues,
                    if m.loader == "Forge" && s(inst, "loader") == "NeoForge" {
                        "warning"
                    } else {
                        "error"
                    },
                    name,
                    format!("Мод для {}, выбран {}", m.loader, s(inst, "loader")),
                );
            }
            if m.server {
                issue(
                    &mut issues,
                    "warning",
                    name,
                    "Мод предназначен только для сервера".into(),
                );
            }
        }
        mods.extend(local);
    }
    ids.insert(
        "minecraft".into(),
        (s(inst, "version").into(), String::new()),
    );
    if !s(inst, "loaderVersion").is_empty() {
        let id = match s(inst, "loader") {
            "Fabric" => "fabricloader",
            "Quilt" => "quilt_loader",
            "Forge" => "forge",
            _ => "neoforge",
        };
        ids.insert(id.into(), (s(inst, "loaderVersion").into(), String::new()));
    }
    for m in &mods {
        if let Some(depends) = m.depends.as_object() {
            for (id, range) in depends {
                if id == "java" {
                    continue;
                }
                if (id == "fabricloader" && ["Fabric", "Quilt"].contains(&s(inst, "loader")))
                    || (id == "quilt_loader" && s(inst, "loader") == "Quilt")
                {
                    if !ids.contains_key(id) {
                        continue;
                    }
                }
                let Some((version, _)) = ids.get(id) else {
                    issue(
                        &mut issues,
                        "error",
                        &m.file,
                        format!("Не найдена обязательная зависимость: {id}"),
                    );
                    continue;
                };
                match matches(version, range) {
                    Some(false) => issue(
                        &mut issues,
                        "error",
                        &m.file,
                        format!("Не подходит {id} {version}; требуется {range}"),
                    ),
                    None => issue(
                        &mut issues,
                        "warning",
                        &m.file,
                        format!("Не удалось полностью проверить диапазон {id}: {range}"),
                    ),
                    _ => {}
                }
            }
        }
        if let Some(breaks) = m.breaks.as_object() {
            for (id, range) in breaks {
                if let Some((version, _)) = ids.get(id) {
                    if matches(version, range) == Some(true) {
                        issue(
                            &mut issues,
                            "error",
                            &m.file,
                            format!("Заявлен конфликт с {id} {version}"),
                        );
                    }
                }
            }
        }
    }
    let mut online = false;
    if want_online && !items.is_empty() {
        let hashes = items.iter().map(|(_, hash, _)| hash).collect::<Vec<_>>();
        if let Ok(known) = net.post(
            &format!("{}/version_files", content::API),
            &json!({"hashes":hashes,"algorithm":"sha512"}),
        ) {
            online = true;
            let versions = known
                .as_object()
                .map(|o| o.values().collect::<Vec<_>>())
                .unwrap_or_default();
            let mut projects = HashMap::new();
            for (name, hash, local) in &items {
                let v = &known[hash];
                if v.is_null() {
                    if local.is_empty() || local.iter().all(|m| m.id.is_empty()) {
                        issue(
                            &mut issues,
                            "warning",
                            name,
                            "Нет полных метаданных: совместимость не подтверждена".into(),
                        );
                    }
                    continue;
                }
                if let Some(previous) = projects.insert(s(v, "project_id"), name) {
                    issue(
                        &mut issues,
                        "error",
                        name,
                        format!("Повтор проекта Modrinth: {previous}"),
                    );
                }
                if !list(v, "game_versions")
                    .iter()
                    .any(|x| x.as_str() == Some(s(inst, "version")))
                {
                    issue(
                        &mut issues,
                        "error",
                        name,
                        "На Modrinth эта версия мода не поддерживает выбранный Minecraft".into(),
                    );
                }
                let loaders = list(v, "loaders");
                if !loaders.iter().any(|x| {
                    x.as_str() == Some(&s(inst, "loader").to_lowercase())
                        || (s(inst, "loader") == "Quilt" && x.as_str() == Some("fabric"))
                }) {
                    issue(
                        &mut issues,
                        "error",
                        name,
                        "На Modrinth указан другой загрузчик".into(),
                    );
                }
                for d in list(v, "dependencies") {
                    let exists = (!s(d, "project_id").is_empty() || !s(d, "version_id").is_empty())
                        && versions.iter().any(|x| {
                            (s(d, "project_id").is_empty()
                                || s(x, "project_id") == s(d, "project_id"))
                                && (s(d, "version_id").is_empty()
                                    || s(x, "id") == s(d, "version_id"))
                        });
                    if s(d, "dependency_type") == "required" && !exists {
                        issue(&mut issues,"warning",name,format!("Не подтверждена зависимость Modrinth {}. Возможно, она установлена локально под другим хешем.",if s(d,"project_id").is_empty(){s(d,"version_id")}else{s(d,"project_id")}));
                    }
                    if s(d, "dependency_type") == "incompatible" && exists {
                        issue(
                            &mut issues,
                            "error",
                            name,
                            "Заявлен конфликт с проектом Modrinth".into(),
                        );
                    }
                }
            }
        }
    }
    let errors = issues
        .iter()
        .filter(|i| s(i, "severity") == "error")
        .count();
    Ok(
        json!({"checked":checked,"disabled":disabled,"online":online,"errors":errors,"warnings":issues.len()-errors,"issues":issues}),
    )
}
