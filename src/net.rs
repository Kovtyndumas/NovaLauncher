use crate::util::*;
use anyhow::{bail, Result};
use serde_json::Value;
use std::{fs, io::Write, path::Path, time::Duration};
#[derive(Clone)]
pub struct Net(pub reqwest::blocking::Client);
impl Net {
    pub fn new() -> Result<Self> {
        Ok(Self(reqwest::blocking::Client::builder().user_agent("Kovtyndumas/NovaLauncher/3.0.0-rust (https://github.com/Kovtyndumas/NovaLauncher)").connect_timeout(Duration::from_secs(20)).timeout(Duration::from_secs(180)).build()?))
    }
    pub fn json(&self, url: &str) -> Result<Value> {
        let r = self.0.get(url).send()?;
        let status = r.status();
        if !status.is_success() {
            bail!(
                "HTTP {} · {}",
                status.as_u16(),
                url::Url::parse(url)?.host_str().unwrap_or("API")
            );
        }
        Ok(r.json()?)
    }
    pub fn post(&self, url: &str, data: &Value) -> Result<Value> {
        let r = self.0.post(url).json(data).send()?;
        if !r.status().is_success() {
            bail!("API: HTTP {}", r.status().as_u16());
        }
        Ok(r.json()?)
    }
    pub fn form(&self, url: &str, data: &[(&str, &str)]) -> Result<Value> {
        let r = self.0.post(url).form(data).send()?;
        if !r.status().is_success() {
            bail!("Вход: HTTP {}", r.status().as_u16());
        }
        Ok(r.json()?)
    }
    pub fn download(
        &self,
        url: &str,
        dest: &Path,
        kind: &str,
        hash: &str,
        size: u64,
    ) -> Result<()> {
        if dest.is_file()
            && ((!hash.is_empty() && sha(dest, kind)?.eq_ignore_ascii_case(hash))
                || (hash.is_empty() && size > 0 && fs::metadata(dest)?.len() == size))
        {
            return Ok(());
        }
        fs::create_dir_all(dest.parent().unwrap())?;
        let tmp = dest.with_extension(format!("{}.part", uuid::Uuid::new_v4()));
        let result = (|| {
            let mut r = self.0.get(url).send()?;
            if !r.status().is_success() {
                bail!("Загрузка: HTTP {}", r.status().as_u16());
            }
            let mut f = fs::File::create(&tmp)?;
            r.copy_to(&mut f)?;
            f.flush()?;
            drop(f);
            if size > 0 && fs::metadata(&tmp)?.len() != size {
                bail!("Неполный файл: {}", dest.display());
            }
            if !hash.is_empty() && !sha(&tmp, kind)?.eq_ignore_ascii_case(hash) {
                bail!("Не совпала контрольная сумма: {}", dest.display());
            }
            fs::rename(&tmp, dest)?;
            Ok(())
        })();
        let _ = fs::remove_file(tmp);
        result
    }
}
pub fn enc(s: &str) -> String {
    url::form_urlencoded::byte_serialize(s.as_bytes()).collect()
}
