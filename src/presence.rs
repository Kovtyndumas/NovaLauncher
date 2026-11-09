use crate::util::*;
use discord_rich_presence::{activity, DiscordIpc, DiscordIpcClient};
use serde_json::{json, Value};
use std::sync::{Arc, Mutex};
pub struct Presence {
    pub desired: Mutex<Value>,
    pub status: Mutex<String>,
}
impl Presence {
    pub fn new() -> Arc<Self> {
        let this = Arc::new(Self {
            desired: Mutex::new(
                json!({"enabled":false,"id":"1568956638310699119","details":"В Nova Launcher","state":"Выбирает сборку"}),
            ),
            status: Mutex::new("disabled".into()),
        });
        let worker = this.clone();
        std::thread::spawn(move || {
            let mut current_id = String::new();
            let mut client: Option<DiscordIpcClient> = None;
            let mut previous = String::new();
            loop {
                let desired = worker.desired.lock().unwrap().clone();
                let id = s(&desired, "id");
                if !flag(&desired, "enabled", false)
                    || id.len() < 17
                    || id.len() > 22
                    || !id.chars().all(|c| c.is_ascii_digit())
                {
                    if let Some(mut c) = client.take() {
                        let _ = c.close();
                    }
                    *worker.status.lock().unwrap() = if flag(&desired, "enabled", false) {
                        "missing-id".into()
                    } else {
                        "disabled".into()
                    };
                    current_id.clear();
                } else {
                    if current_id != id {
                        if let Some(mut c) = client.take() {
                            let _ = c.close();
                        }
                        current_id = id.to_owned();
                        previous.clear();
                    }
                    if client.is_none() {
                        *worker.status.lock().unwrap() = "connecting".into();
                        let mut ipc = DiscordIpcClient::new(id);
                        if ipc.connect().is_ok() {
                            client = Some(ipc);
                            previous.clear();
                            *worker.status.lock().unwrap() = "connected".into();
                        } else {
                            *worker.status.lock().unwrap() = "waiting".into();
                        }
                    }
                    if let Some(c) = client.as_mut() {
                        let signature = desired.to_string();
                        if signature != previous {
                            let mut a = activity::Activity::new()
                                .details(s(&desired, "details"))
                                .state(s(&desired, "state"));
                            if let Some(t) = desired["started"].as_i64() {
                                a = a.timestamps(activity::Timestamps::new().start(t));
                            }
                            if c.set_activity(a).is_ok() {
                                previous = signature;
                            } else {
                                client = None;
                                *worker.status.lock().unwrap() = "waiting".into();
                            }
                        }
                    }
                }
                std::thread::sleep(std::time::Duration::from_secs(5));
            }
        });
        this
    }
    pub fn configure(&self, settings: &Value) {
        let mut d = self.desired.lock().unwrap();
        d["enabled"] = json!(flag(settings, "discordPresence", true));
        d["id"] = json!(if s(settings, "discordAppId").is_empty() {
            "1568956638310699119"
        } else {
            s(settings, "discordAppId")
        });
    }
    pub fn update(&self, phase: &str, inst: &Value) {
        let mut d = self.desired.lock().unwrap();
        d["details"] = json!(match phase {
            "playing" => "Играет в Minecraft",
            "launching" => "Запускает Minecraft",
            _ => "Выбирает сборку",
        });
        d["state"] = json!(if inst.is_null() {
            "Nova Launcher".to_owned()
        } else {
            format!(
                "{} · {} {}",
                s(inst, "name"),
                s(inst, "loader"),
                s(inst, "version")
            )
        });
        d.as_object_mut().unwrap().remove("started");
        if phase == "playing" {
            d["started"] = json!(chrono::Utc::now().timestamp());
        }
    }
}
