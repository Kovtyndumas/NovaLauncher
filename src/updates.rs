use crate::{net::Net, runtime, store::Store, util::*};
use anyhow::{bail, Result};
use serde_json::{json, Value};
use std::path::PathBuf;
pub fn check(net: &Net) -> Result<Value> {
    let releases =
        net.json("https://api.github.com/repos/Kovtyndumas/NovaLauncher/releases?per_page=30")?;
    for r in releases.as_array().unwrap_or(&vec![]) {
        let tag = s(r, "tag_name");
        if !tag.starts_with("rust-v") || flag(r, "draft", false) {
            continue;
        }
        if tag == concat!("rust-v", env!("CARGO_PKG_VERSION")) {
            return Ok(json!({"phase":"current","message":"Установлена актуальная Rust-версия"}));
        }
        for a in list(r, "assets") {
            let name = s(a, "name");
            let platform = match runtime::os() {
                "osx" => {
                    name.ends_with(".dmg")
                        && (if runtime::arch() == "aarch64" {
                            name.contains("aarch64") || name.contains("arm64")
                        } else {
                            name.contains("x64")
                                || name.contains("x86_64")
                                || name.contains("intel")
                        })
                }
                "windows" => name.ends_with(".exe"),
                _ => name.ends_with(".AppImage") || name.ends_with(".deb"),
            };
            if platform && s(a, "digest").starts_with("sha256:") {
                return Ok(
                    json!({"phase":"available","message":format!("Доступна Nova Rust {tag}"),"version":tag,"asset":a,"url":s(r,"html_url")}),
                );
            }
        }
        return Ok(
            json!({"phase":"current","message":"Нет установщика Rust-версии для этой платформы","url":s(r,"html_url")}),
        );
    }
    Ok(json!({"phase":"current","message":"Rust-релиз ещё не опубликован · локальная Rust-версия"}))
}
pub fn download(store: &Store, net: &Net, state: &Value) -> Result<Value> {
    let a = &state["asset"];
    let url = s(a, "browser_download_url");
    if !url.starts_with("https://github.com/Kovtyndumas/NovaLauncher/releases/download/") {
        bail!("Неправильный источник обновления");
    }
    let hash = s(a, "digest")
        .strip_prefix("sha256:")
        .ok_or_else(|| anyhow::anyhow!("Нет SHA-256 обновления"))?;
    let p = store.root.join("updates").join(leaf(s(a, "name"))?);
    net.download(url, &p, "sha256", hash, number(a, "size", 0))?;
    Ok(json!({"phase":"downloaded","message":"Обновление скачано. Откройте установщик.","file":p}))
}
pub fn install(state: &Value) -> Result<()> {
    let p = PathBuf::from(s(state, "file"));
    if s(state, "phase") != "downloaded" || !p.is_file() {
        bail!("Сначала скачайте обновление");
    }
    open::that(p)?;
    Ok(())
}
