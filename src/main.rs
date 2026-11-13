#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]
mod auth;
mod ai;
mod backend;
mod backups;
mod compatibility;
mod content;
mod design;
mod downloads;
mod hardware;
mod recovery;
mod safety;
mod horizon;
mod minecraft;
mod migration;
mod net;
mod navigation;
mod presence;
mod profiles;
mod checkpoints;
mod worlds;
mod diagnostics;
mod runtime;
mod store;
#[cfg(test)]
mod tests;
mod updates;
mod util;
use anyhow::{Context, Result};
use backend::Backend;
use serde_json::{json, Value};
use std::{
    path::PathBuf,
    sync::{Arc, Mutex},
};
use tauri::{Emitter, Manager};
use util::*;
fn smoke() -> bool {
    std::env::args().any(|s| s == "--smoke")
}
#[tauri::command]
async fn dispatch(
    app: tauri::AppHandle,
    window: tauri::WebviewWindow,
    state: tauri::State<'_, Arc<Backend>>,
    method: String,
    mut args: Vec<Value>,
) -> std::result::Result<Value, String> {
    if window.label() != "main" {
        return Err("Команда недоступна этому окну".into());
    }
    let backend = state.inner().clone();
    if method == "ready" {
        let _ = window.show();
        if smoke() {
            let app2 = app.clone();
            std::thread::spawn(move || {
                std::thread::sleep(std::time::Duration::from_millis(500));
                if let Some(w) = app2.get_webview_window("main") {
                    let _ = w.eval(include_str!("../ui/smoke.js"));
                }
            });
        }
        if std::env::args().any(|a|a=="--horizon-sign-in") {
            let handle=app.clone();
            std::thread::spawn(move||{
                std::thread::sleep(std::time::Duration::from_millis(600));
                if let Some(window)=handle.get_webview_window("main"){
                    let _=window.eval("const loginTimer=setInterval(()=>{if(window.NovaReady){clearInterval(loginTimer);signInHorizon(S.db.instances.find(i=>i.loader==='Rust')?.id);}},100);");
                }
            });
        }
        return Ok(Value::Null);
    }
    if method == "smokeReport" {
        if !smoke() {
            return Err("Тестовый режим выключен".into());
        }
        let report = args.first().cloned().unwrap_or(Value::Null);
        let success = report["ok"].as_bool() == Some(true);
        let _ = write_json(&PathBuf::from("target/ui-smoke.json"), &report);
        println!(
            "NOVA_RUST_UI_{} {}",
            if success { "OK" } else { "FAILED" },
            report
        );
        app.exit(if success { 0 } else { 1 });
        return Ok(report);
    }
    if method == "versions" && smoke() {
        return Ok(json!({"versions":["26.3","26.2","1.21.1","1.0"],"cached":true}));
    }
    if method == "login" {
        let app = app.clone();
        return tauri::async_runtime::spawn_blocking(move || {
            backend.operation(|| login_window(&app, &backend))
        })
        .await
        .map_err(|e| e.to_string())?
        .map_err(|e| e.to_string());
    }
    if method == "horizonChoose" {
        return tauri::async_runtime::spawn_blocking(move || {
            let p = rfd::FileDialog::new()
                .set_title("Выберите Horizon Launcher")
                .pick_file();
            if let Some(path) = p {
                horizon::executable(&path)?;
                backend.store.edit(|db| {
                    db["settings"]["horizonPath"] = json!(path);
                    Ok(())
                })?;
                Ok(json!({"path":path,"db":backend.store.public()}))
            } else {
                Ok(Value::Null)
            }
        })
        .await
        .map_err(|e| e.to_string())?
        .map_err(|e: anyhow::Error| e.to_string());
    }
    let ui_method = method.clone();
    let app2 = app.clone();
    tauri::async_runtime::spawn_blocking(move||->Result<Value>{
 if method=="exportDiagnostic"{let id=args.first().and_then(Value::as_str).unwrap_or("");let report=crate::diagnostics::report(&backend.store,id,args.get(1).and_then(Value::as_str).unwrap_or(""))?;let Some(path)=rfd::FileDialog::new().set_file_name("Nova-diagnostics.txt").add_filter("Текст",&["txt"]).save_file()else{return Ok(Value::Null)};crate::util::atomic(&path,s(&report,"text").as_bytes())?;return Ok(json!({"saved":true}));}
 if method=="migrationScan" {let Some(path)=rfd::FileDialog::new().set_title("Сборки Prism / MultiMC или папка Minecraft").pick_folder()else{return Ok(Value::Null)};let plan=crate::migration::scan(&path)?;*backend.migration.lock().unwrap()=plan.clone();return Ok(plan);}
 if method=="chooseWallpaper"{return match rfd::FileDialog::new().set_title("Свои обои").add_filter("Изображение",&["png","jpg","jpeg","webp"]).pick_file(){Some(path)=>crate::design::wallpaper(&path),None=>Ok(Value::Null)};}
 if method=="importDesign"{return match rfd::FileDialog::new().set_title("Импорт оформления Nova").add_filter("Оформление Nova",&["json"]).pick_file(){Some(path)=>crate::design::import(&path),None=>Ok(Value::Null)};}
 if method=="exportDesign"{return match rfd::FileDialog::new().set_file_name("My-Nova.nova-theme.json").add_filter("Оформление Nova",&["json"]).save_file(){Some(path)=>crate::design::export(&path,args.first().unwrap_or(&Value::Null)),None=>Ok(Value::Null)};}
 if method=="importPack"&&args.first().is_none_or(Value::is_null){let Some(path)=rfd::FileDialog::new().set_title("Импорт Modrinth").add_filter("Modrinth",&["mrpack"]).pick_file()else{return Ok(Value::Null)};args=vec![json!(path)];}
 if method=="exportPack"{let id=args.first().and_then(Value::as_str).unwrap_or("");let inst=backend.store.get(id)?;let Some(path)=rfd::FileDialog::new().set_file_name(format!("{}.mrpack",s(&inst,"name"))).add_filter("Modrinth",&["mrpack"]).save_file()else{return Ok(Value::Null)};args.push(json!(path));}
 if method=="deleteInstance"{let answer=rfd::MessageDialog::new().set_title("Удалить сборку?").set_description("Файлы и миры сохранятся в deleted-instances. Восстановить их можно из папки данных Nova Rust.").set_buttons(rfd::MessageButtons::OkCancel).show();if answer!=rfd::MessageDialogResult::Ok{return Ok(Value::Null);}}
 if method=="importLegacy"{let Some(path)=rfd::FileDialog::new().set_title("Папка данных Nova с db.json").set_directory(crate::store::app_data("nova-launcher")).pick_folder()else{return Ok(Value::Null)};args=vec![json!(path)];}
 let result=backend.call(&method,&args);if ui_method=="launcherRecover"&&result.is_ok(){app2.exit(0);}if ui_method=="launcherInstall"&&result.is_ok(){if let Some(w)=app2.get_webview_window("main"){let _=w.set_focus();}}result
 }).await.map_err(|e|e.to_string())?.map_err(|e|e.to_string())
}
fn login_window(app: &tauri::AppHandle, backend: &Backend) -> Result<Value> {
    let state = uuid::Uuid::new_v4().to_string();
    let target = auth::link(&state).parse()?;
    let (tx, rx) = std::sync::mpsc::channel::<std::result::Result<String, String>>();
    let tx = Arc::new(Mutex::new(Some(tx)));
    let app2 = app.clone();
    let sender = tx.clone();
    app.run_on_main_thread(move || {
        let app3 = app2.clone();
        let callback = sender.clone();
        let window =
            tauri::WebviewWindowBuilder::new(&app2, "ms-auth", tauri::WebviewUrl::External(target))
                .title("Microsoft · Nova Rust")
                .inner_size(520., 700.)
                .on_navigation(move |url| {
                    if let Some(result) = auth::redirect_code(url, &state) {
                        let result = result.map_err(|e| e.to_string());
                        if let Some(tx) = callback.lock().unwrap().take() {
                            let _ = tx.send(result);
                        }
                        let app = app3.clone();
                        let _ = app3.run_on_main_thread(move || {
                            if let Some(w) = app.get_webview_window("ms-auth") {
                                let _ = w.close();
                            }
                        });
                        false
                    } else {
                        true
                    }
                })
                .build();
        match window {
            Ok(window) => {
                window.on_window_event(move |event| {
                    if matches!(event, tauri::WindowEvent::Destroyed) {
                        if let Some(tx) = sender.lock().unwrap().take() {
                            let _ = tx.send(Err("Вход отменён".into()));
                        }
                    }
                });
            }
            Err(e) => {
                if let Some(tx) = sender.lock().unwrap().take() {
                    let _ = tx.send(Err(e.to_string()));
                }
            }
        }
    })?;
    let response = rx.recv_timeout(std::time::Duration::from_secs(300));
    let closing = app.clone();
    let _ = app.run_on_main_thread(move || {
        if let Some(window) = closing.get_webview_window("ms-auth") {
            let _ = window.close();
        }
    });
    let code = response
        .context("Время входа истекло. Попробуйте ещё раз.")?
        .map_err(anyhow::Error::msg)?;
    Ok(json!(auth::login(&backend.store, &backend.net, &code)?))
}
fn root() -> PathBuf {
    if smoke() || std::env::args().any(|a| a == "--probe-network" || a == "--test-game") {
        std::env::temp_dir().join(format!("nova-rust-test-{}", uuid::Uuid::new_v4()))
    } else {
        std::env::var("NOVA_RUST_DATA")
            .map(PathBuf::from)
            .unwrap_or_else(|_| store::app_data("nova-rust"))
    }
}
fn main() {
    if let Err(e) = run() {
        eprintln!("Nova Rust: {e:#}");
        std::process::exit(1);
    }
}
fn run() -> Result<()> {
    let root = root();
    if std::env::args().any(|a|a=="--prepare-horizon"||a=="--check-horizon-game"){
        let store=store::Store::new(root)?;let net=net::Net::new()?;
        let inst=json!({"id":"nova-horizon-check","version":"26.3","loader":"Rust"});
        let exe=horizon::ensure(&store,&net,&|line|println!("{line}"))?;
        if !flag(&horizon::account_status(&exe,&store,&inst)?,"owner",false){anyhow::bail!("Требуется вход Horizon");}
        if std::env::args().any(|a|a=="--check-horizon-game"){
            horizon::run(&exe,&store,&inst,"play-check","",&|line|println!("{line}"),&||{})?;
            println!("NOVA_HORIZON_GAME_EXIT_OK");
        }else{
            horizon::run(&exe,&store,&inst,"prepare","",&|line|println!("{line}"),&||{})?;
            println!("NOVA_HORIZON_PREPARED");
        }
        return Ok(());
    }
    if std::env::args().any(|a| a == "--probe-ai") {
        let temporary=std::env::temp_dir().join(format!("nova-ai-probe-{}",uuid::Uuid::new_v4()));
        let result=(||{
            let store=store::Store::new(temporary.clone())?;
            write_json(&temporary.join("gemini-private.json"),&read_json(&root.join("gemini-private.json"))?)?;
            store.edit(|db|{db["instances"]=json!([{"id":"ai-probe","name":"AI probe","version":"1.21.1","loader":"Fabric"}]);Ok(())})?;
            let result=ai::plan(&store,&net::Net::new()?,"ai-probe","Сборка для повышения FPS и удобного инвентаря, не больше четырёх модов")?;
            if std::env::args().any(|a| a == "--probe-ai-install") {
                let ids=json!(list(&result,"mods").iter().map(|m|s(m,"project")).collect::<Vec<_>>());
                let installed=ai::install(&store,&net::Net::new()?,&result,&store.get("ai-probe")?,s(&result,"id"),&ids,&|line|println!("{line}"))?;
                println!("NOVA_GEMINI_INSTALL_OK {}",installed);
            }
            println!("NOVA_GEMINI_OK {}",result);Ok(())
        })();
        let _=std::fs::remove_dir_all(temporary);
        return result;
    }
    if std::env::args().any(|a| a == "--probe-network") {
        return probe(root);
    }
    if std::env::args().any(|a| a == "--test-game") {
        return game_probe(root);
    }
    let app=tauri::Builder::default().invoke_handler(tauri::generate_handler![dispatch]).setup(move|app|{let handle=app.handle().clone();let events=Arc::new(move|event:&str,args:Value|{let _=handle.emit(event,args);});let backend=Backend::new(root.clone(),events)?;if smoke(){backend.store.edit(|db|{db["instances"]=json!([{"id":"smoke","name":"Мой мир","version":"1.21.1","loader":"Fabric"}]);db["settings"]["onboarded"]=json!(true);db["settings"]["startupAnimations"]=json!(false);db["settings"]["discordPresence"]=json!(false);db["settings"]["autoUpdate"]=json!(false);Ok(())})?;backend.presence.configure(&backend.store.settings());if let Some(w)=app.get_webview_window("main"){let _=w.show();let _=w.set_focus();}let watchdog=app.handle().clone();std::thread::spawn(move||{std::thread::sleep(std::time::Duration::from_secs(45));eprintln!("NOVA_RUST_UI_TIMEOUT");watchdog.exit(1);});}else{let service=backend.clone();std::thread::spawn(move||{std::thread::sleep(std::time::Duration::from_secs(12));loop{if flag(&service.store.settings(),"autoUpdate",true){let _=service.call("launcherCheck",&[]);}std::thread::sleep(std::time::Duration::from_secs(14400));}});}app.manage(backend);Ok(())}).build(tauri::generate_context!())?;
    app.run(|_, _| {});
    Ok(())
}
fn probe(root: PathBuf) -> Result<()> {
    let b = Backend::new(
        root.clone(),
        Arc::new(|e, args| {
            if e == "game:log" {
                println!("{}", args[0]);
            }
        }),
    )?;
    let versions = b.call("versions", &[])?;
    if !list(&versions, "versions")
        .iter()
        .any(|v| v.as_str() == Some("1.0"))
    {
        anyhow::bail!("Нет 1.0");
    }
    let inst = json!({"id":"probe","name":"Probe","version":"1.21.1","loader":"Fabric"});
    b.store.add(inst.clone())?;
    for (project, kind) in [
        ("fabric-api", "mod"),
        ("complementary-reimagined", "shader"),
        ("fresh-animations", "resourcepack"),
    ] {
        let search = b.call(
            "search",
            &[json!({"q":project,"type":kind,"ver":"1.21.1","loader":"Fabric"})],
        )?;
        if search.as_array().is_none_or(|v| v.is_empty()) {
            anyhow::bail!("Пустой каталог {kind}");
        }
        b.call(
            "install",
            &[json!({"project":project,"inst":inst,"type":kind})],
        )?;
        let files = content::files(&b.store.dir("probe")?, kind)?;
        if files.as_array().is_none_or(|v| v.is_empty()) {
            anyhow::bail!("Не установлен {kind}");
        }
    }
    let history=b.call("changeHistory",&[json!("probe")])?;
    if history.as_array().is_none_or(|v|v.len()!=3){anyhow::bail!("Установка контента не записалась в историю");}
    let project=b.call("projectDetails",&[json!("sodium")])?;
    if s(&project,"id").is_empty(){anyhow::bail!("Нет описания проекта");}
    let compatible=b.call("projectVersions",&[json!("sodium"),json!("1.21.1"),json!("Fabric"),json!("mod")])?;
    if compatible.as_array().is_none_or(|v|v.is_empty()){anyhow::bail!("Нет совместимых версий Sodium");}
    let plan=b.call("performancePlan",&[json!("probe")])?;
    if list(&plan,"mods").is_empty(){anyhow::bail!("Пустая подборка оптимизации");}
    let selected=json!(list(&plan,"mods").iter().map(|v|s(v,"project")).collect::<Vec<_>>());
    let installed=b.call("aiInstall",&[json!("probe"),plan["id"].clone(),selected])?;
    if number(&installed,"count",0)==0{anyhow::bail!("Моды оптимизации не установлены");}
    println!("NOVA_RUST_OPTIMIZATION_OK · {} mods · project details · compatible versions · change history",installed["count"]);
    println!(
        "NOVA_RUST_NETWORK_OK · {} releases · mod shader resourcepack",
        list(&versions, "versions").len()
    );
    std::fs::remove_dir_all(root)?;
    Ok(())
}
fn game_probe(root: PathBuf) -> Result<()> {
    let (events, received) = std::sync::mpsc::channel();
    let events = Arc::new(Mutex::new(events));
    let b = Backend::new(
        root.clone(),
        Arc::new(move |name: &str, args: Value| {
            let _ = events.lock().unwrap().send((name.to_string(), args));
        }),
    )?;
    let inst =
        json!({"id":"real-game","name":"Native Rust startup","version":"26.2","loader":"Vanilla"});
    b.store.add(inst.clone())?;
    b.store.edit(|db| {
        db["settings"]["name"] = json!("NovaTest");
        db["settings"]["ram"] = json!(2);
        db["settings"]["discordPresence"] = json!(false);
        Ok(())
    })?;
    let cache = store::app_data("nova-launcher").join("instances/mw6v2ywa");
    for name in ["assets", "libraries"] {
        link_cache(&cache.join(name), &root.join("minecraft").join(name))?;
    }
    b.call("launch", &[inst])?;
    let until = std::time::Instant::now() + std::time::Duration::from_secs(120);
    let mut initialized = false;
    let mut stopped = false;
    loop {
        if std::time::Instant::now() > until {
            anyhow::bail!("Minecraft startup timeout");
        }
        if let Ok((event, args)) = received.recv_timeout(std::time::Duration::from_secs(1)) {
            if event == "game:log" {
                let line = args[0].as_str().unwrap_or("");
                println!("{line}");
                if line.contains("OpenAL initialized") || line.contains("Sound engine started") {
                    initialized = true;
                    if !stopped {
                        stopped = true;
                        let b = b.clone();
                        std::thread::spawn(move || {
                            std::thread::sleep(std::time::Duration::from_secs(3));
                            if let Some(child) = b.child.lock().unwrap().as_mut() {
                                let _ = child.kill();
                            }
                        });
                    }
                }
            }
            if event == "game:closed" {
                break;
            }
        }
    }
    if !initialized {
        anyhow::bail!("Native engine did not initialize");
    }
    println!("NOVA_RUST_REAL_MINECRAFT_OK");
    std::fs::remove_dir_all(root)?;
    Ok(())
}
fn link_cache(src: &std::path::Path, dst: &std::path::Path) -> Result<()> {
    if !src.exists() || src.is_symlink() {
        return Ok(());
    }
    if src.is_dir() {
        std::fs::create_dir_all(dst)?;
        for e in std::fs::read_dir(src)? {
            let e = e?;
            link_cache(&e.path(), &dst.join(e.file_name()))?;
        }
    } else {
        if std::fs::hard_link(src, dst).is_err() {
            std::fs::copy(src, dst)?;
        }
    }
    Ok(())
}
