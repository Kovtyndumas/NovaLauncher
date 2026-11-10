use crate::{store::Store, util::*};
use anyhow::{bail, Result};
use serde_json::{json, Value};
use std::io::Read;
pub fn redact(text: &str, home: Option<&str>, instance: Option<&str>) -> String {
    let mut value = text.to_owned();
    if let Some(path) = instance.filter(|s| !s.is_empty()) {
        value = value.replace(path, "[INSTANCE]");
    }
    if let Some(path) = home.filter(|s| !s.is_empty()) {
        value = value.replace(path, "[HOME]");
    }
    for (pattern, replacement) in [
        (r#"(?i)(bearer\s+)[A-Za-z0-9._~+/=-]+"#, "$1[REDACTED]"),
        (
            r#"(?i)((?:access[_-]?token|refresh[_-]?token|msRefresh|client[_-]?secret|api[_-]?key|authorization)[\"']?\s*(?:[:=]|\s)\s*[\"']?)[^\s\"',;}]+"#,
            "$1[REDACTED]",
        ),
        (r#"(?i)token:[^\s\"']+"#, "token:[REDACTED]"),
        (
            r#"eyJ[A-Za-z0-9_-]+\.[A-Za-z0-9_-]+\.[A-Za-z0-9_-]+"#,
            "[REDACTED-JWT]",
        ),
        (r#"/Users/[^/\s:;\"']+|/home/[^/\s:;\"']+"#, "[HOME]"),
        (r#"(?i)[a-z]:\\Users\\[^\\\s:;\"']+"#, "[HOME]"),
    ] {
        value = regex::Regex::new(pattern)
            .unwrap()
            .replace_all(&value, replacement)
            .into_owned();
    }
    value
}
pub fn report(store: &Store, id: &str, provided: &str) -> Result<Value> {
    let inst = store.get(id)?;
    let root = store.dir(id)?;
    if provided.len() > 1024 * 1024 {
        bail!("Слишком большой фрагмент журнала");
    }
    let mut logs = provided.to_owned();
    if logs.is_empty() {
        if let Ok(file) = std::fs::File::open(root.join("nova-last-launch.log")) {
            logs.clear();
            file.take(1024 * 1024).read_to_string(&mut logs)?;
        }
    }
    let settings = store.game_settings(&inst);
    let text = format!(
        "Nova {}\n{} · {}\nMinecraft {} · {}\nRAM {} GB · custom Java: {}\n\n{}",
        env!("CARGO_PKG_VERSION"),
        std::env::consts::OS,
        std::env::consts::ARCH,
        s(&inst, "version"),
        s(&inst, "loader"),
        number(&settings, "ram", 4),
        !s(&settings, "java").is_empty(),
        logs
    );
    let home = dirs::home_dir().map(|p| p.to_string_lossy().to_string());
    let clean = redact(&text, home.as_deref(), Some(&root.to_string_lossy()));
    Ok(
        json!({"text":clean,"notice":"Токены, папка сборки и имена пользователей в путях скрыты. Проверьте текст перед отправкой: сообщения модов могут содержать другие личные данные."}),
    )
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn removes_tokens_and_private_user_paths() {
        let text = r#"--accessToken SECRET auth_access_token=ALSOSECRET {"refresh_token":"REFRESH"} Authorization: Bearer ABC123 apiKey=PRIVATE token:abc:uuid /Users/Alice/game C:\Users\Bob\game /home/carol/files"#;
        let clean = redact(text, None, None);
        for secret in [
            "SECRET",
            "ALSOSECRET",
            "REFRESH",
            "ABC123",
            "PRIVATE",
            "Alice",
            "Bob",
            "carol",
            "abc:uuid",
        ] {
            assert!(!clean.contains(secret), "Not removed: {secret}: {clean}");
        }
    }
}
