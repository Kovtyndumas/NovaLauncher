use crate::{backups, hardware, runtime, store::Store, util::*};
use anyhow::Result;
use serde_json::{json, Value};
use std::{fs, path::Path, process::Command};

pub fn disk_free(path: &Path) -> Option<u64> {
    #[cfg(unix)]
    {
        let output = Command::new("df").arg("-Pk").arg(path).output().ok()?;
        if !output.status.success() {
            return None;
        }
        let text = String::from_utf8_lossy(&output.stdout);
        text.lines()
            .last()?
            .split_whitespace()
            .nth(3)?
            .parse::<u64>()
            .ok()?
            .checked_mul(1024)
    }
    #[cfg(windows)]
    {
        let output = Command::new("powershell")
            .args([
                "-NoProfile",
                "-NonInteractive",
                "-Command",
                "(Get-Item -LiteralPath $env:NOVA_CHECK_PATH).PSDrive.Free",
            ])
            .env("NOVA_CHECK_PATH", path)
            .output()
            .ok()?;
        if !output.status.success() {
            return None;
        }
        String::from_utf8_lossy(&output.stdout).trim().parse().ok()
    }
}
pub fn recommended_ram(total: u64) -> u64 {
    if total == 0 {
        4
    } else {
        (total / 2).clamp(2, 8)
    }
}
pub fn preflight(store: &Store, inst: &Value) -> Result<Value> {
    let settings = store.game_settings(inst);
    let mut checks = vec![];
    let root = store.dir(s(inst, "id"))?;
    let info = hardware::info();
    let total = number(&info, "ramGiB", 0);
    let ram = number(&settings, "ram", 4);
    let mut add = |key: &str, status: &str, title: &str, detail: String, fix: &str| {
        checks.push(json!({"key":key,"status":status,"title":title,"detail":detail,"fix":fix}))
    };
    if s(inst, "loader") != "Rust" {
        add(
            "memory",
            if total > 0 && ram >= total {
                "error"
            } else if total > 0 && ram > total.saturating_sub(2) {
                "warning"
            } else {
                "ok"
            },
            "Оперативная память",
            if total > 0 {
                format!("Игре: {ram} ГБ · компьютеру доступно всего: {total} ГБ")
            } else {
                format!("Игре: {ram} ГБ · общий объём определить не удалось")
            },
            "memory",
        );
        let java = s(&settings, "java");
        if !java.is_empty() {
            let actual = runtime::major(Path::new(java));
            let cached = read_json(
                &store
                    .root
                    .join("minecraft/versions")
                    .join(s(inst, "version"))
                    .join(format!("{}.json", s(inst, "version"))),
            )
            .ok();
            let expected = cached
                .as_ref()
                .and_then(|v| v["javaVersion"]["majorVersion"].as_u64());
            add(
                "java",
                if actual == 0 || expected.is_some_and(|v| v != actual as u64) {
                    "error"
                } else {
                    "ok"
                },
                "Своя Java",
                format!(
                    "Java {actual}{}",
                    expected
                        .map(|v| format!(" · игре требуется {v}"))
                        .unwrap_or_else(
                            || " · точное требование проверится при подготовке игры".into()
                        )
                ),
                "java",
            );
        } else {
            add(
                "java",
                if flag(&settings, "autoJava", true) {
                    "ok"
                } else {
                    "warning"
                },
                "Среда Java",
                if flag(&settings, "autoJava", true) {
                    "Автоматический подбор нужной версии".into()
                } else {
                    "Автозагрузка выключена: нужна установленная совместимая Java".into()
                },
                "java",
            );
        }
    }
    match disk_free(&root) {
        Some(free) => add(
            "disk",
            if free < 512 * 1024 * 1024 {
                "error"
            } else if free < 5 * 1024 * 1024 * 1024 {
                "warning"
            } else {
                "ok"
            },
            "Свободное место",
            format!(
                "{:.1} ГБ на диске сборки · установка модпаков может потребовать больше",
                free as f64 / 1073741824.0
            ),
            "folder",
        ),
        None => add(
            "disk",
            "warning",
            "Свободное место",
            "Не удалось определить свободное место на диске".into(),
            "folder",
        ),
    }
    add(
        "worlds",
        "ok",
        "Копии миров",
        if flag(&settings, "autoWorldBackup", true) {
            "Автокопия перед изменением контента и раз в сутки перед игрой".into()
        } else {
            "Автоматические копии выключены".into()
        },
        "worlds",
    );
    let errors = checks.iter().filter(|v| v["status"] == "error").count();
    Ok(
        json!({"checks":checks,"errors":errors,"recommendedRam":recommended_ram(total),"instance":s(inst,"id")}),
    )
}
pub fn tree_size(path: &Path) -> Result<u64> {
    if path.is_symlink() {
        return Ok(0);
    }
    if path.is_file() {
        return Ok(fs::metadata(path)?.len());
    }
    let mut bytes = 0u64;
    if path.is_dir() {
        for entry in fs::read_dir(path)? {
            bytes = bytes.saturating_add(tree_size(&entry?.path())?);
        }
    }
    Ok(bytes)
}
pub fn auto_worlds(store: &Store, id: &str, force: bool) -> Result<Option<Value>> {
    let settings = store.settings();
    if !flag(&settings, "autoWorldBackup", true) {
        return Ok(None);
    }
    let inst = store.get(id)?;
    if s(&inst, "loader") == "Rust" {
        return Ok(None);
    }
    let root = store.dir(id)?;
    if backups::names(&root.join("saves")).is_empty() {
        return Ok(None);
    }
    let all = backups::list_backups(&root, "worlds")?;
    if !force
        && all.as_array().unwrap().iter().any(|v| {
            flag(v, "automatic", false)
                && chrono::DateTime::parse_from_rfc3339(s(v, "at"))
                    .map(|at| {
                        (chrono::Utc::now() - at.with_timezone(&chrono::Utc)).num_hours() < 24
                    })
                    .unwrap_or(false)
        })
    {
        return Ok(None);
    }
    let needed = tree_size(&root.join("saves"))?;
    if disk_free(&root).is_some_and(|free| free < needed.saturating_add(512 * 1024 * 1024)) {
        anyhow::bail!("Недостаточно места для резервной копии миров. Освободите место или измените настройки копирования.");
    }
    let mut copy = backups::snapshot(
        &root,
        "worlds",
        if force {
            "Автокопия перед изменением сборки"
        } else {
            "Ежедневная автокопия перед игрой"
        },
    )?;
    copy["automatic"] = json!(true);
    copy["bytes"] = json!(needed);
    let dir = root.join("backups/worlds");
    write_json(&dir.join(s(&copy, "id")).join("manifest.json"), &copy)?;
    let keep = number(&settings, "worldBackupCount", 5).clamp(2, 20) as usize;
    let limit = number(&settings, "worldBackupGiB", 4).clamp(1, 64) * 1073741824;
    let list = backups::list_backups(&root, "worlds")?;
    let copies: Vec<&Value> = list
        .as_array()
        .unwrap()
        .iter()
        .filter(|v| flag(v, "automatic", false))
        .collect();
    let mut bytes = 0u64;
    for (index, item) in copies.iter().enumerate() {
        bytes = bytes.saturating_add(number(item, "bytes", 0));
        // Keep the newest automatic copy even if one world alone exceeds the quota.
        if index > 0 && (index >= keep || bytes > limit) {
            fs::remove_dir_all(safe(&dir, leaf(s(item, "id"))?)?)?;
        }
    }
    Ok(Some(copy))
}
pub fn inventory(root: &Path) -> Result<Value> {
    let mut files = json!({});
    for category in ["mods", "resourcepacks", "shaderpacks"] {
        let dir = root.join(category);
        if !dir.is_dir() {
            continue;
        }
        for entry in fs::read_dir(&dir)? {
            let entry = entry?;
            if entry.file_type()?.is_file() {
                files[format!("{category}/{}", entry.file_name().to_string_lossy())] =
                    json!(sha(&entry.path(), "sha256")?);
            }
        }
    }
    Ok(files)
}
pub fn changes(before: &Value, after: &Value) -> Value {
    let mut result = vec![];
    if let Some(map) = after.as_object() {
        for (name, hash) in map {
            if before.get(name) != Some(hash) {
                result.push(json!({"name":name,"kind":if before.get(name).is_some(){"updated"}else{"added"}}));
            }
        }
    }
    if let Some(map) = before.as_object() {
        for name in map.keys() {
            if after.get(name).is_none() {
                result.push(json!({"name":name,"kind":"removed"}));
            }
        }
    }
    json!(result)
}
pub fn history(root: &Path) -> Result<Value> {
    let path = root.join(".nova-changes.json");
    if path.exists() {
        let value = read_json(&path)?;
        if !value.is_array() {
            anyhow::bail!("Повреждена история изменений");
        }
        Ok(value)
    } else {
        Ok(json!([]))
    }
}
pub fn record(
    root: &Path,
    label: &str,
    before: &Value,
    error: Option<&str>,
    prior_backup: &str,
) -> Result<()> {
    let after = inventory(root)?;
    let mut journal = history(root)?;
    let backups = backups::list_backups(root, "mods")?;
    let latest = backups
        .as_array()
        .and_then(|items| items.first())
        .filter(|v| error.is_none() && s(v, "id") != prior_backup);
    let entries = journal.as_array_mut().unwrap();
    entries.insert(0,json!({"id":uuid::Uuid::new_v4().to_string(),"at":now(),"label":label,"changes":changes(before,&after),"error":error,"backup":latest.map(|v|s(v,"id"))}));
    entries.truncate(200);
    write_json(&root.join(".nova-changes.json"), &journal)
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn diff_includes_additions_removals_and_updates() {
        let d = changes(
            &json!({"a":"old","b":"same","c":"gone"}),
            &json!({"a":"new","b":"same","d":"new"}),
        );
        assert_eq!(d.as_array().unwrap().len(), 3);
        assert!(d
            .as_array()
            .unwrap()
            .iter()
            .any(|v| v["name"] == "a" && v["kind"] == "updated"));
    }
    #[test]
    fn auto_copies_keep_manual_and_skip_empty_and_daily_duplicates() {
        let temp = tempfile::tempdir().unwrap();
        let store = Store::new(temp.path().into()).unwrap();
        store
            .add(json!({"id":"a","name":"A","version":"1.21.1","loader":"Fabric"}))
            .unwrap();
        assert!(auto_worlds(&store, "a", false).unwrap().is_none());
        let root = store.dir("a").unwrap();
        fs::create_dir_all(root.join("saves/World")).unwrap();
        fs::write(root.join("saves/World/level.dat"), b"world").unwrap();
        let manual = backups::snapshot(&root, "worlds", "Ручная копия").unwrap();
        store
            .edit(|db| {
                db["settings"]["worldBackupCount"] = json!(2);
                Ok(())
            })
            .unwrap();
        auto_worlds(&store, "a", false).unwrap();
        assert!(auto_worlds(&store, "a", false).unwrap().is_none());
        auto_worlds(&store, "a", true).unwrap();
        auto_worlds(&store, "a", true).unwrap();
        let copies = backups::list_backups(&root, "worlds").unwrap();
        assert_eq!(copies.as_array().unwrap().len(), 3);
        assert!(root.join("backups/worlds").join(s(&manual, "id")).exists());
    }
    #[test]
    fn ram_recommendations_leave_room_for_system() {
        assert_eq!(recommended_ram(8), 4);
        assert_eq!(recommended_ram(16), 8);
        assert_eq!(recommended_ram(64), 8);
    }
}
