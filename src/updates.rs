use crate::{net::Net, runtime, store::Store, util::*};
use anyhow::{bail, Context, Result};
use serde_json::{json, Value};
use std::{
    fs,
    path::{Path, PathBuf},
};
fn version(tag: &str) -> Option<(u64, u64, u64, u8, u64)> {
    let re = regex::Regex::new(r"^(?:rust-v)?(\d+)\.(\d+)\.(\d+)(?:-beta\.(\d+))?$").ok()?;
    let c = re.captures(tag)?;
    Some((
        c[1].parse().ok()?,
        c[2].parse().ok()?,
        c[3].parse().ok()?,
        u8::from(c.get(4).is_none()),
        c.get(4)
            .map(|v| v.as_str().parse())
            .transpose()
            .ok()?
            .unwrap_or(0),
    ))
}
fn asset_matches(name: &str, os: &str, arch: &str) -> bool {
    let lower = name.to_ascii_lowercase();
    match os {
        "osx" => {
            name.ends_with(".dmg")
                && if arch == "aarch64" {
                    lower.contains("aarch64") || lower.contains("arm64")
                } else {
                    lower.contains("x64") || lower.contains("x86_64") || lower.contains("intel")
                }
        }
        "windows" => name.ends_with(".exe"),
        _ => name.ends_with(".AppImage") || name.ends_with(".deb"),
    }
}
pub fn release_state(releases: &Value, current: &str, os: &str, arch: &str) -> Value {
    let installed = version(current).unwrap_or((0, 0, 0, 0, 0));
    let mut candidates: Vec<&Value> = releases
        .as_array()
        .map(Vec::as_slice)
        .unwrap_or(&[])
        .iter()
        .filter(|r| {
            s(r, "tag_name").starts_with("rust-v")
                && !flag(r, "draft", false)
                && version(s(r, "tag_name")).is_some_and(|v| v > installed)
        })
        .collect();
    candidates.sort_by_key(|r| std::cmp::Reverse(version(s(r, "tag_name"))));
    for release in candidates {
        for asset in list(release, "assets") {
            if asset_matches(s(asset, "name"), os, arch)
                && s(asset, "digest").starts_with("sha256:")
            {
                return json!({"phase":"available","message":format!("Доступна Nova {}",s(release,"tag_name")),"version":release["tag_name"],"asset":asset,"url":release["html_url"]});
            }
        }
    }
    json!({"phase":"current","message":"Установлена актуальная или более новая локальная версия Nova"})
}
pub fn check(net: &Net) -> Result<Value> {
    let releases =
        net.json("https://api.github.com/repos/Kovtyndumas/NovaLauncher/releases?per_page=30")?;
    Ok(release_state(
        &releases,
        env!("CARGO_PKG_VERSION"),
        runtime::os(),
        runtime::arch(),
    ))
}
pub fn download(store: &Store, net: &Net, state: &Value) -> Result<Value> {
    let asset = &state["asset"];
    let url = s(asset, "browser_download_url");
    if !url.starts_with("https://github.com/Kovtyndumas/NovaLauncher/releases/download/") {
        bail!("Неправильный источник обновления");
    }
    let hash = s(asset, "digest")
        .strip_prefix("sha256:")
        .context("Нет SHA-256 обновления")?;
    if hash.len() != 64 || !hash.bytes().all(|b| b.is_ascii_hexdigit()) {
        bail!("Некорректная контрольная сумма установщика");
    }
    let path = store.root.join("updates").join(leaf(s(asset, "name"))?);
    net.download(url, &path, "sha256", hash, number(asset, "size", 0))?;
    Ok(
        json!({"phase":"downloaded","message":"Обновление скачано. Перед установкой Nova сохранит предыдущую версию.","file":path,"sha256":hash,"version":state["version"]}),
    )
}
pub fn install(state: &Value) -> Result<()> {
    let path = PathBuf::from(s(state, "file"));
    if s(state, "phase") != "downloaded" || !path.is_file() {
        bail!("Сначала скачайте обновление");
    }
    if s(state, "sha256").is_empty()
        || !sha(&path, "sha256")?.eq_ignore_ascii_case(s(state, "sha256"))
    {
        bail!("Установщик изменился. Скачайте обновление заново.");
    }
    open::that(path)?;
    Ok(())
}
fn app_bundle(exe: &Path) -> Option<PathBuf> {
    exe.ancestors()
        .find(|p| p.extension().is_some_and(|ext| ext == "app"))
        .map(Path::to_path_buf)
}
fn backup_from(store: &Store, exe: &Path, app: Option<&Path>) -> Result<Value> {
    let id = uuid::Uuid::new_v4().to_string();
    let root = store.root.join("launcher-recovery");
    let target = root.join(&id);
    fs::create_dir_all(&target)?;
    let result = (|| {
        let (launch, binary) = if let Some(app) = app {
            let launch = target.join("Nova Previous.app");
            copy_tree(app, &launch, &[])?;
            let relative = exe.strip_prefix(app)?;
            (launch.clone(), launch.join(relative))
        } else {
            let name = if cfg!(windows) {
                "Nova Previous.exe"
            } else {
                "nova-previous"
            };
            let launch = target.join(name);
            fs::copy(exe, &launch)?;
            (launch.clone(), launch)
        };
        let manifest = json!({"id":id,"version":env!("CARGO_PKG_VERSION"),"at":now(),"launch":launch.strip_prefix(&root)?.to_string_lossy(),"binary":binary.strip_prefix(&root)?.to_string_lossy(),"sha256":sha(&binary,"sha256")?});
        write_json(&target.join("manifest.json"), &manifest)?;
        atomic(&root.join("ВОССТАНОВЛЕНИЕ.txt"),"Восстановление Nova\n\nВ каждой папке находится предыдущая версия приложения. Откройте Nova Previous.app (Mac), Nova Previous.exe (Windows) или nova-previous (Linux). На Linux разрешите исполнение файла. Закройте новую Nova и перенесите сохранённую версию обратно на её место. Игровые данные при сохранении приложения не откатываются.\n".as_bytes())?;
        write_json(&root.join("current.json"), &manifest)?;
        Ok(manifest)
    })();
    if result.is_err() {
        let _ = fs::remove_dir_all(target);
    }
    result
}
pub fn backup_current(store: &Store) -> Result<Value> {
    let exe = std::env::current_exe()?;
    let bundle = app_bundle(&exe);
    let original = std::env::var_os("APPIMAGE")
        .map(PathBuf::from)
        .filter(|p| p.is_file());
    backup_from(
        store,
        original.as_deref().unwrap_or(&exe),
        bundle.as_deref(),
    )?;
    recovery_status(store)
}
fn recovery_paths(store: &Store) -> Result<(Value, PathBuf, PathBuf)> {
    let root = store.root.join("launcher-recovery");
    let manifest = read_json(&root.join("current.json"))?;
    let launch = safe(&root, s(&manifest, "launch"))?;
    let binary = safe(&root, s(&manifest, "binary"))?;
    if !binary.is_file()
        || !launch.exists()
        || s(&manifest, "sha256").is_empty()
        || sha(&binary, "sha256")? != s(&manifest, "sha256")
    {
        bail!("Резервная версия Nova отсутствует или повреждена");
    }
    Ok((manifest, launch, binary))
}
pub fn recovery_status(store: &Store) -> Result<Value> {
    let root = store.root.join("launcher-recovery");
    if !root.join("current.json").exists() {
        return Ok(json!({"available":false}));
    }
    let (manifest, launch, _) = recovery_paths(store)?;
    Ok(
        json!({"available":true,"version":manifest["version"],"at":manifest["at"],"path":launch,"folder":root}),
    )
}
pub fn open_recovery(store: &Store) -> Result<Value> {
    let (_, launch, _) = recovery_paths(store)?;
    #[cfg(target_os = "macos")]
    {
        if launch.extension().is_some_and(|ext| ext == "app") {
            std::process::Command::new("open")
                .arg("-n")
                .arg(&launch)
                .spawn()?;
        } else {
            std::process::Command::new(&launch).spawn()?;
        }
    }
    #[cfg(not(target_os = "macos"))]
    {
        std::process::Command::new(&launch).spawn()?;
    }
    Ok(json!(true))
}
pub fn recovery_folder(store: &Store) -> Result<Value> {
    let root = store.root.join("launcher-recovery");
    fs::create_dir_all(&root)?;
    open::that(root)?;
    Ok(Value::Null)
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn never_offers_an_older_release_and_sorts_beta_numerically() {
        let asset = json!({"name":"Nova-arm64.dmg","digest":"sha256:abc"});
        let releases = json!([{"tag_name":"rust-v3.0.0-beta.9","assets":[asset.clone()]},{"tag_name":"rust-v3.0.0-beta.15","assets":[asset.clone()]},{"tag_name":"rust-v3.0.0-beta.10","assets":[asset]}]);
        assert_eq!(
            release_state(&releases, "3.0.0-beta.16", "osx", "aarch64")["phase"],
            "current"
        );
        assert_eq!(
            release_state(&releases, "3.0.0-beta.9", "osx", "aarch64")["version"],
            "rust-v3.0.0-beta.15"
        );
    }
    #[test]
    fn preserved_binary_is_verified_before_recovery() {
        let temp = tempfile::tempdir().unwrap();
        let store = Store::new(temp.path().join("data")).unwrap();
        let exe = temp.path().join("nova");
        fs::write(&exe, b"known working executable").unwrap();
        backup_from(&store, &exe, None).unwrap();
        assert_eq!(recovery_status(&store).unwrap()["available"], true);
        let (_, _, binary) = recovery_paths(&store).unwrap();
        fs::write(binary, b"changed").unwrap();
        assert!(recovery_status(&store).is_err());
        assert!(store
            .root
            .join("launcher-recovery/ВОССТАНОВЛЕНИЕ.txt")
            .exists());
    }
}
