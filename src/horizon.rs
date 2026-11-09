use crate::{
    net::{enc, Net},
    runtime,
    store::{app_data, Store},
    util::*,
};
use anyhow::{bail, Context, Result};
use serde_json::Value;
use std::{
    fs,
    io::{BufRead, BufReader},
    path::{Path, PathBuf},
    process::{Command, Stdio},
};
pub const TAG: &str = "v0.2.97";
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
        ("windows", _) => "horizon-launcher-windows-x64.zip",
        ("linux", "x64") => "horizon-launcher-linux-x86_64.tar.gz",
        _ => bail!("Horizon 26.3 доступен для Mac ARM64, Windows x64 и Linux x64"),
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
        "play" => vec!["play", "--version", TAG],
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
    let data = store.root.join("horizon-instances").join(s(inst, "id"));
    fs::create_dir_all(&data)?;
    let game = store.root.join("instances").join(s(inst, "id"));
    fs::create_dir_all(&game)?;
    let mut args = command(action)?;
    if action == "play" && !server.is_empty() {
        args.extend(["--server".into(), server.into()]);
    }
    let mut child = Command::new(exe)
        .args(args)
        .current_dir(&data)
        .env("HORIZON_LAUNCHER_HOME", data.join("launcher"))
        .env("HORIZON_HOME", game)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()?;
    started();
    let err = child.stderr.take().unwrap();
    let error_thread = std::thread::spawn(move || {
        let mut output = String::new();
        use std::io::Read;
        let _ = BufReader::new(err).read_to_string(&mut output);
        output
    });
    let mut output = String::new();
    for line in BufReader::new(child.stdout.take().unwrap())
        .lines()
        .map_while(|r| r.ok())
    {
        let safe = regex::Regex::new(r#"(?i)(access_token|refresh_token)(["\s:=]+)[^\s,"]+"#)
            .unwrap()
            .replace_all(&line, "$1$2[hidden]")
            .into_owned();
        log(safe.clone());
        output.push_str(&safe);
        output.push('\n');
    }
    let status = child.wait()?;
    let err = error_thread.join().unwrap_or_default();
    if !status.success() {
        bail!(
            "Horizon: {} · {}",
            status,
            err.chars().take(1200).collect::<String>()
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
    let installed = run(&exe, store, inst, "versions", "", log, &|| {})?;
    if !installed.contains(TAG) {
        run(&exe, store, inst, "install", "", log, &|| {})?;
    }
    run(&exe, store, inst, "prepare", "", log, &|| {})?;
    run(&exe, store, inst, "play", server, log, started)?;
    Ok(0)
}
