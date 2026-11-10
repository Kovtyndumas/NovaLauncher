use crate::{backups, util::*};
use anyhow::{bail, Context, Result};
use serde_json::{json, Value};
use std::{fs, path::Path};
fn manifest(dir: &Path) -> Result<Value> {
    let mut result = json!({});
    if dir.is_dir() {
        for e in fs::read_dir(dir)? {
            let e = e?;
            let name = e.file_name().to_string_lossy().to_string();
            if name.ends_with(".jar") && e.file_type()?.is_file() {
                result[&name] = json!(sha(&e.path(), "sha512")?);
            }
        }
    }
    Ok(result)
}
pub fn mark_good(root: &Path) -> Result<()> {
    write_json(
        &root.join(".nova-good-mods.json"),
        &json!({"at":now(),"mods":manifest(&root.join("mods"))?}),
    )
}
pub fn plan(root: &Path) -> Result<Value> {
    let good = read_json(&root.join(".nova-good-mods.json")).ok();
    let (baseline, basis) = if let Some(v) = good.filter(|v| v["mods"].is_object()) {
        (v["mods"].clone(), "Последний успешный запуск")
    } else {
        let copies = backups::list_backups(root, "mods")?;
        let candidate = list(&json!({"items":copies}), "items")
            .iter()
            .find(|v| {
                let reason = s(v, "reason");
                !flag(v, "restored", false)
                    && !flag(v, "legacy", false)
                    && (reason.contains("установ")
                        || reason.contains("обновлен")
                        || reason.contains("обновлени"))
            })
            .cloned();
        if let Some(v) = candidate {
            (
                manifest(
                    &root
                        .join("backups/mods")
                        .join(leaf(s(&v, "id"))?)
                        .join("files/mods"),
                )?,
                "Копия перед последней установкой или обновлением",
            )
        } else {
            (Value::Null, "Нет сохранённой точки сравнения")
        }
    };
    let current = manifest(&root.join("mods"))?;
    let mut mods = vec![];
    if baseline.is_object() {
        for (name, hash) in current.as_object().unwrap() {
            if baseline[name] != *hash {
                mods.push(json!({"name":name,"hash":hash,"change":if baseline.get(name).is_some(){"updated"}else{"added"}}));
            }
        }
    }
    let journal = read_json(&root.join(".nova-recovery.json")).unwrap_or(Value::Null);
    Ok(json!({"supported":true,"basis":basis,"mods":mods,"undo":journal["active"]==true}))
}
fn rename_all(root: &Path, items: &[Value], undo: bool) -> Result<()> {
    let mut changed: Vec<(std::path::PathBuf, std::path::PathBuf)> = vec![];
    for item in items {
        let name = leaf(s(item, "name"))?;
        if !name.ends_with(".jar") {
            bail!("Некорректный мод");
        }
        let normal = root.join("mods").join(name);
        let disabled = root.join("mods").join(format!("{name}.disabled"));
        let (from, to) = if undo {
            (disabled, normal)
        } else {
            (normal, disabled)
        };
        if undo && !from.exists() && to.is_file() && sha(&to, "sha512")? == s(item, "hash") {
            continue;
        }
        if to.exists() || !from.is_file() || sha(&from, "sha512")? != s(item, "hash") {
            bail!("Файл изменился. Обновите список: {name}");
        }
    }
    for item in items {
        let normal = root.join("mods").join(s(item, "name"));
        let disabled = root
            .join("mods")
            .join(format!("{}.disabled", s(item, "name")));
        let (from, to) = if undo {
            (disabled, normal)
        } else {
            (normal, disabled)
        };
        if undo && !from.exists() {
            continue;
        }
        if let Err(error) = fs::rename(&from, &to) {
            for (from, to) in changed.iter().rev() {
                let _ = fs::rename(to, from);
            }
            return Err(error.into());
        }
        changed.push((from, to));
    }
    Ok(())
}
pub fn apply(root: &Path, selection: &Value) -> Result<Value> {
    if read_json(&root.join(".nova-recovery.json")).unwrap_or(Value::Null)["active"] == true {
        bail!("Сначала верните моды предыдущего восстановления");
    }
    let plan = plan(root)?;
    let mut chosen = vec![];
    for item in selection.as_array().context("Выберите моды")? {
        let known = list(&plan, "mods")
            .iter()
            .find(|v| v["name"] == item["name"] && v["hash"] == item["hash"])
            .context("Список устарел. Повторите проверку")?;
        if !chosen.contains(known) {
            chosen.push(known.clone());
        }
    }
    if chosen.is_empty() {
        bail!("Выберите хотя бы один мод");
    }
    let backup = backups::snapshot(root, "mods", "Перед безопасным запуском")?;
    // Persist the undo journal before touching files. A failed rename clears it.
    write_json(
        &root.join(".nova-recovery.json"),
        &json!({"active":true,"at":now(),"mods":chosen,"backup":backup["id"]}),
    )?;
    if let Err(error) = rename_all(root, &chosen, false) {
        let _ = fs::remove_file(root.join(".nova-recovery.json"));
        return Err(error);
    }
    plan_result(root, chosen.len())
}
fn plan_result(root: &Path, count: usize) -> Result<Value> {
    let mut value = plan(root)?;
    value["count"] = json!(count);
    Ok(value)
}
pub fn undo(root: &Path) -> Result<Value> {
    let mut journal = read_json(&root.join(".nova-recovery.json"))?;
    if journal["active"] != true {
        bail!("Нет отключённых модов для возврата");
    }
    let items = list(&journal, "mods").to_vec();
    rename_all(root, &items, true)?;
    journal["active"] = json!(false);
    write_json(&root.join(".nova-recovery.json"), &journal)?;
    plan_result(root, items.len())
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn recovery_detects_changes_and_undo_preserves_other_files() {
        let t = tempfile::tempdir().unwrap();
        let root = t.path();
        fs::create_dir(root.join("mods")).unwrap();
        fs::write(root.join("mods/old.jar"), b"old").unwrap();
        mark_good(root).unwrap();
        fs::write(root.join("mods/new.jar"), b"new").unwrap();
        let p = plan(root).unwrap();
        assert_eq!(list(&p, "mods").len(), 1);
        apply(root, &p["mods"]).unwrap();
        assert!(root.join("mods/new.jar.disabled").exists());
        fs::write(root.join("mods/unrelated.jar"), b"other").unwrap();
        undo(root).unwrap();
        assert!(root.join("mods/new.jar").exists());
        assert!(root.join("mods/unrelated.jar").exists());
    }
    #[test]
    fn rejects_stale_or_forged_selection_without_mutation() {
        let t = tempfile::tempdir().unwrap();
        let root = t.path();
        fs::create_dir(root.join("mods")).unwrap();
        mark_good(root).unwrap();
        fs::write(root.join("mods/a.jar"), b"one").unwrap();
        let p = plan(root).unwrap();
        fs::write(root.join("mods/a.jar"), b"two").unwrap();
        assert!(apply(root, &p["mods"]).is_err());
        assert!(root.join("mods/a.jar").exists());
        assert!(apply(root, &json!([{"name":"../bad.jar","hash":"bad"}])).is_err());
    }
    #[test]
    fn empty_backup_is_valid_baseline() {
        let t = tempfile::tempdir().unwrap();
        backups::snapshot(t.path(), "mods", "Перед установкой контента").unwrap();
        fs::create_dir(t.path().join("mods")).unwrap();
        fs::write(t.path().join("mods/first.jar"), b"first").unwrap();
        assert_eq!(list(&plan(t.path()).unwrap(), "mods").len(), 1);
    }
}

#[cfg(test)]
mod extra_tests {
    use super::*;
    #[test]
    fn undo_refuses_overwriting_new_versions() {
        let t = tempfile::tempdir().unwrap();
        let root = t.path();
        fs::create_dir(root.join("mods")).unwrap();
        mark_good(root).unwrap();
        fs::write(root.join("mods/a.jar"), b"first").unwrap();
        let p = plan(root).unwrap();
        apply(root, &p["mods"]).unwrap();
        fs::write(root.join("mods/a.jar"), b"replacement").unwrap();
        assert!(undo(root).is_err());
        assert_eq!(fs::read(root.join("mods/a.jar")).unwrap(), b"replacement");
        assert!(root.join("mods/a.jar.disabled").exists());
    }
    #[test]
    fn undo_recovers_an_interrupted_rename_batch() {
        let t = tempfile::tempdir().unwrap();
        let root = t.path();
        fs::create_dir(root.join("mods")).unwrap();
        mark_good(root).unwrap();
        fs::write(root.join("mods/a.jar"), b"a").unwrap();
        fs::write(root.join("mods/b.jar"), b"b").unwrap();
        let p = plan(root).unwrap();
        apply(root, &p["mods"]).unwrap();
        fs::rename(root.join("mods/a.jar.disabled"), root.join("mods/a.jar")).unwrap();
        undo(root).unwrap();
        assert!(root.join("mods/a.jar").exists() && root.join("mods/b.jar").exists());
        assert_eq!(plan(root).unwrap()["undo"], false);
    }
}
