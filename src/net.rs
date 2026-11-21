use crate::util::*;
use anyhow::{bail, Result};
use serde_json::Value;
use std::{fs, io::{Read, Write}, path::Path, sync::Arc, time::{Duration, Instant}};
#[derive(Clone)]
pub struct Net(pub reqwest::blocking::Client, pub Arc<crate::downloads::Downloads>);
impl Net {
    pub fn new() -> Result<Self> {
        Ok(Self(reqwest::blocking::Client::builder().user_agent("Kovtyndumas/NovaLauncher/3.0.0-rust (https://github.com/Kovtyndumas/NovaLauncher)").connect_timeout(Duration::from_secs(20)).timeout(Duration::from_secs(180)).build()?, Arc::new(crate::downloads::Downloads::default())))
    }
    pub fn json(&self, url: &str) -> Result<Value> {
        self.1.online()?;
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
        self.1.online()?;
        let r = self.0.post(url).json(data).send()?;
        if !r.status().is_success() {
            bail!("API: HTTP {}", r.status().as_u16());
        }
        Ok(r.json()?)
    }
    pub fn form(&self, url: &str, data: &[(&str, &str)]) -> Result<Value> {
        self.1.online()?;
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
        self.1.online().map_err(|_| anyhow::anyhow!("Без интернета: файл {} отсутствует или повреждён. Подключитесь и восстановите файлы сборки.", dest.file_name().unwrap_or_default().to_string_lossy()))?;
        let mut task=self.1.begin(&dest.file_name().unwrap_or_default().to_string_lossy(),size);
        task.check()?;
        fs::create_dir_all(dest.parent().unwrap())?;
        let tmp = dest.with_extension(format!("{}.part", uuid::Uuid::new_v4()));
        let result = (|| {
            let mut r = self.0.get(url).send()?;
            if !r.status().is_success() {
                bail!("Загрузка: HTTP {}", r.status().as_u16());
            }
            let mut f = fs::File::create(&tmp)?;
            let total=if size>0 {size} else {r.content_length().unwrap_or(0)};
            let mut buffer=[0u8;8192];let mut received=0;let mut last=Instant::now();
            loop {
                task.check()?;
                let count=r.read(&mut buffer)?;if count==0 {break;}
                task.throttle(count)?;f.write_all(&buffer[..count])?;received+=count as u64;
                if last.elapsed()>=Duration::from_millis(150) {task.progress(received,total);last=Instant::now();}
            }
            task.progress(received,total);
            if total>0&&received!=total {bail!("Неполный файл");}
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
        let error=result.as_ref().err().map(|e|format!("{e:#}"));task.finish(error.as_deref());
        result
    }
}
pub fn enc(s: &str) -> String {
    url::form_urlencoded::byte_serialize(s.as_bytes()).collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    #[test]
    fn offline_uses_verified_cache_and_rejects_missing_or_corrupt_files() {
        let temp=tempfile::tempdir().unwrap();let file=temp.path().join("client.jar");
        fs::write(&file,b"verified content").unwrap();let hash=sha(&file,"sha1").unwrap();
        let net=Net::new().unwrap();net.1.configure(&json!({"offlineMode":true}));
        assert!(net.download("https://invalid.invalid",&file,"sha1",&hash,16).is_ok());
        fs::write(&file,b"corrupt").unwrap();
        assert!(net.download("https://invalid.invalid",&file,"sha1",&hash,16).unwrap_err().to_string().contains("Без интернета"));
        assert_eq!(fs::read(&file).unwrap(),b"corrupt");
        assert!(net.download("https://invalid.invalid",&temp.path().join("missing"),"sha1",&hash,16).is_err());
        assert!(net.json("https://invalid.invalid").unwrap_err().to_string().contains("без интернета"));
    }
    #[test]
    fn download_checks_checksum_before_replacing_a_file() {
        use std::net::TcpListener;
        let temp=tempfile::tempdir().unwrap();let file=temp.path().join("mod.jar");fs::write(&file,b"original").unwrap();
        let listener=TcpListener::bind("127.0.0.1:0").unwrap();let address=listener.local_addr().unwrap();
        let server=std::thread::spawn(move||{let(mut socket,_)=listener.accept().unwrap();socket.set_read_timeout(Some(Duration::from_secs(3))).unwrap();let mut request=[0;4096];let _=socket.read(&mut request).unwrap();socket.write_all(b"HTTP/1.1 200 OK\r\nContent-Length: 5\r\nConnection: close\r\n\r\nwrong").unwrap();});
        let net=Net::new().unwrap();assert!(net.download(&format!("http://{address}"),&file,"sha1","0000000000000000000000000000000000000000",5).is_err());server.join().unwrap();
        assert_eq!(fs::read(&file).unwrap(),b"original");assert_eq!(net.1.status()["tasks"][0]["phase"],"failed");
        assert_eq!(fs::read_dir(temp.path()).unwrap().count(),1);
    }
}
