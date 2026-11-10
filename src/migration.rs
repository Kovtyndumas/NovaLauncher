use crate::{store::Store, util::*};
use anyhow::{bail, Context, Result};
use serde_json::{json, Value};
use std::{
    fs,
    path::{Path, PathBuf},
};
const CONTENT: &[&str] = &[
    "saves",
    "mods",
    "config",
    "defaultconfigs",
    "resourcepacks",
    "shaderpacks",
    "screenshots",
    "options.txt",
    "optionsshaders.txt",
    "servers.dat",
    "servers.dat_old",
];
fn candidate(path: &Path) -> Option<Value> {
    let pack = read_json(&path.join("mmc-pack.json")).ok()?;
    let components = list(&pack, "components");
    let mc = components.iter().find(|v| s(v, "uid") == "net.minecraft")?;
    let (loader, loader_version) = components
        .iter()
        .find_map(|v| {
            let name = match s(v, "uid") {
                "net.fabricmc.fabric-loader" => "Fabric",
                "org.quiltmc.quilt-loader" => "Quilt",
                "net.minecraftforge" => "Forge",
                "net.neoforged" => "NeoForge",
                _ => return None,
            };
            Some((name, s(v, "version")))
        })
        .unwrap_or(("Vanilla", ""));
    let cfg = fs::read_to_string(path.join("instance.cfg")).unwrap_or_default();
    let name = cfg
        .lines()
        .find_map(|l| l.strip_prefix("name="))
        .unwrap_or_else(|| {
            path.file_name()
                .and_then(|v| v.to_str())
                .unwrap_or("Импорт")
        });
    let game = if path.join(".minecraft").is_dir() {
        path.join(".minecraft")
    } else {
        path.join("minecraft")
    };
    if !game.is_dir() || game.is_symlink() {
        return None;
    }
    Some(
        json!({"id":uuid::Uuid::new_v4().to_string(),"name":name,"version":s(mc,"version"),"loader":loader,"loaderVersion":loader_version,"source":game,"format":"Prism / MultiMC"}),
    )
}
pub fn scan(path: &Path) -> Result<Value> {
    if !path.is_dir() || path.is_symlink() {
        bail!("Выберите обычную папку сборок или игры");
    }
    let mut candidates = vec![];
    if let Some(item) = candidate(path) {
        candidates.push(item);
    } else {
        let parent = if path.join("instances").is_dir() {
            path.join("instances")
        } else {
            path.to_path_buf()
        };
        for entry in fs::read_dir(parent)?.take(500) {
            let entry = entry?;
            if entry.file_type()?.is_dir() {
                if let Some(item) = candidate(&entry.path()) {
                    candidates.push(item);
                }
            }
        }
    }
    if candidates.is_empty() {
        let has_content = CONTENT.iter().any(|name| path.join(name).exists());
        if !has_content {
            bail!("Сборки не найдены. Выберите папку Prism/MultiMC или папку игры с saves, mods либо config.");
        }
        candidates.push(json!({"id":uuid::Uuid::new_v4().to_string(),"name":path.file_name().and_then(|v|v.to_str()).unwrap_or("Моя сборка"),"version":"","loader":"Vanilla","loaderVersion":"","source":path,"format":"Папка Minecraft"}));
    }
    Ok(json!({"token":uuid::Uuid::new_v4().to_string(),"items":candidates}))
}
pub fn import(store: &Store, plan: &Value, request: &Value) -> Result<Value> {
    if s(plan, "token").is_empty() || s(plan, "token") != s(request, "token") {
        bail!("Предпросмотр устарел. Выберите папку заново.");
    }
    let selected = request["items"].as_array().context("Выберите сборки")?;
    if selected.is_empty() || selected.len() > 100 {
        bail!("Выберите от 1 до 100 сборок");
    }
    let mut prepared = vec![];
    let mut ids = std::collections::HashSet::new();
    for selection in selected {
        let item = list(plan, "items")
            .iter()
            .find(|v| v["id"] == selection["id"])
            .context("Сборка отсутствует в предпросмотре")?;
        if !ids.insert(s(item, "id")) {
            bail!("Сборка выбрана дважды");
        }
        let version = if s(item, "version").is_empty() {
            s(selection, "version")
        } else {
            s(item, "version")
        };
        let loader = if s(item, "version").is_empty() {
            s(selection, "loader")
        } else {
            s(item, "loader")
        };
        let mut inst = json!({"id":uuid::Uuid::new_v4().to_string(),"name":s(item,"name"),"version":version,"loader":loader});
        if !s(item, "loaderVersion").is_empty() {
            inst["loaderVersion"] = item["loaderVersion"].clone();
        }
        crate::store::validate(&json!({"settings":{},"instances":[inst.clone()]}))?;
        prepared.push((inst, PathBuf::from(s(item, "source"))));
    }
    // Stage every copy before committing the database once. Originals are read only.
    let mut created = vec![];
    let result = (|| {
        for (inst, source) in &prepared {
            let target = store.root.join("instances").join(s(inst, "id"));
            created.push(target.clone());
            fs::create_dir_all(&target)?;
            for name in CONTENT {
                copy_tree(&source.join(name), &target.join(name), &[])?;
            }
        }
        store.edit(|db| {
            for (inst, _) in &prepared {
                db["instances"].as_array_mut().unwrap().push(inst.clone());
            }
            Ok(())
        })?;
        Ok(json!({"count":prepared.len(),"db":store.public()}))
    })();
    if result.is_err() {
        for dir in created {
            let _ = fs::remove_dir_all(dir);
        }
    }
    result
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn prism_import_is_previewed_and_does_not_copy_accounts() {
        let src = tempfile::tempdir().unwrap();
        let dst = tempfile::tempdir().unwrap();
        let path = src.path().join("instances/my-world");
        fs::create_dir_all(path.join(".minecraft/saves/My World")).unwrap();
        write_json(&path.join("mmc-pack.json"),&json!({"components":[{"uid":"net.minecraft","version":"1.21.1"},{"uid":"net.fabricmc.fabric-loader","version":"0.16.10"}]})).unwrap();
        fs::write(path.join("instance.cfg"), "name=My World\n").unwrap();
        fs::write(path.join(".minecraft/saves/My World/level.dat"), b"world").unwrap();
        fs::write(path.join(".minecraft/launcher_accounts.json"), b"secret").unwrap();
        let plan = scan(src.path()).unwrap();
        assert_eq!(plan["items"][0]["loader"], "Fabric");
        let store = Store::new(dst.path().into()).unwrap();
        let result = import(
            &store,
            &plan,
            &json!({"token":plan["token"],"items":[{"id":plan["items"][0]["id"]}]}),
        )
        .unwrap();
        let id = s(&result["db"]["instances"][0], "id").to_owned();
        let target = store.dir(&id).unwrap();
        assert!(target.join("saves/My World/level.dat").exists());
        assert!(!target.join("launcher_accounts.json").exists());
        assert!(path.join(".minecraft/saves/My World/level.dat").exists());
    }
    #[test]
    fn missing_version_and_forged_selection_do_not_create_instances() {
        let src = tempfile::tempdir().unwrap();
        fs::create_dir(src.path().join("mods")).unwrap();
        let dst = tempfile::tempdir().unwrap();
        let store = Store::new(dst.path().into()).unwrap();
        let plan = scan(src.path()).unwrap();
        assert!(import(
            &store,
            &plan,
            &json!({"token":plan["token"],"items":[{"id":plan["items"][0]["id"]}]})
        )
        .is_err());
        assert!(import(&store,&plan,&json!({"token":plan["token"],"items":[{"id":"fake","version":"1.21.1","loader":"Fabric"}]})).is_err());
        assert!(list(&store.db(), "instances").is_empty());
    }
}
