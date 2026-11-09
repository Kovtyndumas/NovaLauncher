use crate::util::*;
use anyhow::{bail, Result};
use serde_json::{json, Value};
use std::{
    fs,
    path::{Path, PathBuf},
    sync::Mutex,
};
pub struct Store {
    pub root: PathBuf,
    db: Mutex<Value>,
}
pub fn app_data(name: &str) -> PathBuf {
    #[cfg(target_os = "macos")]
    return dirs::home_dir()
        .unwrap()
        .join("Library/Application Support")
        .join(name);
    #[cfg(not(target_os = "macos"))]
    return dirs::config_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join(name);
}
impl Store {
    pub fn new(root: PathBuf) -> Result<Self> {
        fs::create_dir_all(&root)?;
        let db = if root.join("db.json").exists() {
            read_json(&root.join("db.json"))?
        } else {
            json!({"instances":[],"servers":[],"settings":{"name":"Steve","ram":4,"theme":"black","snow":false}})
        };
        validate(&db)?;
        Ok(Self {
            root,
            db: Mutex::new(db),
        })
    }
    pub fn db(&self) -> Value {
        self.db.lock().unwrap().clone()
    }
    pub fn public(&self) -> Value {
        let mut db = self.db();
        if let Some(s) = db["settings"].as_object_mut() {
            s.remove("msRefresh");
        }
        db
    }
    pub fn settings(&self) -> Value {
        self.db()["settings"].clone()
    }
    pub fn edit<F>(&self, f: F) -> Result<Value>
    where
        F: FnOnce(&mut Value) -> Result<()>,
    {
        let mut db = self.db.lock().unwrap();
        let mut next = db.clone();
        f(&mut next)?;
        validate(&next)?;
        write_json(&self.root.join("db.json"), &next)?;
        *db = next;
        Ok(db.clone())
    }
    pub fn get(&self, id: &str) -> Result<Value> {
        self.db()["instances"]
            .as_array()
            .unwrap()
            .iter()
            .find(|i| s(i, "id") == id)
            .cloned()
            .ok_or_else(|| anyhow::anyhow!("Сборка не найдена"))
    }
    pub fn dir(&self, id: &str) -> Result<PathBuf> {
        let _ = self.get(id)?;
        let p = safe(&self.root.join("instances"), id)?;
        fs::create_dir_all(&p)?;
        Ok(p)
    }
    pub fn add(&self, i: Value) -> Result<Value> {
        self.edit(|db| {
            db["instances"].as_array_mut().unwrap().push(i);
            Ok(())
        })
    }
    pub fn merge(&self, next: Value, base: &Value) -> Result<()> {
        validate(&next)?;
        self.edit(|db| {
            if base.is_null() {
                let secret = db["settings"]["msRefresh"].clone();
                *db = next.clone();
                if !secret.is_null() {
                    db["settings"]["msRefresh"] = secret;
                }
                return Ok(());
            }
            for (k, v) in next["settings"].as_object().unwrap() {
                if k != "msRefresh" && base["settings"][k] != *v {
                    db["settings"][k] = v.clone();
                }
            }
            let old = list(base, "instances");
            let incoming = list(&next, "instances");
            let current = db["instances"].as_array_mut().unwrap();
            current.retain(|i| {
                !old.iter().any(|o| s(o, "id") == s(i, "id"))
                    || incoming.iter().any(|n| s(n, "id") == s(i, "id"))
            });
            for item in incoming {
                let id = s(item, "id");
                if let Some(index) = current.iter().position(|i| s(i, "id") == id) {
                    let before = old.iter().find(|i| s(i, "id") == id);
                    if before != Some(item) {
                        let lv = current[index]["loaderVersion"].clone();
                        let same_loader = s(&current[index], "version") == s(item, "version")
                            && s(&current[index], "loader") == s(item, "loader");
                        current[index] = item.clone();
                        if same_loader && !lv.is_null() && item["loaderVersion"].is_null() {
                            current[index]["loaderVersion"] = lv;
                        }
                    }
                } else if !old.iter().any(|i| s(i, "id") == id) {
                    current.push(item.clone());
                }
            }
            if base["servers"] != next["servers"] {
                db["servers"] = next["servers"].clone();
            }
            Ok(())
        })?;
        Ok(())
    }
    pub fn loader_version(&self, id: &str, version: &str) -> Result<()> {
        self.edit(|db| {
            for i in db["instances"].as_array_mut().unwrap() {
                if s(i, "id") == id {
                    i["loaderVersion"] = json!(version);
                }
            }
            Ok(())
        })?;
        Ok(())
    }
    pub fn duplicate(&self, id: &str) -> Result<Value> {
        let mut i = self.get(id)?;
        let source = self.dir(id)?;
        i["id"] = json!(uuid::Uuid::new_v4().to_string());
        i["name"] = json!(format!("{} — копия", s(&i, "name")));
        let target = self.root.join("instances").join(s(&i, "id"));
        copy_tree(
            &source,
            &target,
            &[
                "backups",
                ".nova-backups",
                "libraries",
                "assets",
                "versions",
                "natives",
                "logs",
                "crash-reports",
            ],
        )?;
        if let Err(e) = self.add(i.clone()) {
            let _ = fs::remove_dir_all(target);
            return Err(e);
        }
        Ok(json!({"instance":i,"db":self.public()}))
    }
    pub fn remove(&self, id: &str) -> Result<Value> {
        let source = self.dir(id)?;
        let recovery = self.root.join("deleted-instances").join(format!(
            "{}-{}",
            id,
            chrono::Utc::now().timestamp_millis()
        ));
        fs::create_dir_all(recovery.parent().unwrap())?;
        fs::rename(&source, &recovery)?;
        if let Err(e) = self.edit(|db| {
            db["instances"]
                .as_array_mut()
                .unwrap()
                .retain(|i| s(i, "id") != id);
            Ok(())
        }) {
            let _ = fs::rename(&recovery, source);
            return Err(e);
        }
        Ok(json!({"db":self.public(),"retained":true,"recoveryPath":recovery}))
    }
    pub fn import(&self, root: &Path) -> Result<usize> {
        let db = read_json(&root.join("db.json"))?;
        validate(&db)?;
        let fresh = list(&self.db(), "instances").is_empty();
        let mut count = 0;
        for item in list(&db, "instances") {
            let mut i = item.clone();
            let old = s(&i, "id").to_string();
            i["id"] = json!(uuid::Uuid::new_v4().to_string());
            let dest = self.root.join("instances").join(s(&i, "id"));
            copy_tree(
                &safe(&root.join("instances"), &old)?,
                &dest,
                &[
                    "libraries",
                    "assets",
                    "versions",
                    "natives",
                    "logs",
                    "crash-reports",
                ],
            )?;
            self.add(i)?;
            count += 1;
        }
        self.edit(|current| {
            if list(current, "servers").is_empty() {
                current["servers"] = db.get("servers").cloned().unwrap_or(json!([]));
            }
            if fresh {
                for (k, v) in db["settings"].as_object().unwrap() {
                    if !["msRefresh", "ms", "onboarded", "introSeen", "updateFeed"]
                        .contains(&k.as_str())
                    {
                        current["settings"][k] = v.clone();
                    }
                }
                current["settings"]["ms"] = json!(false);
            }
            Ok(())
        })?;
        Ok(count)
    }
}
pub fn validate(db: &Value) -> Result<()> {
    let Some(items) = db["instances"].as_array() else {
        bail!("Повреждён список сборок");
    };
    if !db["settings"].is_object() {
        bail!("Повреждены настройки");
    }
    let mut ids = std::collections::HashSet::new();
    for i in items {
        let id = s(i, "id");
        if id.is_empty()
            || !id
                .chars()
                .all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-')
            || !ids.insert(id)
            || s(i, "name").is_empty()
            || s(i, "version").is_empty()
            || !s(i, "version")
                .chars()
                .all(|c| c.is_ascii_alphanumeric() || "._+-".contains(c))
            || !["Vanilla", "Fabric", "Quilt", "Forge", "NeoForge", "Rust"]
                .contains(&s(i, "loader"))
        {
            bail!("Некорректная сборка");
        }
    }
    Ok(())
}
