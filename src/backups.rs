use crate::util::*;
use anyhow::{bail, Result};
use serde_json::{json, Value};
use std::{fs, path::Path};
fn items(category: &str) -> Vec<&'static str> {
    if category == "worlds" {
        vec!["saves"]
    } else {
        vec![
            "mods",
            "resourcepacks",
            "shaderpacks",
            "config",
            "options.txt",
            "optionsshaders.txt",
            ".nova-content.json",
        ]
    }
}
pub fn snapshot(root: &Path, category: &str, reason: &str) -> Result<Value> {
    let id = format!(
        "{}-{}",
        chrono::Utc::now().timestamp_millis(),
        &uuid::Uuid::new_v4().to_string()[..8]
    );
    let dir = root.join("backups").join(category).join(&id);
    let result = (|| {
        for name in items(category) {
            copy_tree(&root.join(name), &dir.join("files").join(name), &[])?;
        }
        fs::create_dir_all(&dir)?;
        let mods = names(&root.join("mods"));
        let worlds = names(&root.join("saves"));
        let data = json!({"id":id,"at":now(),"reason":reason,"files":mods,"count":mods.len(),"worlds":worlds,"restored":false});
        write_json(&dir.join("manifest.json"), &data)?;
        Ok(data)
    })();
    if result.is_err() {
        let _ = fs::remove_dir_all(dir);
    }
    result
}
pub fn names(root: &Path) -> Vec<String> {
    fs::read_dir(root)
        .map(|e| {
            e.flatten()
                .map(|e| e.file_name().to_string_lossy().to_string())
                .collect()
        })
        .unwrap_or_default()
}
pub fn list_backups(root: &Path, category: &str) -> Result<Value> {
    let mut result = vec![];
    let dir = root.join("backups").join(category);
    if dir.exists() {
        for e in fs::read_dir(dir)? {
            let e = e?;
            let p = e.path();
            if let Ok(mut value) = read_json(&p.join("manifest.json")) {
                if value.is_object()
                    && (p.join("files").is_dir()
                        || (category == "worlds" && p.join("saves").is_dir()))
                {
                    value["id"] = json!(e.file_name().to_string_lossy());
                    result.push(value);
                } else if value.is_array() && category == "mods" {
                    let items = value.as_array().unwrap();
                    let at = fs::metadata(&p)?
                        .modified()
                        .map(chrono::DateTime::<chrono::Utc>::from)
                        .map(|t| t.to_rfc3339())
                        .unwrap_or_else(|_| now());
                    result.push(json!({"id":e.file_name().to_string_lossy(),"at":at,"count":items.len(),"files":items.iter().map(|i|s(i,"name")).collect::<Vec<_>>(),"legacy":true,"restored":p.join("restored.json").exists()}));
                }
            }
        }
    }
    result.sort_by(|a, b| s(b, "at").cmp(s(a, "at")));
    Ok(json!(result))
}
pub fn restore(root: &Path, category: &str, id: &str) -> Result<Value> {
    leaf(id)?;
    let dir = safe(&root.join("backups").join(category), id)?;
    let mut manifest = read_json(&dir.join("manifest.json"))?;
    if manifest.is_array() && category == "mods" {
        let entries = manifest.as_array().unwrap();
        for item in entries {
            let old = safe(&dir, leaf(s(item, "name"))?)?;
            if !old.is_file()
                || (!s(item, "beforeHash").is_empty()
                    && sha(&old, "sha512")? != s(item, "beforeHash"))
            {
                bail!("Повреждена старая копия мода");
            }
            let current = safe(&root.join("mods"), leaf(s(item, "to"))?)?;
            if current.is_file()
                && !s(item, "afterHash").is_empty()
                && sha(&current, "sha512")? != s(item, "afterHash")
            {
                bail!("Мод изменился после обновления: {}", s(item, "to"));
            }
        }
        snapshot(root, category, "Перед восстановлением")?;
        for item in entries {
            let current = safe(&root.join("mods"), s(item, "to"))?;
            if current.exists() {
                fs::remove_file(current)?;
            }
            copy_tree(
                &safe(&dir, s(item, "name"))?,
                &safe(&root.join("mods"), s(item, "name"))?,
                &[],
            )?;
        }
        write_json(&dir.join("restored.json"), &json!({"at":now()}))?;
        return Ok(json!({"count":entries.len()}));
    }
    let legacy_worlds = category == "worlds" && dir.join("saves").is_dir();
    if !dir.join("files").is_dir() && !legacy_worlds {
        bail!("Неподдерживаемая резервная копия");
    }
    snapshot(root, category, "Перед восстановлением")?;
    for name in items(category) {
        let target = root.join(name);
        if target.is_dir() {
            fs::remove_dir_all(&target)?;
        } else if target.exists() {
            fs::remove_file(&target)?;
        }
        let source = if legacy_worlds {
            dir.join(name)
        } else {
            dir.join("files").join(name)
        };
        copy_tree(&source, &target, &[])?;
    }
    manifest["restored"] = json!(true);
    write_json(&dir.join("manifest.json"), &manifest)?;
    Ok(json!({"count":number(&manifest,"count",0)}))
}
pub fn remove_mod(root: &Path, name: &str) -> Result<Value> {
    leaf(name)?;
    if !name.ends_with(".jar") && !name.ends_with(".jar.disabled") {
        bail!("Некорректный мод");
    }
    let source = safe(&root.join("mods"), name)?;
    if !source.is_file() {
        bail!("Мод не найден");
    }
    let id = format!(
        "{}-{}",
        chrono::Utc::now().timestamp_millis(),
        uuid::Uuid::new_v4()
    );
    let dir = root.join("backups/removed-mods").join(&id);
    fs::create_dir_all(&dir)?;
    fs::rename(&source, dir.join(name))?;
    let data = json!({"id":id,"name":name,"at":now(),"restored":false});
    if let Err(e) = write_json(&dir.join("manifest.json"), &data) {
        let _ = fs::rename(dir.join(name), source);
        return Err(e);
    }
    Ok(data)
}
pub fn removed(root: &Path) -> Result<Value> {
    let dir = root.join("backups/removed-mods");
    let mut items = vec![];
    if dir.exists() {
        for e in fs::read_dir(dir)? {
            let p = e?.path();
            if let Ok(mut data) = read_json(&p.join("manifest.json")) {
                data["id"] = json!(p.file_name().unwrap().to_string_lossy());
                data["restored"] =
                    json!(flag(&data, "restored", false) || p.join("restored.json").exists());
                items.push(data);
            }
        }
    }
    items.sort_by(|a, b| s(b, "at").cmp(s(a, "at")));
    Ok(json!(items))
}
pub fn restore_removed(root: &Path, id: &str) -> Result<Value> {
    leaf(id)?;
    let dir = safe(&root.join("backups/removed-mods"), id)?;
    let mut data = read_json(&dir.join("manifest.json"))?;
    let name = leaf(s(&data, "name"))?.to_owned();
    let target = safe(&root.join("mods"), &name)?;
    let alternate = if name.ends_with(".disabled") {
        target.with_file_name(name.trim_end_matches(".disabled"))
    } else {
        target.with_file_name(format!("{name}.disabled"))
    };
    if target.exists() || alternate.exists() {
        bail!("Мод уже есть в сборке");
    }
    fs::create_dir_all(target.parent().unwrap())?;
    fs::rename(dir.join(&name), target)?;
    data["restored"] = json!(true);
    write_json(&dir.join("manifest.json"), &data)?;
    Ok(data)
}
