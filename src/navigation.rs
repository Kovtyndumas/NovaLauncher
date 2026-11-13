use crate::util::*;
use anyhow::{bail, Result};
use serde_json::{json, Value};
pub fn is_rageland(host: &str) -> bool {
    let normalized = host.trim().to_ascii_lowercase();
    normalized
        .trim_end_matches('.')
        .trim_end_matches(":25565")
        .trim_end_matches('.')
        == "mc.rageland.live"
}
/// Keep personal servers, make the requested built-in server non-removable, and
/// repair only a missing/deleted selection. Existing valid selections always win.
pub fn normalize(db: &mut Value, history: Option<&Value>) -> Result<()> {
    if db["servers"].is_null() {
        db["servers"] = json!([]);
    }
    let servers = db["servers"]
        .as_array_mut()
        .ok_or_else(|| anyhow::anyhow!("Повреждён список серверов"))?;
    let mut found = false;
    for server in servers.iter_mut() {
        if is_rageland(s(server, "host")) {
            if !server.is_object() {
                bail!("Некорректный сервер");
            }
            server["builtin"] = json!("rageland");
            if s(server, "name").is_empty() {
                server["name"] = json!("RageLand");
            }
            found = true;
        }
    }
    if !found {
        servers.push(json!({"name":"RageLand","host":"mc.rageland.live","favorite":true,"builtin":"rageland"}));
    }
    let selected = s(&db["settings"], "selectedInstance");
    let exists =
        |id: &str| !id.is_empty() && list(db, "instances").iter().any(|inst| s(inst, "id") == id);
    if !exists(selected) {
        let recent = history.and_then(Value::as_array).and_then(|items| {
            items.iter().find_map(|entry| {
                let id = s(entry, "instanceId");
                exists(id).then(|| id.to_owned())
            })
        });
        let fallback = recent.or_else(|| {
            list(db, "instances")
                .first()
                .map(|inst| s(inst, "id").to_owned())
        });
        match fallback {
            Some(id) => db["settings"]["selectedInstance"] = json!(id),
            None => {
                db["settings"]
                    .as_object_mut()
                    .unwrap()
                    .remove("selectedInstance");
            }
        }
    }
    Ok(())
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn adds_rageland_without_replacing_personal_servers_and_does_not_duplicate() {
        let mut db = json!({"settings":{},"instances":[],"servers":[{"name":"My server","host":"my.example"},{"name":"My RageLand","host":"MC.RAGELAND.LIVE:25565"}]});
        normalize(&mut db, None).unwrap();
        assert_eq!(list(&db, "servers").len(), 2);
        assert_eq!(db["servers"][0]["host"], "my.example");
        assert_eq!(db["servers"][1]["name"], "My RageLand");
        assert_eq!(db["servers"][1]["builtin"], "rageland");
        db["servers"] = json!([{"name":"My server","host":"my.example"}]);
        normalize(&mut db, None).unwrap();
        assert_eq!(list(&db, "servers").len(), 2);
        assert_eq!(db["servers"][1]["host"], "mc.rageland.live");
    }
    #[test]
    fn keeps_selection_and_migrates_from_play_history_only_if_missing() {
        let mut db =
            json!({"settings":{},"instances":[{"id":"first"},{"id":"played"}],"servers":[]});
        let history = json!([{"instanceId":"deleted"},{"instanceId":"played"}]);
        normalize(&mut db, Some(&history)).unwrap();
        assert_eq!(db["settings"]["selectedInstance"], "played");
        db["settings"]["selectedInstance"] = json!("first");
        normalize(&mut db, Some(&history)).unwrap();
        assert_eq!(db["settings"]["selectedInstance"], "first");
        db["instances"] = json!([{"id":"played"}]);
        normalize(&mut db, None).unwrap();
        assert_eq!(db["settings"]["selectedInstance"], "played");
    }
}
