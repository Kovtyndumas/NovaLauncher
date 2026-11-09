use anyhow::{bail, Context, Result};
use serde_json::{json, Value};
use sha2::{Digest, Sha256, Sha512};
use std::{
    fs,
    io::{Read, Write},
    path::{Path, PathBuf},
};
pub fn s<'a>(v: &'a Value, k: &str) -> &'a str {
    v.get(k).and_then(Value::as_str).unwrap_or("")
}
pub fn list<'a>(v: &'a Value, k: &str) -> &'a [Value] {
    v.get(k)
        .and_then(Value::as_array)
        .map(Vec::as_slice)
        .unwrap_or(&[])
}
pub fn flag(v: &Value, k: &str, default: bool) -> bool {
    v.get(k).and_then(Value::as_bool).unwrap_or(default)
}
pub fn number(v: &Value, k: &str, default: u64) -> u64 {
    v.get(k)
        .and_then(|v| {
            v.as_u64()
                .or_else(|| v.as_str().and_then(|s| s.parse().ok()))
        })
        .unwrap_or(default)
}
pub fn read_json(p: &Path) -> Result<Value> {
    Ok(serde_json::from_slice(&fs::read(p).with_context(
        || format!("Не удалось прочитать {}", p.display()),
    )?)?)
}
pub fn write_json(p: &Path, v: &Value) -> Result<()> {
    atomic(p, &serde_json::to_vec_pretty(v)?)
}
pub fn atomic(p: &Path, bytes: &[u8]) -> Result<()> {
    if let Some(dir) = p.parent() {
        fs::create_dir_all(dir)?;
    }
    let temp = p.with_extension(format!("{}.tmp", uuid::Uuid::new_v4()));
    let result = (|| {
        let mut options = fs::OpenOptions::new();
        options.write(true).create_new(true);
        #[cfg(unix)]
        {
            use std::os::unix::fs::OpenOptionsExt;
            options.mode(0o600);
        }
        let mut f = options.open(&temp)?;
        f.write_all(bytes)?;
        f.sync_all()?;
        fs::rename(&temp, p)?;
        Ok(())
    })();
    let _ = fs::remove_file(temp);
    result
}
pub fn leaf(name: &str) -> Result<&str> {
    if name.is_empty() || name == "." || name == ".." || name.contains(['/', '\\', ':', '\0']) {
        bail!("Некорректное имя файла");
    }
    Ok(name)
}
pub fn safe(root: &Path, name: &str) -> Result<PathBuf> {
    let normalized = name.replace('\\', "/");
    if normalized.starts_with('/')
        || normalized.contains(':')
        || normalized.split('/').any(|p| p == "..")
        || normalized.contains('\0')
    {
        bail!("Путь выходит за пределы сборки: {name}");
    }
    let mut dest = root.to_owned();
    if dest.is_symlink() {
        bail!("Ссылка вместо папки");
    }
    for part in normalized.split('/').filter(|x| !x.is_empty() && *x != ".") {
        dest.push(part);
        if dest.is_symlink() {
            bail!("Символическая ссылка в пути");
        }
    }
    Ok(dest)
}
pub fn sha(p: &Path, kind: &str) -> Result<String> {
    let mut file = fs::File::open(p)?;
    let mut buf = [0u8; 65536];
    macro_rules! digest {
        ($t:ty) => {{
            let mut d = <$t>::new();
            loop {
                let n = file.read(&mut buf)?;
                if n == 0 {
                    break;
                }
                d.update(&buf[..n]);
            }
            format!("{:x}", d.finalize())
        }};
    }
    Ok(match kind {
        "sha1" => digest!(sha1::Sha1),
        "sha256" => digest!(Sha256),
        _ => digest!(Sha512),
    })
}
pub fn copy_tree(src: &Path, dst: &Path, excluded: &[&str]) -> Result<()> {
    if !src.exists() {
        return Ok(());
    }
    if src.is_symlink() {
        return Ok(());
    }
    if src.is_file() {
        if let Some(p) = dst.parent() {
            fs::create_dir_all(p)?;
        }
        fs::copy(src, dst)?;
        return Ok(());
    }
    fs::create_dir_all(dst)?;
    for item in fs::read_dir(src)? {
        let item = item?;
        if excluded.contains(&item.file_name().to_string_lossy().as_ref()) {
            continue;
        }
        copy_tree(&item.path(), &dst.join(item.file_name()), excluded)?;
    }
    Ok(())
}
pub fn unzip(archive: &Path, target: &Path, prefix: &str) -> Result<()> {
    let mut zip = zip::ZipArchive::new(fs::File::open(archive)?)?;
    let mut total = 0u64;
    if zip.len() > 250000 {
        bail!("Слишком много файлов");
    }
    for n in 0..zip.len() {
        let mut file = zip.by_index(n)?;
        let name = file.name().to_owned();
        if !name.starts_with(prefix) {
            continue;
        }
        if file
            .unix_mode()
            .map(|m| m & 0o170000 == 0o120000)
            .unwrap_or(false)
        {
            bail!("Ссылки в ZIP не поддерживаются");
        }
        let relative = &name[prefix.len()..];
        if relative.is_empty() {
            continue;
        }
        let p = safe(target, relative)?;
        total = total.saturating_add(file.size());
        if total > 32 * 1024 * 1024 * 1024 {
            bail!("Слишком большой архив");
        }
        if file.is_dir() {
            fs::create_dir_all(p)?;
        } else {
            fs::create_dir_all(p.parent().unwrap())?;
            let mut out = fs::File::create(&p)?;
            std::io::copy(&mut file, &mut out)?;
            #[cfg(unix)]
            if let Some(mode) = file.unix_mode() {
                use std::os::unix::fs::PermissionsExt;
                fs::set_permissions(&p, fs::Permissions::from_mode(mode & 0o777))?;
            }
        }
    }
    Ok(())
}
pub fn tar_gz(archive: &Path, target: &Path) -> Result<()> {
    let mut archive = tar::Archive::new(flate2::read::GzDecoder::new(fs::File::open(archive)?));
    for entry in archive.entries()? {
        let mut entry = entry?;
        let name = entry.path()?.to_string_lossy().to_string();
        let dest = safe(target, &name)?;
        if entry.header().entry_type().is_symlink() || entry.header().entry_type().is_hard_link() {
            continue;
        }
        entry.unpack(dest)?;
    }
    Ok(())
}
pub fn zip_tree(src: &Path, target: &Path) -> Result<()> {
    fs::create_dir_all(target.parent().unwrap())?;
    let mut zip = zip::ZipWriter::new(fs::File::create(target)?);
    fn add(zip: &mut zip::ZipWriter<fs::File>, root: &Path, p: &Path) -> Result<()> {
        for e in fs::read_dir(p)? {
            let p = e?.path();
            if p.is_symlink() {
                continue;
            }
            if p.is_dir() {
                add(zip, root, &p)?;
            } else {
                let n = p.strip_prefix(root)?.to_string_lossy().replace('\\', "/");
                zip.start_file(
                    n,
                    zip::write::SimpleFileOptions::default()
                        .compression_method(zip::CompressionMethod::Deflated),
                )?;
                let mut f = fs::File::open(p)?;
                std::io::copy(&mut f, zip)?;
            }
        }
        Ok(())
    }
    add(&mut zip, src, src)?;
    zip.finish()?;
    Ok(())
}
pub fn now() -> String {
    chrono::Utc::now().to_rfc3339()
}
pub fn report(log: &str, code: i32, id: &str) -> Value {
    let low = log.to_lowercase();
    let (title, advice) = if low.contains("требуется вход horizon") || low.contains("or sign in with the microsoft account") || low.contains("no signed-in account owns minecraft") {
        ("Требуется вход в Horizon", "Для 26.3 RUST нажмите «Войти в Horizon» и подтвердите Microsoft-аккаунт с Minecraft Java Edition. Этот вход сохраняется для всех RUST-сборок.")
    } else if low.contains("outofmemoryerror") {
        (
            "Недостаточно памяти",
            "Измените RAM в настройках или закройте лишние приложения.",
        )
    } else if low.contains("unsupportedclassversionerror") {
        (
            "Неподходящая Java",
            "Очистите ручной путь к Java: Nova выберет нужную версию.",
        )
    } else if low.contains("glfw error")
        || low.contains("failed to create window")
        || low.contains("failed to initialize opengl")
    {
        (
            "Ошибка графики",
            "Проверьте драйвер, отключите шейдеры, проверьте архитектуру Java.",
        )
    } else if low.contains("unsatisfiedlinkerror") {
        (
            "Ошибка нативной библиотеки",
            "Архитектура Java должна совпадать с архитектурой библиотек игры.",
        )
    } else if low.contains("modresolutionexception") || low.contains("incompatible mod set") {
        (
            "Несовместимые моды",
            "Запустите проверку совместимости и проверьте зависимости.",
        )
    } else {
        ("Причина не определена","Смотрите последние строки журнала и crash-reports. Код завершения не определяет причину.")
    };
    json!({"instanceId":id,"code":code,"title":title,"advice":advice,"evidence":log.lines().rev().take(8).collect::<Vec<_>>(),"at":now()})
}
