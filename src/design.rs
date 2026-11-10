use anyhow::{bail, Context, Result};
use base64::{engine::general_purpose::STANDARD, Engine};
use serde_json::{json, Value};
use std::{fs, path::Path};
const LIMIT: u64 = 8 * 1024 * 1024;
pub fn wallpaper(path: &Path) -> Result<Value> {
    if fs::metadata(path)?.len() > LIMIT {
        bail!("Изображение должно быть меньше 8 МБ");
    }
    let bytes = fs::read(path)?;
    let kind = if bytes.starts_with(b"\x89PNG\r\n\x1a\n") {
        "png"
    } else if bytes.starts_with(&[255, 216, 255]) {
        "jpeg"
    } else if bytes.starts_with(b"RIFF") && bytes.get(8..12) == Some(b"WEBP") {
        "webp"
    } else {
        bail!("Выберите PNG, JPEG или WebP");
    };
    Ok(json!(format!(
        "data:image/{kind};base64,{}",
        STANDARD.encode(bytes)
    )))
}
// A theme is data, never CSS, JavaScript, account settings, or a path to a file.
pub fn clean(value: &Value) -> Result<Value> {
    if value["format"] != "nova-design" || value["version"] != 1 {
        bail!("Неподдерживаемый файл оформления Nova");
    }
    let input = value["appearance"]
        .as_object()
        .context("Нет настроек оформления")?;
    let mut appearance = json!({});
    for key in [
        "accent",
        "radius",
        "compact",
        "wallpaper",
        "character",
        "stats",
        "shortcuts",
        "layout",
        "modules",
        "layoutVersion",
        "wallpaperImage",
        "wallpaperBlur",
        "wallpaperDim",
        "moduleSettings",
    ] {
        if let Some(v) = input.get(key) {
            appearance[key] = v.clone();
        }
    }
    // Only known module settings survive; don't share server addresses with a design.
    if let Some(settings) = appearance["moduleSettings"].as_object_mut() {
        settings.retain(|k, _| {
            [
                "info",
                "actions",
                "character",
                "stats",
                "favorite",
                "history",
                "quick",
                "memory",
            ]
            .contains(&k.as_str())
        });
        for v in settings.values_mut() {
            if let Some(map) = v.as_object_mut() {
                map.retain(|k, _| ["textScale", "opacity", "details"].contains(&k.as_str()));
            } else {
                *v = json!({});
            }
        }
    }
    if let Some(image) = appearance["wallpaperImage"].as_str() {
        if !image.is_empty() {
            let Some(data) = image.strip_prefix("data:image/jpeg;base64,") else {
                bail!("Обои в оформлении должны быть JPEG");
            };
            if data.len() > 3 * 1024 * 1024 {
                bail!("Слишком большие обои");
            }
            let bytes = STANDARD.decode(data).context("Повреждённое изображение")?;
            if !bytes.starts_with(&[255, 216, 255]) {
                bail!("Повреждённое изображение");
            }
        }
    }
    let theme = match value["theme"].as_str() {
        Some("light") => "light",
        Some("graphite") => "graphite",
        _ => "black",
    };
    Ok(
        json!({"format":"nova-design","version":1,"name":value["name"].as_str().unwrap_or("Мой экран").chars().take(60).collect::<String>(),"theme":theme,"appearance":appearance}),
    )
}
pub fn import(path: &Path) -> Result<Value> {
    if fs::metadata(path)?.len() > LIMIT {
        bail!("Файл оформления больше 8 МБ");
    }
    clean(&serde_json::from_slice(&fs::read(path)?)?)
}
pub fn export(path: &Path, value: &Value) -> Result<Value> {
    let data = clean(value)?;
    crate::util::write_json(path, &data)?;
    Ok(json!(true))
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn design_never_exports_credentials_or_hosts() {
        let v=clean(&json!({"format":"nova-design","version":1,"theme":"light","settings":{"msRefresh":"secret"},"appearance":{"accent":"#0a84ff","msRefresh":"secret","moduleSettings":{"favorite":{"serverHost":"private","details":true,"script":"evil"}}}})).unwrap();
        assert!(!v.to_string().contains("secret"));
        assert!(!v.to_string().contains("private"));
        assert!(!v.to_string().contains("evil"));
        assert_eq!(v["theme"], "light");
    }
    #[test]
    fn design_rejects_external_images_and_unknown_format() {
        assert!(clean(&json!({"format":"nova-design","version":1,"appearance":{"wallpaperImage":"https://evil"}})).is_err());
        assert!(clean(&json!({"format":"other","version":1,"appearance":{}})).is_err());
    }
}
