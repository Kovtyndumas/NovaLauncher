use crate::util::*;
use anyhow::{bail, Result};
use serde_json::Value;
use std::{
    fs,
    io::{Read, Write},
    path::Path,
    sync::Arc,
    time::{Duration, Instant},
};
#[derive(Clone)]
pub struct Net(
    pub reqwest::blocking::Client,
    pub Arc<crate::downloads::Downloads>,
);
impl Net {
    pub fn new() -> Result<Self> {
        Ok(Self(reqwest::blocking::Client::builder().user_agent("Kovtyndumas/NovaLauncher/3.0.0-rust (https://github.com/Kovtyndumas/NovaLauncher)").connect_timeout(Duration::from_secs(20)).timeout(Duration::from_secs(180)).build()?, Arc::new(crate::downloads::Downloads::default())))
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
        self.download_named(
            url,
            dest,
            kind,
            hash,
            size,
            &dest.file_name().unwrap_or_default().to_string_lossy(),
        )
    }
    pub fn download_named(
        &self,
        url: &str,
        dest: &Path,
        kind: &str,
        hash: &str,
        size: u64,
        name: &str,
    ) -> Result<()> {
        self.1.check()?;
        if dest.is_file()
            && ((!hash.is_empty() && sha(dest, kind)?.eq_ignore_ascii_case(hash))
                || (hash.is_empty() && size > 0 && fs::metadata(dest)?.len() == size))
        {
            return Ok(());
        }
        fs::create_dir_all(dest.parent().unwrap())?;
        let mut task = self.1.begin(name, size);
        let destination = self.1.destination(dest);
        let _destination = destination.lock().unwrap();
        // A stable partial file can be reused only when the final content is verified by hash.
        let tmp = dest.with_file_name(format!(
            "{}.nova.part",
            dest.file_name().unwrap_or_default().to_string_lossy()
        ));
        let result = (|| {
            for attempt in 1..=3 {
                task.check()?;
                task.attempt(attempt);
                let result = (|| {
                    let mut offset = if !hash.is_empty() {
                        fs::metadata(&tmp).map(|m| m.len()).unwrap_or(0)
                    } else {
                        0
                    };
                    if size > 0 && offset >= size {
                        offset = 0;
                    }
                    let mut request = self
                        .0
                        .get(url)
                        .header(reqwest::header::ACCEPT_ENCODING, "identity");
                    if offset > 0 {
                        request =
                            request.header(reqwest::header::RANGE, format!("bytes={offset}-"));
                    }
                    let mut response = request.send().map_err(|_| {
                        anyhow::anyhow!("Не удалось подключиться к серверу загрузки")
                    })?;
                    if !response.status().is_success() {
                        bail!("Загрузка: HTTP {}", response.status().as_u16());
                    }
                    if response.status() == reqwest::StatusCode::PARTIAL_CONTENT {
                        let range = response
                            .headers()
                            .get(reqwest::header::CONTENT_RANGE)
                            .and_then(|v| v.to_str().ok())
                            .unwrap_or("");
                        if offset == 0 || !range.starts_with(&format!("bytes {offset}-")) {
                            bail!("Сервер вернул неверный диапазон файла");
                        }
                    } else {
                        offset = 0;
                    }
                    let total = if size > 0 {
                        size
                    } else {
                        response.content_length().unwrap_or(0) + offset
                    };
                    let mut output = fs::OpenOptions::new()
                        .write(true)
                        .create(true)
                        .truncate(offset == 0)
                        .append(offset > 0)
                        .open(&tmp)?;
                    let mut received = offset;
                    let mut buffer = [0u8; 65536];
                    let mut last = Instant::now();
                    task.progress(received, total, 0);
                    loop {
                        task.check()?;
                        let count = response.read(&mut buffer).map_err(|_| {
                            anyhow::anyhow!("Соединение прервано во время загрузки")
                        })?;
                        if count == 0 {
                            break;
                        }
                        output.write_all(&buffer[..count])?;
                        received += count as u64;
                        if size > 0 && received > size {
                            bail!("Размер файла превышает ожидаемый");
                        }
                        if last.elapsed() >= Duration::from_millis(150) {
                            task.progress(received, total, received - offset);
                            last = Instant::now();
                        }
                    }
                    task.check()?;
                    output.sync_all()?;
                    drop(output);
                    if total > 0 && received != total {
                        bail!("Неполный файл");
                    }
                    task.progress(received, total, received - offset);
                    task.phase("verifying");
                    if !hash.is_empty() && !sha(&tmp, kind)?.eq_ignore_ascii_case(hash) {
                        let _ = fs::remove_file(&tmp);
                        bail!("Не совпала контрольная сумма");
                    }
                    task.check()?;
                    fs::rename(&tmp, dest)?;
                    Ok(())
                })();
                match result {
                    Ok(()) => return Ok(()),
                    Err(error) => {
                        if task.cancelled() || attempt == 3 {
                            return Err(error);
                        }
                        task.phase("retrying");
                        for _ in 0..attempt * 5 {
                            task.check()?;
                            std::thread::sleep(Duration::from_millis(100));
                        }
                    }
                }
            }
            unreachable!()
        })();
        if hash.is_empty() {
            let _ = fs::remove_file(&tmp);
        }
        let message = result.as_ref().err().map(|e| format!("{e:#}"));
        task.finish(message.as_deref());
        result
    }
}
pub fn enc(s: &str) -> String {
    url::form_urlencoded::byte_serialize(s.as_bytes()).collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{
        net::{TcpListener, TcpStream},
        thread,
    };
    fn accept(listener: &TcpListener) -> TcpStream {
        listener.set_nonblocking(true).unwrap();
        let start = Instant::now();
        loop {
            match listener.accept() {
                Ok((s, _)) => {
                    s.set_read_timeout(Some(Duration::from_secs(3))).unwrap();
                    return s;
                }
                Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                    assert!(
                        start.elapsed() < Duration::from_secs(5),
                        "No download request"
                    );
                    thread::sleep(Duration::from_millis(10));
                }
                Err(e) => panic!("{e}"),
            }
        }
    }
    fn request(stream: &mut TcpStream) -> String {
        let mut result = vec![];
        let mut byte = [0];
        while !result.ends_with(b"\r\n\r\n") {
            assert_eq!(stream.read(&mut byte).unwrap(), 1);
            result.push(byte[0]);
            assert!(result.len() < 16384);
        }
        String::from_utf8(result).unwrap().to_lowercase()
    }
    fn fixture() -> (tempfile::TempDir, std::path::PathBuf, String) {
        let t = tempfile::tempdir().unwrap();
        let file = t.path().join("file.jar");
        fs::write(t.path().join("expected"), b"abcdef").unwrap();
        let hash = sha(&t.path().join("expected"), "sha512").unwrap();
        (t, file, hash)
    }
    #[test]
    fn interrupted_download_retries_with_validated_range() {
        let (_t, file, hash) = fixture();
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let url = format!("http://{}/file", listener.local_addr().unwrap());
        let server = thread::spawn(move || {
            let mut first = accept(&listener);
            request(&mut first);
            first
                .write_all(b"HTTP/1.1 200 OK\r\nContent-Length: 6\r\nConnection: close\r\n\r\nabc")
                .unwrap();
            drop(first);
            let mut second = accept(&listener);
            assert!(request(&mut second).contains("range: bytes=3-"));
            second.write_all(b"HTTP/1.1 206 Partial Content\r\nContent-Length: 3\r\nContent-Range: bytes 3-5/6\r\nConnection: close\r\n\r\ndef").unwrap();
        });
        let net = Net::new().unwrap();
        net.download(&url, &file, "sha512", &hash, 6).unwrap();
        server.join().unwrap();
        assert_eq!(fs::read(&file).unwrap(), b"abcdef");
        assert_eq!(net.1.status()[0]["phase"], "completed");
        assert_eq!(net.1.status()[0]["attempt"], 2);
        assert!(!file.with_file_name("file.jar.nova.part").exists());
    }
    #[test]
    fn range_ignored_by_server_restarts_instead_of_appending() {
        let (_t, file, hash) = fixture();
        fs::write(file.with_file_name("file.jar.nova.part"), b"abc").unwrap();
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let url = format!("http://{}/file", listener.local_addr().unwrap());
        let server = thread::spawn(move || {
            let mut stream = accept(&listener);
            assert!(request(&mut stream).contains("range: bytes=3-"));
            stream
                .write_all(
                    b"HTTP/1.1 200 OK\r\nContent-Length: 6\r\nConnection: close\r\n\r\nabcdef",
                )
                .unwrap();
        });
        Net::new()
            .unwrap()
            .download(&url, &file, "sha512", &hash, 6)
            .unwrap();
        server.join().unwrap();
        assert_eq!(fs::read(file).unwrap(), b"abcdef");
    }
    #[test]
    fn repeating_a_failed_operation_reuses_the_verified_partial_path() {
        let (_temp, file, hash) = fixture();
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let url = format!("http://{}/file", listener.local_addr().unwrap());
        let server = thread::spawn(move || {
            let mut first = accept(&listener);
            request(&mut first);
            first
                .write_all(b"HTTP/1.1 200 OK\r\nContent-Length: 6\r\nConnection: close\r\n\r\nabc")
                .unwrap();
            drop(first);
            for _ in 0..2 {
                let mut stream = accept(&listener);
                assert!(request(&mut stream).contains("range: bytes=3-"));
                stream.write_all(b"HTTP/1.1 503 Service Unavailable\r\nContent-Length: 0\r\nConnection: close\r\n\r\n").unwrap();
            }
            let mut retry = accept(&listener);
            assert!(request(&mut retry).contains("range: bytes=3-"));
            retry.write_all(b"HTTP/1.1 206 Partial Content\r\nContent-Length: 3\r\nContent-Range: bytes 3-5/6\r\nConnection: close\r\n\r\ndef").unwrap();
        });
        let net = Net::new().unwrap();
        assert!(net
            .download_named(&url, &file, "sha512", &hash, 6, "Example mod")
            .is_err());
        assert_eq!(
            fs::read(file.with_file_name("file.jar.nova.part")).unwrap(),
            b"abc"
        );
        net.download_named(&url, &file, "sha512", &hash, 6, "Example mod")
            .unwrap();
        server.join().unwrap();
        assert_eq!(fs::read(&file).unwrap(), b"abcdef");
        assert_eq!(net.1.status()[0]["name"], "Example mod");
        assert_eq!(net.1.status()[1]["phase"], "failed");
    }
    #[test]
    fn invalid_checksum_never_replaces_existing_file() {
        let (_t, file, hash) = fixture();
        fs::write(&file, b"original").unwrap();
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let url = format!("http://{}/file", listener.local_addr().unwrap());
        let server = thread::spawn(move || {
            for _ in 0..3 {
                let mut stream = accept(&listener);
                request(&mut stream);
                stream
                    .write_all(
                        b"HTTP/1.1 200 OK\r\nContent-Length: 6\r\nConnection: close\r\n\r\nwrong!",
                    )
                    .unwrap();
            }
        });
        let net = Net::new().unwrap();
        assert!(net.download(&url, &file, "sha512", &hash, 6).is_err());
        server.join().unwrap();
        assert_eq!(fs::read(file).unwrap(), b"original");
        assert_eq!(net.1.status()[0]["phase"], "failed");
    }
}
