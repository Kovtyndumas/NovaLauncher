use crate::{backups, compatibility, net::Net, safety, store::Store, util::*};
use anyhow::{bail, Context, Result};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use std::{fs, path::Path};
const CONTENT: &[&str] = &[
    "mods",
    "config",
    "resourcepacks",
    "shaderpacks",
    "options.txt",
    "optionsshaders.txt",
    ".nova-content.json",
];
fn files(root: &Path, path: &Path, out: &mut Value) -> Result<()> {
    if path.is_symlink() {
        bail!(
            "Символическая ссылка в содержимом сборки: {}",
            path.display()
        );
    }
    if path.is_file() {
        out[path.strip_prefix(root)?.to_string_lossy().to_string()] = json!(sha(path, "sha256")?);
    } else if path.is_dir() {
        for entry in fs::read_dir(path)? {
            files(root, &entry?.path(), out)?;
        }
    }
    Ok(())
}
pub fn fingerprint(store: &Store, inst: &Value) -> Result<String> {
    let root = store.dir(s(inst, "id"))?;
    let mut data = json!({});
    for name in CONTENT {
        files(&root, &root.join(name), &mut data)?;
    }
    data["launch"] = json!({"version":inst["version"],"loader":inst["loader"],"loaderVersion":inst["loaderVersion"],"settings":store.game_settings(inst)});
    // Identity and authentication are not part of a content comparison.
    if let Some(settings) = data["launch"]["settings"].as_object_mut() {
        settings.retain(|key, _| crate::profiles::KEYS.contains(&key.as_str()));
    }
    Ok(format!("{:x}", Sha256::digest(serde_json::to_vec(&data)?)))
}
pub fn list(store: &Store, id: &str) -> Result<Value> {
    let path = store.dir(id)?.join(".nova-working-points.json");
    if path.exists() {
        let value = read_json(&path)?;
        if !value.is_array() {
            bail!("Повреждён список рабочих точек");
        }
        Ok(value)
    } else {
        Ok(json!([]))
    }
}
pub fn confirm(store: &Store, id: &str, label: &str) -> Result<Value> {
    let inst = store.get(id)?;
    if s(&inst, "loader") == "Rust" {
        bail!("Рабочие точки доступны для Java-сборок");
    }
    let root = store.dir(id)?;
    let backup = backups::snapshot(&root, "mods", "Рабочая точка, подтверждённая игроком")?;
    let entry = json!({"id":backup["id"],"at":now(),"label":if label.trim().is_empty(){"Всё работает"}else{label.trim()},"instance":inst,"fingerprint":fingerprint(store,&inst)?});
    let mut points = list(store, id)?;
    let entries = points.as_array_mut().unwrap();
    entries.insert(0, entry.clone());
    entries.truncate(100);
    write_json(&root.join(".nova-working-points.json"), &points)?;
    if inst["trial"].is_object() {
        write_json(
            &root.join(".nova-trial-confirmed.json"),
            &json!({"fingerprint":entry["fingerprint"],"at":now()}),
        )?;
    }
    Ok(entry)
}
pub fn restore(store: &Store, id: &str, point: &str) -> Result<Value> {
    let inst = store.get(id)?;
    let entries = list(store, id)?;
    let entry = entries
        .as_array()
        .unwrap()
        .iter()
        .find(|v| s(v, "id") == point)
        .context("Рабочая точка не найдена")?;
    let root = store.dir(id)?;
    safety::auto_worlds(store, id, true)?;
    let before = backups::snapshot(&root, "mods", "Перед возвратом рабочей точки")?;
    backups::restore(&root, "mods", point)?;
    let saved = store.edit(|db| {
        let current = db["instances"]
            .as_array_mut()
            .unwrap()
            .iter_mut()
            .find(|v| s(v, "id") == id)
            .context("Сборка удалена")?;
        for key in ["version", "loader", "loaderVersion", "launchSettings"] {
            if entry["instance"][key].is_null() {
                current.as_object_mut().unwrap().remove(key);
            } else {
                current[key] = entry["instance"][key].clone();
            }
        }
        Ok(())
    });
    if let Err(error) = saved {
        backups::restore(&root, "mods", s(&before, "id")).context(
            "Не удалось откатить файлы после ошибки сохранения; копии сохранены в backups/mods",
        )?;
        return Err(error);
    }
    let _ = inst;
    Ok(json!({"db":store.public(),"worldsPreserved":true}))
}
pub fn trial(store: &Store, id: &str) -> Result<Value> {
    let original = store.get(id)?;
    if s(&original, "loader") == "Rust" {
        bail!("Проверка обновлений в копии доступна для Java-сборок");
    }
    if original["trial"].is_object() {
        bail!("Создавайте проверочную копию исходной сборки");
    }
    let before = fingerprint(store, &original)?;
    let source = store.dir(id)?;
    let mut inst = original.clone();
    inst["id"] = json!(uuid::Uuid::new_v4().to_string());
    inst["name"] = json!(format!("{} — проверка обновления", s(&original, "name")));
    inst["trial"] = json!({"source":id,"sourceName":original["name"],"fingerprint":before,"at":now(),"promoted":false});
    let target = store.root.join("instances").join(s(&inst, "id"));
    let result = (|| {
        copy_tree(
            &source,
            &target,
            &[
                "backups",
                ".nova-downloads",
                ".nova-working-points.json",
                ".nova-trial-confirmed.json",
                ".nova-changes.json",
                ".nova-recovery.json",
                "libraries",
                "assets",
                "versions",
                "natives",
                "logs",
                "crash-reports",
                "session.lock",
            ],
        )?;
        if fingerprint(store, &original)? != before {
            bail!("Исходная сборка изменилась во время копирования. Повторите создание копии.");
        }
        store.add(inst.clone())?;
        Ok(json!({"instance":inst,"db":store.public()}))
    })();
    if result.is_err() {
        let _ = fs::remove_dir_all(target);
    }
    result
}
pub fn promote(store: &Store, net: &Net, id: &str) -> Result<Value> {
    let inst = store.get(id)?;
    let trial = &inst["trial"];
    let source = s(trial, "source");
    if source.is_empty() || flag(trial, "promoted", false) {
        bail!("Это не активная проверочная копия");
    }
    let original = store.get(source)?;
    let original_root = store.dir(source)?;
    let root = store.dir(id)?;
    let confirmation = read_json(&root.join(".nova-trial-confirmed.json"))
        .context("Сначала подтвердите, что проверочная копия работает в игре")?;
    if s(&confirmation, "fingerprint") != fingerprint(store, &inst)? {
        bail!("Копия изменилась после подтверждения. Проверьте её ещё раз.");
    }
    if s(trial, "fingerprint") != fingerprint(store, &original)? {
        bail!("Исходная сборка изменилась. Создайте новую проверочную копию, чтобы сохранить изменения.");
    }
    let report = compatibility::check(&root, net, &inst, false)?;
    if number(&report, "errors", 0) > 0 {
        bail!("В копии остались известные ошибки зависимостей. Исправьте их перед переносом.");
    }
    safety::auto_worlds(store, source, true)?;
    let before = backups::snapshot(
        &original_root,
        "mods",
        "Перед переносом проверенного обновления",
    )?;
    let tested = backups::snapshot(&root, "mods", "Проверенное обновление")?;
    let copy = original_root
        .join("backups/mods")
        .join(leaf(s(&tested, "id"))?);
    copy_tree(
        &root.join("backups/mods").join(s(&tested, "id")),
        &copy,
        &[],
    )?;
    backups::restore(&original_root, "mods", s(&tested, "id"))?;
    if let Err(error) = store.edit(|db| {
        for current in db["instances"].as_array_mut().unwrap() {
            if s(current, "id") == source {
                if inst["launchSettings"].is_null() {
                    current.as_object_mut().unwrap().remove("launchSettings");
                } else {
                    current["launchSettings"] = inst["launchSettings"].clone();
                }
            }
            if s(current, "id") == id {
                current["trial"]["promoted"] = json!(true);
            }
        }
        Ok(())
    }) {
        backups::restore(&original_root, "mods", s(&before, "id"))?;
        return Err(error);
    }
    Ok(json!({"db":store.public(),"source":source,"worldsPreserved":true,"backup":before["id"]}))
}
#[cfg(test)]
mod tests {
    use super::*;
    fn setup() -> (tempfile::TempDir, Store) {
        let t = tempfile::tempdir().unwrap();
        let store = Store::new(t.path().into()).unwrap();
        store
            .add(json!({"id":"main","name":"Main","version":"1.21.1","loader":"Vanilla"}))
            .unwrap();
        let root = store.dir("main").unwrap();
        fs::create_dir_all(root.join("config")).unwrap();
        fs::create_dir_all(root.join("saves/World")).unwrap();
        fs::write(root.join("config/settings.txt"), b"old").unwrap();
        fs::write(root.join("saves/World/level.dat"), b"world").unwrap();
        (t, store)
    }
    #[test]
    fn confirmed_update_preserves_original_worlds_and_restores_content() {
        let (_t, store) = setup();
        let copy = trial(&store, "main").unwrap();
        let id = s(&copy["instance"], "id");
        let root = store.dir(id).unwrap();
        fs::write(root.join("config/settings.txt"), b"new").unwrap();
        fs::write(root.join("saves/World/level.dat"), b"test world").unwrap();
        assert!(promote(&store, &Net::new().unwrap(), id).is_err());
        confirm(&store, id, "Tested").unwrap();
        promote(&store, &Net::new().unwrap(), id).unwrap();
        let original = store.dir("main").unwrap();
        assert_eq!(
            fs::read(original.join("config/settings.txt")).unwrap(),
            b"new"
        );
        assert_eq!(
            fs::read(original.join("saves/World/level.dat")).unwrap(),
            b"world"
        );
    }
    #[test]
    fn refuses_stale_source_and_changed_trial() {
        let (_t, store) = setup();
        let copy = trial(&store, "main").unwrap();
        let id = s(&copy["instance"], "id");
        confirm(&store, id, "").unwrap();
        fs::write(
            store.dir("main").unwrap().join("config/settings.txt"),
            b"external change",
        )
        .unwrap();
        assert!(promote(&store, &Net::new().unwrap(), id).is_err());
        assert_eq!(
            fs::read(store.dir("main").unwrap().join("config/settings.txt")).unwrap(),
            b"external change"
        );
    }
    #[test]
    fn working_point_restores_settings_without_touching_world() {
        let (_t, store) = setup();
        let point = confirm(&store, "main", "Good").unwrap();
        store
            .edit(|db| {
                db["instances"][0]["launchSettings"] = json!({"ram":8});
                Ok(())
            })
            .unwrap();
        fs::write(
            store.dir("main").unwrap().join("config/settings.txt"),
            b"changed",
        )
        .unwrap();
        restore(&store, "main", s(&point, "id")).unwrap();
        assert!(store.get("main").unwrap()["launchSettings"].is_null());
        assert_eq!(
            fs::read(store.dir("main").unwrap().join("config/settings.txt")).unwrap(),
            b"old"
        );
    }
}
