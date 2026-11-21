use anyhow::{bail, Result};
use serde_json::{json, Value};
pub const KEYS: &[&str] = &["ram", "java", "autoJava", "jvm", "ww", "wh", "full"];
pub fn clean(value: &Value) -> Result<Value> {
    let object = value
        .as_object()
        .ok_or_else(|| anyhow::anyhow!("Некорректные параметры запуска"))?;
    let mut result = json!({});
    for (key, value) in object {
        match key.as_str() {
            "ram" | "ww" | "wh" => {
                let number = value
                    .as_u64()
                    .ok_or_else(|| anyhow::anyhow!("{key}: нужно целое число"))?;
                let (min, max) = match key.as_str() {
                    "ram" => (1, 64),
                    "ww" => (0, 7680),
                    _ => (0, 4320),
                };
                if !(min..=max).contains(&number) {
                    bail!("{key}: допустимо {min}–{max}");
                }
            }
            "java" | "jvm" => {
                if !value.is_string() || value.as_str().unwrap().len() > 8192 {
                    bail!("Некорректное значение {key}");
                }
            }
            "autoJava" | "full" => {
                if !value.is_boolean() {
                    bail!("Некорректное значение {key}");
                }
            }
            _ => bail!("Неизвестный параметр запуска: {key}"),
        }
        result[key] = value.clone();
    }
    Ok(result)
}
pub fn effective(settings: &Value, instance: &Value) -> Value {
    let mut result = settings.clone();
    if let Some(overrides) = instance["launchSettings"].as_object() {
        for key in KEYS {
            if let Some(value) = overrides.get(*key) {
                result[*key] = value.clone();
            }
        }
    }
    result
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn profiles_inherit_and_cannot_override_identity_or_network() {
        let global = json!({"ram":4,"java":"global","name":"Steve","offlineMode":true});
        let merged = effective(&global, &json!({"launchSettings":{"ram":8,"name":"bad"}}));
        assert_eq!(merged["ram"], 8);
        assert_eq!(merged["name"], "Steve");
        assert_eq!(merged["java"], "global");
        assert!(clean(&json!({"ram":0})).is_err());
        assert!(clean(&json!({"ram":3.5})).is_err());
        assert!(clean(&json!({"token":"secret"})).is_err());
        assert_eq!(clean(&json!({})).unwrap(), json!({}));
    }
}
