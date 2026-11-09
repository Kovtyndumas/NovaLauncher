use crate::{
    auth, backups, content, horizon, minecraft, net::Net, presence::Presence, store::Store,
    updates, util::*,
};
use anyhow::{bail, Context, Result};
use serde_json::{json, Value};
use std::{
    fs,
    path::PathBuf,
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc, Mutex,
    },
};
pub type Events = Arc<dyn Fn(&str, Value) + Send + Sync>;
pub struct Backend {
    pub store: Store,
    pub net: Net,
    pub busy: AtomicBool,
    pub child: Mutex<Option<std::process::Child>>,
    pub events: Events,
    pub presence: Arc<Presence>,
    pub update: Mutex<Value>,
}
impl Backend {
    pub fn new(root: PathBuf, events: Events) -> Result<Arc<Self>> {
        let store = Store::new(root)?;
        let presence = Presence::new();
        presence.configure(&store.settings());
        Ok(Arc::new(Self {
            store,
            net: Net::new()?,
            busy: AtomicBool::new(false),
            child: Mutex::new(None),
            events,
            presence,
            update: Mutex::new(
                json!({"phase":"idle","message":"Rust-обновления ещё не проверялись"}),
            ),
        }))
    }
    pub fn emit(&self, event: &str, args: Value) {
        (self.events)(event, args);
    }
    pub fn log(&self, line: String) {
        self.emit("game:log", json!([line]));
    }
    pub fn operation<F>(&self, job: F) -> Result<Value>
    where
        F: FnOnce() -> Result<Value>,
    {
        if self.busy.swap(true, Ordering::SeqCst) {
            bail!("Закройте игру и дождитесь завершения операции");
        }
        struct Guard<'a>(&'a AtomicBool);
        impl Drop for Guard<'_> {
            fn drop(&mut self) {
                self.0.store(false, Ordering::SeqCst);
            }
        }
        let _guard = Guard(&self.busy);
        job()
    }
    pub fn call(self: &Arc<Self>, method: &str, args: &[Value]) -> Result<Value> {
        let arg = |n: usize| args.get(n).cloned().unwrap_or(Value::Null);
        let id = arg(0).as_str().unwrap_or("").to_owned();
        let log = |line| self.log(line);
        match method {
            "getDb" => Ok(self.store.public()),
            "setDb" => {
                self.store.merge(arg(0), &arg(1))?;
                self.presence.configure(&self.store.settings());
                Ok(Value::Null)
            }
            "versions" => minecraft::versions(&self.store, &self.net),
            "history" => self.history(),
            "search" => content::search(&self.net, &arg(0)),
            "mods" => {
                let root = self.store.dir(&id)?;
                Ok(json!(content::files(&root, "mod")?
                    .as_array()
                    .unwrap()
                    .iter()
                    .map(|i| s(i, "name"))
                    .collect::<Vec<_>>()))
            }
            "contentFiles" => content::files(&self.store.dir(&id)?, arg(1).as_str().unwrap_or("")),
            "toggle" => self.operation(|| {
                content::toggle(&self.store.dir(&id)?, "mod", arg(1).as_str().unwrap_or(""))
            }),
            "install" => self.operation(|| {
                let options = arg(0);
                let inst = self.store.get(s(&options["inst"], "id"))?;
                Ok(json!(content::install(
                    &self.store,
                    &self.net,
                    &inst,
                    s(&options, "project"),
                    s(&options, "type"),
                    &log
                )?))
            }),
            "pack" => self.operation(|| {
                let o = arg(0);
                content::install_pack(
                    &self.store,
                    &self.net,
                    s(&o, "project"),
                    s(&o, "version"),
                    &log,
                )
            }),
            "importPack" => self.operation(|| {
                content::import_pack(&self.store, &self.net, &PathBuf::from(&id), &log)
            }),
            "exportPack" => self.operation(|| {
                let inst = self.store.get(&id)?;
                content::export_pack(
                    &self.store,
                    &inst,
                    &PathBuf::from(arg(1).as_str().context("Не выбран файл")?),
                )
            }),
            "checkUpdates" => {
                content::updates(&self.store.dir(&id)?, &self.net, &self.store.get(&id)?)
            }
            "updateMods" => self.operation(|| {
                content::apply_updates(
                    &self.store.dir(&id)?,
                    &self.net,
                    &self.store.get(&id)?,
                    &arg(1),
                    &log,
                )
            }),
            "manageContent" => self.operation(|| {
                content::manage(
                    &self.store.dir(&id)?,
                    &self.net,
                    &self.store.get(&id)?,
                    arg(1).as_str().unwrap_or(""),
                    arg(2).as_str().unwrap_or(""),
                    arg(3).as_str().unwrap_or(""),
                    &log,
                )
            }),
            "compatibility" => crate::compatibility::check(
                &self.store.dir(&id)?,
                &self.net,
                &self.store.get(&id)?,
                arg(1).as_bool().unwrap_or(true),
            ),
            "backups" => backups::list_backups(&self.store.dir(&id)?, "mods"),
            "rollback" => self.operation(|| {
                backups::restore(&self.store.dir(&id)?, "mods", arg(1).as_str().unwrap_or(""))
            }),
            "worldBackups" => backups::list_backups(&self.store.dir(&id)?, "worlds"),
            "backupWorlds" => self.operation(|| {
                backups::snapshot(&self.store.dir(&id)?, "worlds", "Ручная копия миров")
            }),
            "restoreWorlds" => self.operation(|| {
                backups::restore(
                    &self.store.dir(&id)?,
                    "worlds",
                    arg(1).as_str().unwrap_or(""),
                )
            }),
            "removeMod" => self.operation(|| {
                backups::remove_mod(&self.store.dir(&id)?, arg(1).as_str().unwrap_or(""))
            }),
            "removedMods" => backups::removed(&self.store.dir(&id)?),
            "restoreRemoved" => self.operation(|| {
                backups::restore_removed(&self.store.dir(&id)?, arg(1).as_str().unwrap_or(""))
            }),
            "duplicateInstance" => self.operation(|| self.store.duplicate(&id)),
            "deleteInstance" => self.operation(|| self.store.remove(&id)),
            "openDir" => {
                open::that(self.store.dir(&id)?)?;
                Ok(Value::Null)
            }
            "openContentDir" => {
                let p = self
                    .store
                    .dir(&id)?
                    .join(content::folder(arg(1).as_str().unwrap_or(""))?);
                fs::create_dir_all(&p)?;
                open::that(p)?;
                Ok(Value::Null)
            }
            "ping" => {
                let r = self.net.json(&format!(
                    "https://api.mcsrvstat.us/3/{}",
                    crate::net::enc(&id)
                ))?;
                Ok(
                    json!({"online":flag(&r,"online",false),"players":r["players"],"motd":r["motd"]["clean"][0],"version":r["version"]}),
                )
            }
            "restore" => {
                if self.store.root.join("account.json").exists() {
                    Ok(json!(s(&auth::session(&self.store, &self.net)?, "name")))
                } else {
                    Ok(json!(""))
                }
            }
            "logout" => {
                let _ = fs::remove_file(self.store.root.join("account.json"));
                Ok(Value::Null)
            }
            "discordStatus" => Ok(json!(self.presence.status.lock().unwrap().clone())),
            "discordSelect" => {
                if !self.busy.load(Ordering::SeqCst) {
                    self.presence
                        .update("launcher", &self.store.get(&id).unwrap_or(Value::Null));
                }
                Ok(Value::Null)
            }
            "horizonStatus" => Ok(
                json!({"path":horizon::detect(&self.store).map(|p|p.display().to_string()).unwrap_or_default()}),
            ),
            "horizonDownload" => {
                open::that("https://playhorizon.dev/docs/install")?;
                Ok(Value::Null)
            }
            "rustSetup" => self.operation(|| {
                let inst = self.store.get(&id)?;
                let action = arg(1);
                let action = action.as_str().unwrap_or("");
                if action != "open" && action != "install" {
                    bail!("Неизвестная команда Horizon");
                }
                let exe = horizon::ensure(&self.store, &self.net, &log)?;
                horizon::run(&exe, &self.store, &inst, action, "", &log, &|| {})?;
                Ok(json!({"installed":true}))
            }),
            "horizonRun" => self.operation(|| {
                let inst =
                    json!({"id":"horizon","name":"Horizon","version":"26.3","loader":"Rust"});
                let exe = horizon::ensure(&self.store, &self.net, &log)?;
                let output = horizon::run(
                    &exe,
                    &self.store,
                    &inst,
                    &id,
                    "",
                    &|line| self.emit("horizon:log", json!([line])),
                    &|| {},
                )?;
                Ok(json!({"output":output,"code":0}))
            }),
            "launcherStatus" => Ok(self.update.lock().unwrap().clone()),
            "launcherCheck" => {
                let state = updates::check(&self.net)?;
                *self.update.lock().unwrap() = state.clone();
                self.emit("launcher:update", json!([state]));
                Ok(self.update.lock().unwrap().clone())
            }
            "launcherDownload" => self.operation(|| {
                let state = updates::download(
                    &self.store,
                    &self.net,
                    &self.update.lock().unwrap().clone(),
                )?;
                *self.update.lock().unwrap() = state.clone();
                self.emit("launcher:update", json!([state]));
                Ok(self.update.lock().unwrap().clone())
            }),
            "launcherInstall" => {
                if self.busy.load(Ordering::SeqCst) {
                    bail!("Закройте игру");
                }
                updates::install(&self.update.lock().unwrap())?;
                Ok(Value::Null)
            }
            "lastCrash" => {
                let root = self.store.dir(&id)?;
                let mut files = vec![];
                if let Ok(entries) = fs::read_dir(root.join("crash-reports")) {
                    for e in entries.flatten() {
                        if e.path().is_file() {
                            files.push(e.path());
                        }
                    }
                }
                files.sort_by_key(|p| {
                    std::cmp::Reverse(fs::metadata(p).and_then(|m| m.modified()).ok())
                });
                let p = files
                    .first()
                    .cloned()
                    .unwrap_or(root.join("nova-last-launch.log"));
                if !p.exists() {
                    return Ok(Value::Null);
                }
                let text = fs::read_to_string(p)?;
                Ok(report(&text, -1, &id))
            }
            "launch" => {
                let inst = self.store.get(s(&arg(0), "id"))?;
                let server = arg(1).as_str().unwrap_or("").to_owned();
                if self.busy.swap(true, Ordering::SeqCst) {
                    bail!("Игра или операция уже запущена");
                }
                let backend = self.clone();
                std::thread::spawn(move || {
                    let started = chrono::Utc::now();
                    backend.presence.update("launching", &inst);
                    let b = backend.clone();
                    let logs = Arc::new(move |line| b.log(line));
                    let on_start = || {
                        backend.presence.update("playing", &inst);
                        backend.emit("game:started", json!([s(&inst, "id")]));
                    };
                    let result = if s(&inst, "loader") == "Rust" {
                        horizon::play(
                            &backend.store,
                            &backend.net,
                            &inst,
                            &server,
                            &*logs,
                            &on_start,
                        )
                    } else {
                        minecraft::launch(
                            &backend.store,
                            &backend.net,
                            &inst,
                            &server,
                            logs.clone(),
                            &on_start,
                            &backend.child,
                        )
                    };
                    let (code, crash) = match result {
                        Ok(code) => {
                            if code == 0 {
                                (0, Value::Null)
                            } else {
                                let text = fs::read_to_string(
                                    backend
                                        .store
                                        .root
                                        .join("instances")
                                        .join(s(&inst, "id"))
                                        .join("nova-last-launch.log"),
                                )
                                .unwrap_or_default();
                                (code, report(&text, code, s(&inst, "id")))
                            }
                        }
                        Err(e) => {
                            logs(e.to_string());
                            (-1, report(&e.to_string(), -1, s(&inst, "id")))
                        }
                    };
                    backend.busy.store(false, Ordering::SeqCst);
                    backend.presence.update("launcher", &inst);
                    let _ = backend.record(&inst, started, code);
                    backend.emit("game:closed", json!([code, crash]));
                });
                Ok(json!(true))
            }
            "importLegacy" => self.operation(|| {
                let count = self.store.import(&PathBuf::from(&id))?;
                Ok(json!({"count":count,"db":self.store.public()}))
            }),
            _ => bail!("Неизвестная команда: {method}"),
        }
    }
    pub fn history(&self) -> Result<Value> {
        let file = self.store.root.join("play-history.json");
        if file.exists() {
            read_json(&file)
        } else {
            Ok(json!([]))
        }
    }
    fn record(&self, inst: &Value, start: chrono::DateTime<chrono::Utc>, code: i32) -> Result<()> {
        let mut history = self.history()?;
        let items = history.as_array_mut().context("Неправильная история")?;
        items.insert(0,json!({"instanceId":s(inst,"id"),"name":s(inst,"name"),"version":s(inst,"version"),"loader":s(inst,"loader"),"engine":if s(inst,"loader")=="Rust"{"horizon"}else{"java"},"at":start.to_rfc3339(),"seconds":(chrono::Utc::now()-start).num_seconds(),"closed":true,"code":code}));
        items.truncate(100);
        write_json(&self.store.root.join("play-history.json"), &history)
    }
}
