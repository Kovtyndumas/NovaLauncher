use crate::{
    net::{enc, Net},
    runtime,
    store::{app_data, Store},
    util::*,
};
use anyhow::{bail, Context, Result};
use serde_json::{json, Value};
use std::{
    fs,
    io::{BufRead, BufReader},
    path::{Path, PathBuf},
    process::{Command, Stdio},
    sync::{atomic::{AtomicBool, Ordering}, mpsc},
    time::{Duration, Instant},
};
// v0.2.97 has no Intel Mac client. The pinned nightly adds that platform
// while still targeting Minecraft 26.3.
pub const TAG: &str = if cfg!(all(target_os="macos",target_arch="x86_64")){"nightly-2026-11-07"}else{"v0.2.97"};
struct ScopedChild(std::process::Child);
impl std::ops::Deref for ScopedChild {type Target=std::process::Child;fn deref(&self)->&Self::Target{&self.0}}
impl std::ops::DerefMut for ScopedChild {fn deref_mut(&mut self)->&mut Self::Target{&mut self.0}}
impl Drop for ScopedChild{fn drop(&mut self){if !matches!(self.0.try_wait(),Ok(Some(_))){let _=self.0.kill();let _=self.0.wait();}}}
/// One Horizon account store for all Nova instances. Reuse old installations in
/// place so their versions and keychain references do not need to be copied.
pub fn launcher_home(store: &Store) -> Result<PathBuf> {
    let settings=store.settings();
    let saved=PathBuf::from(s(&settings,"horizonLauncherHome"));
    if saved.is_absolute() && saved.is_dir(){return Ok(saved);}
    let mut candidates=vec![];
    if let Ok(entries)=fs::read_dir(store.root.join("horizon-instances")) {
        for entry in entries.flatten(){let p=entry.path().join("launcher");if p.is_dir()&&!p.is_symlink(){candidates.push(p);}}
    }
    if let Some(data)=dirs::data_dir(){let p=data.join("horizon-launcher");if p.is_dir(){candidates.push(p);}}
    candidates.sort();
    let score=|p:&PathBuf|{let accounts=read_json(&p.join("accounts.json")).unwrap_or(Value::Null);(!list(&accounts,"accounts").is_empty(),p.join("versions").join(TAG).is_dir())};
    candidates.sort_by_key(|p|std::cmp::Reverse(score(p)));
    let home=candidates.into_iter().find(|p|{let v=score(p);v.0||v.1}).unwrap_or_else(||store.root.join("horizon-launcher"));
    fs::create_dir_all(&home)?;
    store.edit(|db|{db["settings"]["horizonLauncherHome"]=json!(home);Ok(())})?;
    Ok(home)
}
pub fn process(exe:&Path,store:&Store,inst:&Value)->Result<Command>{
    let id=leaf(s(inst,"id"))?;
    let data=store.root.join("horizon-instances").join(id);
    let game=store.root.join("instances").join(id);
    fs::create_dir_all(&data)?;fs::create_dir_all(&game)?;
    let mut command=Command::new(exe);
    command.current_dir(data).env("HORIZON_LAUNCHER_HOME",launcher_home(store)?)
        .env("HORIZON_HOME",game).env("HORIZON_LAUNCHER_NO_SELF_UPDATE","1");
    Ok(command)
}
pub fn account_status(exe:&Path,store:&Store,inst:&Value)->Result<Value>{
    let output=process(exe,store,inst)?.args(["--json","account","list"]).output()?;
    if !output.status.success(){bail!("Не удалось проверить вход Horizon. Откройте Horizon и повторите вход Microsoft.");}
    let status:Value=serde_json::from_slice(&output.stdout).context("Некорректный ответ Horizon о входе")?;
    Ok(json!({"owner":flag(&status,"owner",false),"configured":flag(&status,"sign_in_configured",false),"accounts":list(&status,"accounts").iter().map(|a|json!({"name":s(a,"name"),"selected":flag(a,"selected",false)})).collect::<Vec<_>>()}))
}
fn redact(line:&str)->String{
    regex::Regex::new(r#"(?i)(access_token|refresh_token|authorization|identityToken)(["\s:=]+)(?:Bearer\s+)?[^\s,"]+"#).unwrap().replace_all(line,"$1$2[hidden]").into_owned()
}
pub fn sign_in(exe:&Path,store:&Store,inst:&Value,cancel:&AtomicBool,emit:&dyn Fn(Value))->Result<Value>{
    let mut child=ScopedChild(process(exe,store,inst)?.args(["account","sign-in"]).stdout(Stdio::piped()).stderr(Stdio::piped()).spawn()?);
    emit(json!({"state":"starting"}));
    let (sender,receiver)=mpsc::channel();
    let stdout=child.stdout.take().unwrap();let stderr=child.stderr.take().unwrap();
    let send=sender.clone();
    let out=std::thread::spawn(move||{for line in BufReader::new(stdout).lines().map_while(|r|r.ok()){let _=send.send(line);}});
    let err=std::thread::spawn(move||{for line in BufReader::new(stderr).lines().map_while(|r|r.ok()){let _=sender.send(line);}});
    let started=Instant::now();let mut errors=vec![];
    let code=regex::Regex::new(r"To sign in, open (https://[^ ]+) in a browser and enter the code ([A-Za-z0-9-]+)")?;
    let status=loop{
        if cancel.load(Ordering::SeqCst)||started.elapsed()>Duration::from_secs(900){let _=child.kill();let _=child.wait();let _=out.join();let _=err.join();bail!("Вход в Horizon отменён или время ожидания истекло");}
        while let Ok(line)=receiver.try_recv(){
            if let Some(c)=code.captures(&line){
                let link=url::Url::parse(&c[1])?;
                if !matches!(link.host_str(),Some("microsoft.com"|"www.microsoft.com"|"login.microsoftonline.com")){let _=child.kill();let _=child.wait();bail!("Horizon вернул неизвестную страницу входа");}
                emit(json!({"state":"code","link":link.as_str(),"code":&c[2]}));
            }else{errors.push(redact(&line));if errors.len()>32{errors.remove(0);}}
        }
        if let Some(status)=child.try_wait()?{break status;}
        std::thread::sleep(Duration::from_millis(80));
    };
    let _=out.join();let _=err.join();
    for line in receiver.try_iter(){if !code.is_match(&line){errors.push(redact(&line));}}
    if !status.success(){bail!("Не удалось войти в Horizon: {}",errors.iter().rev().take(3).cloned().collect::<Vec<_>>().join(" · "));}
    let result=account_status(exe,store,inst)?;
    if !flag(&result,"owner",false){bail!("Horizon не сохранил подтверждённый вход. Проверьте доступ к системной связке ключей и наличие Minecraft Java Edition у аккаунта.");}
    emit(json!({"state":"done"}));Ok(result)
}
fn find(root: &Path, depth: u32) -> Option<PathBuf> {
    if depth == 0 || !root.is_dir() {
        return None;
    }
    for item in fs::read_dir(root).ok()?.flatten() {
        let p = item.path();
        if p.is_symlink() {
            continue;
        }
        if p.is_dir() {
            if let Some(found) = find(&p, depth - 1) {
                return Some(found);
            }
        } else if p.file_name().is_some_and(|n| {
            n == if cfg!(windows) {
                "horizon-launcher.exe"
            } else {
                "horizon-launcher"
            }
        }) {
            return Some(p);
        }
    }
    None
}
pub fn executable(path: &Path) -> Result<PathBuf> {
    let path = if path.extension().is_some_and(|e| e == "app") {
        path.join("Contents/MacOS/horizon-launcher")
    } else {
        path.to_owned()
    };
    if !path.is_file() {
        bail!("Не найден Horizon Launcher");
    }
    Ok(path)
}
pub fn detect(store: &Store) -> Option<PathBuf> {
    let settings = store.settings();
    if !s(&settings, "horizonPath").is_empty() {
        if let Ok(exe) = executable(Path::new(s(&settings, "horizonPath"))) {
            return Some(exe);
        }
    }
    for root in [
        store.root.join("runtimes/horizon-launcher"),
        app_data("nova-launcher").join("runtimes/horizon-launcher"),
    ] {
        if let Some(exe) = find(&root, 12) {
            return Some(exe);
        }
    }
    if cfg!(target_os = "macos") {
        return executable(Path::new("/Applications/Horizon.app")).ok();
    }
    None
}
pub fn ensure(store: &Store, net: &Net, log: &dyn Fn(String)) -> Result<PathBuf> {
    if let Some(exe) = detect(store) {
        return Ok(exe);
    }
    let asset = match (runtime::os(), runtime::arch()) {
        ("osx", "aarch64") => "horizon-launcher-macos-arm64.zip",
        ("osx", "x64") => "horizon-launcher-macos-x64.zip",
        ("windows", _) => "horizon-launcher-windows-x64.zip",
        ("linux", "x64") => "horizon-launcher-linux-x86_64.tar.gz",
        _ => bail!("Horizon 26.3 доступен для Mac ARM64/Intel, Windows x64 и Linux x64"),
    };
    log("Загрузка официального Horizon Launcher".into());
    let release = net.json(&format!(
        "https://api.github.com/repos/horizon-rust/horizon/releases/tags/{}",
        enc("launcher@0.2.54")
    ))?;
    let file = list(&release, "assets")
        .iter()
        .find(|a| s(a, "name") == asset)
        .context("Нет установщика Horizon")?;
    let hash = s(file, "digest")
        .strip_prefix("sha256:")
        .context("Нет контрольной суммы Horizon")?;
    let archive = store.root.join("downloads").join(asset);
    net.download(
        s(file, "browser_download_url"),
        &archive,
        "sha256",
        hash,
        number(file, "size", 0),
    )?;
    let root = store.root.join("runtimes/horizon-launcher");
    if asset.ends_with(".zip") {
        unzip(&archive, &root, "")?;
    } else {
        tar_gz(&archive, &root)?;
    }
    let exe = find(&root, 12).context("В архиве нет Horizon")?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(&exe, fs::Permissions::from_mode(0o755))?;
    }
    Ok(exe)
}
pub fn command(action: &str) -> Result<Vec<String>> {
    Ok(match action {
        "open" => vec![],
        "versions" => vec!["versions", "--installed"],
        "install" => vec!["install", TAG],
        "prepare" => vec!["prepare", "--download", "--version", TAG],
        "play" => vec!["play", "--version", TAG, "--instance", "horizon"],
        _ => bail!("Неизвестная команда Horizon"),
    }
    .iter()
    .map(|s| s.to_string())
    .collect())
}
pub fn run(
    exe: &Path,
    store: &Store,
    inst: &Value,
    action: &str,
    server: &str,
    log: &dyn Fn(String),
    started: &dyn Fn(),
) -> Result<String> {
    let mut args = command(if action=="play-check"{"play"}else{action})?;
    if action=="play-check"{
        args.extend(["--arg".into(),"--frames".into(),"--arg".into(),"180".into(),"--arg".into(),"--smoke-report".into(),"--arg".into(),store.root.join("horizon-ui-smoke.json").to_string_lossy().into_owned()]);
    }
    if action == "play" && !server.is_empty() {
        args.extend(["--server".into(), server.into()]);
    }
    let mut launch=process(exe,store,inst)?;
    if action=="prepare"||action=="play"||action=="play-check"{
        let java=runtime::ensure_jdk(store,&Net::new()?,25,runtime::arch(),log)?;
        let bin=java.parent().context("Не найдена папка JDK")?;
        launch.env("JAVA_HOME",bin.parent().context("Не найдена папка JDK")?);
        let mut paths=vec![bin.to_path_buf()];paths.extend(std::env::split_paths(&std::env::var_os("PATH").unwrap_or_default()));
        launch.env("PATH",std::env::join_paths(paths)?);
    }
    let mut child = launch.args(args)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()?;
    started();
    let stdout=child.stdout.take().unwrap();let stderr=child.stderr.take().unwrap();
    let (sender,receiver)=mpsc::channel();let send=sender.clone();
    let output_thread=std::thread::spawn(move||{for line in BufReader::new(stdout).lines().map_while(|r|r.ok()){let _=send.send((false,line));}});
    let error_thread=std::thread::spawn(move||{for line in BufReader::new(stderr).lines().map_while(|r|r.ok()){let _=sender.send((true,line));}});
    let mut output = String::new();
    let mut err=String::new();
    for (is_error,line) in receiver {
        let safe = redact(&line);
        log(safe.clone());
        output.push_str(&safe);
        output.push('\n');
        if is_error{err.push_str(&safe);err.push('\n');}
    }
    let status = child.wait()?;
    let _=output_thread.join();let _=error_thread.join();
    if !status.success() {
        bail!(
            "Horizon: {} · {}",
            status,
            redact(&err).chars().take(1200).collect::<String>()
        );
    }
    Ok(output)
}
pub fn play(
    store: &Store,
    net: &Net,
    inst: &Value,
    server: &str,
    log: &dyn Fn(String),
    started: &dyn Fn(),
) -> Result<i32> {
    if s(inst, "version") != "26.3" {
        bail!("Выберите 26.3 RUST");
    }
    let exe = ensure(store, net, log)?;
    if !flag(&account_status(&exe,store,inst)?,"owner",false){bail!("Требуется вход Horizon: нажмите «Войти в Horizon» и подтвердите аккаунт с Minecraft Java Edition. Вход Microsoft в Nova используется отдельно.");}
    let installed = run(&exe, store, inst, "versions", "", log, &|| {})?;
    if !installed.contains(TAG) {
        run(&exe, store, inst, "install", "", log, &|| {})?;
    }
    run(&exe, store, inst, "prepare", "", log, &|| {})?;
    run(&exe, store, inst, "play", server, log, started)?;
    Ok(0)
}
