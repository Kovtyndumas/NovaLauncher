use anyhow::{bail, Result};
use serde_json::{json, Value};
pub const KEYS: &[&str] = &["ram", "java", "autoJava", "jvm", "ww", "wh", "full"];
pub fn clean(value: &Value) -> Result<Value> {
    let map = value
        .as_object()
        .ok_or_else(|| anyhow::anyhow!("Некорректные настройки запуска"))?;
    let mut result = json!({});
    for (key, value) in map {
        if !KEYS.contains(&key.as_str()) {
            bail!("Неизвестная настройка запуска: {key}");
        }
        if value.is_null() {
            continue;
        }
        match key.as_str() {
            "ram" => {
                if !value.as_u64().is_some_and(|n| (1..=64).contains(&n)) {
                    bail!("RAM: от 1 до 64 ГБ");
                }
            }
            "ww" | "wh" => {
                let (min, max) = if key == "ww" {
                    (320, 7680)
                } else {
                    (240, 4320)
                };
                if !value
                    .as_u64()
                    .is_some_and(|n| n == 0 || (min..=max).contains(&n))
                {
                    bail!("Некорректный размер окна");
                }
            }
            "autoJava" | "full" => {
                if !value.is_boolean() {
                    bail!("Некорректный переключатель");
                }
            }
            _ => {
                if !value
                    .as_str()
                    .is_some_and(|s| s.len() <= 8192 && !s.contains('\0'))
                {
                    bail!("Некорректная строка настройки");
                }
            }
        }
        result[key] = value.clone();
    }
    Ok(result)
}
pub fn effective(global: &Value, inst: &Value) -> Value {
    let mut result = global.clone();
    if let Some(settings) = inst["launchSettings"].as_object() {
        for key in KEYS {
            if let Some(value) = settings.get(*key).filter(|v| !v.is_null()) {
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
    fn profiles_inherit_and_never_override_identity() {
        let global = json!({"ram":4,"name":"Account","java":"","full":false});
        let inst = json!({"launchSettings":{"ram":8,"full":true,"name":"Forged"}});
        let merged = effective(&global, &inst);
        assert_eq!(merged["ram"], 8);
        assert_eq!(merged["name"], "Account");
        assert_eq!(merged["java"], "");
        assert!(clean(&inst["launchSettings"]).is_err());
    }
    #[test]
    fn rejects_invalid_values_and_null_means_inherit() {
        for value in [
            json!({"ram":0}),
            json!({"ram":65}),
            json!({"full":"true"}),
            json!({"ww":5}),
            json!({"java":"bad\u{0000}path"}),
        ] {
            assert!(clean(&value).is_err());
        }
        assert_eq!(
            clean(&json!({"ram":null,"ww":0,"autoJava":true})).unwrap(),
            json!({"ww":0,"autoJava":true})
        );
    }
}
