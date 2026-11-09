use crate::{
    net::Net,
    store::{app_data, Store},
    util::*,
};
use anyhow::{bail, Result};
use serde_json::Value;
use std::{
    fs,
    path::{Path, PathBuf},
    process::Command,
};
pub fn os() -> &'static str {
    if cfg!(target_os = "macos") {
        "osx"
    } else if cfg!(target_os = "windows") {
        "windows"
    } else {
        "linux"
    }
}
pub fn arch() -> &'static str {
    if cfg!(target_arch = "aarch64") {
        "aarch64"
    } else {
        "x64"
    }
}
pub fn executable(home: &Path) -> PathBuf {
    home.join("bin")
        .join(if cfg!(windows) { "java.exe" } else { "java" })
}
pub fn major(exe: &Path) -> u32 {
    let Ok(output) = Command::new(exe).arg("-version").output() else {
        return 0;
    };
    let out = String::from_utf8_lossy(&output.stderr);
    regex::Regex::new("version \\\"(?:1\\.)?(\\d+)")
        .unwrap()
        .captures(&out)
        .and_then(|c| c[1].parse().ok())
        .unwrap_or(0)
}
fn find(root: &Path, depth: u32, out: &mut Vec<PathBuf>) {
    if depth == 0 || !root.is_dir() {
        return;
    }
    let Ok(entries) = fs::read_dir(root) else {
        return;
    };
    for item in entries.flatten() {
        let p = item.path();
        if p.is_symlink() {
            continue;
        }
        if p.is_dir() {
            find(&p, depth - 1, out);
        } else if p
            .file_name()
            .is_some_and(|n| n == if cfg!(windows) { "java.exe" } else { "java" })
        {
            out.push(p);
        }
    }
}
fn runtime_arch(exe: &Path) -> String {
    let home = exe.parent().unwrap().parent().unwrap();
    let text = fs::read_to_string(home.join("release")).unwrap_or_default();
    if text.contains("OS_ARCH=\"aarch64\"") || text.contains("OS_ARCH=\"arm64\"") {
        "aarch64".into()
    } else {
        "x64".into()
    }
}
pub fn ensure(
    store: &Store,
    net: &Net,
    version: u32,
    target_arch: &str,
    log: &dyn Fn(String),
) -> Result<PathBuf> {
    ensure_inner(store,net,version,target_arch,log,false)
}
/// Horizon's resource exporter needs the JDK tools, independently of a Java
/// instance's custom runtime (which may legitimately be Java 8 or 17).
pub fn ensure_jdk(store:&Store,net:&Net,version:u32,target_arch:&str,log:&dyn Fn(String))->Result<PathBuf>{
    ensure_inner(store,net,version,target_arch,log,true)
}
fn ensure_inner(store:&Store,net:&Net,version:u32,target_arch:&str,log:&dyn Fn(String),jdk:bool)->Result<PathBuf>{
    let settings = store.settings();
    let custom = s(&settings, "java");
    if !jdk && !custom.is_empty() {
        let exe = PathBuf::from(custom);
        if major(&exe) != version {
            bail!("Нужна Java {version}. Очистите ручной путь к Java для автоматического выбора.");
        }
        return Ok(exe);
    }
    let mut candidates = vec![];
    if let Ok(home) = std::env::var("JAVA_HOME") {
        candidates.push(executable(Path::new(&home)));
    }
    for root in [
        store.root.join("runtimes"),
        app_data("nova-launcher").join("runtimes"),
        dirs::home_dir().unwrap().join(".jdks"),
    ] {
        find(&root, 10, &mut candidates);
    }
    for p in candidates {
        if runtime_arch(&p) == target_arch && major(&p) == version && (!jdk||has_tools(&p)) {
            return Ok(p);
        }
    }
    if !flag(&settings, "autoJava", true) {
        bail!("Укажите путь к Java {version} или включите автоматическую загрузку");
    }
    log(format!("Загрузка Java {version} · {target_arch}"));
    let platform = if os() == "osx" { "mac" } else { os() };
    let releases=net.json(&format!("https://api.adoptium.net/v3/assets/latest/{version}/hotspot?architecture={target_arch}&image_type=jdk&os={platform}"))?;
    let pkg = &releases[0]["binary"]["package"];
    let url = s(pkg, "link");
    if url.is_empty() {
        bail!("Нет Java для этой платформы");
    }
    let archive = store.root.join("downloads").join(format!(
        "java-{version}-{target_arch}{}",
        if url.ends_with(".zip") {
            ".zip"
        } else {
            ".tar.gz"
        }
    ));
    net.download(
        url,
        &archive,
        "sha256",
        s(pkg, "checksum"),
        number(pkg, "size", 0),
    )?;
    let target = store
        .root
        .join("runtimes")
        .join(format!("{version}-{target_arch}"));
    let stage = target.with_extension(uuid::Uuid::new_v4().to_string());
    fs::create_dir_all(&stage)?;
    let result = (|| {
        if url.ends_with(".zip") {
            unzip(&archive, &stage, "")?;
        } else {
            tar_gz(&archive, &stage)?;
        }
        let mut found = vec![];
        find(&stage, 10, &mut found);
        let java = found
            .first()
            .ok_or_else(|| anyhow::anyhow!("В архиве нет Java"))?;
        let rel = java.strip_prefix(&stage)?.to_owned();
        if target.exists() {
            fs::remove_dir_all(&target)?;
        }
        fs::rename(&stage, &target)?;
        let exe = target.join(rel);
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            fs::set_permissions(&exe, fs::Permissions::from_mode(0o755))?;
        }
        if major(&exe) != version {
            bail!("Java не прошла проверку");
        }
        if jdk&&!has_tools(&exe){bail!("Для подготовки Horizon нужны java, javac и jar из JDK {version}");}
        Ok(exe)
    })();
    let _ = fs::remove_dir_all(stage);
    result
}
fn has_tools(java:&Path)->bool{
    let Some(bin)=java.parent()else{return false;};
    [if cfg!(windows){"javac.exe"}else{"javac"},if cfg!(windows){"jar.exe"}else{"jar"}].iter().all(|name|bin.join(name).is_file())
}
pub fn game_arch(profile: &Value) -> &'static str {
    if os() == "osx"
        && arch() == "aarch64"
        && !list(profile, "libraries").is_empty()
        && !profile.to_string().contains("natives-macos-arm64")
        && !profile.to_string().contains("natives-osx-arm64")
    {
        "x64"
    } else {
        arch()
    }
}
