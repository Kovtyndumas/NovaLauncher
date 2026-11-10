use crate::{safety, store::Store, util::*};
use anyhow::{bail, Context, Result};
use base64::{engine::general_purpose::STANDARD, Engine};
use serde_json::{json, Value};
use std::{
    fs,
    io::{Cursor, Read},
    path::{Path, PathBuf},
};
struct Nbt {
    data: Cursor<Vec<u8>>,
    nodes: usize,
}
impl Nbt {
    fn bytes<const N: usize>(&mut self) -> Result<[u8; N]> {
        let mut b = [0; N];
        self.data.read_exact(&mut b)?;
        Ok(b)
    }
    fn text(&mut self) -> Result<String> {
        let len = u16::from_be_bytes(self.bytes()?) as usize;
        let mut b = vec![0; len];
        self.data.read_exact(&mut b)?;
        Ok(String::from_utf8_lossy(&b).into_owned())
    }
    fn count(&mut self) -> Result<usize> {
        let n = i32::from_be_bytes(self.bytes()?);
        if !(0..=1000000).contains(&n) {
            bail!("Некорректный размер NBT");
        }
        Ok(n as usize)
    }
    fn tag(&mut self, kind: u8, depth: usize) -> Result<Value> {
        self.nodes += 1;
        if depth > 64 || self.nodes > 1000000 {
            bail!("Слишком сложный NBT");
        }
        Ok(match kind {
            1 => json!(self.bytes::<1>()?[0] as i8),
            2 => json!(i16::from_be_bytes(self.bytes()?)),
            3 => json!(i32::from_be_bytes(self.bytes()?)),
            4 => json!(i64::from_be_bytes(self.bytes()?)),
            5 => json!(f32::from_be_bytes(self.bytes()?)),
            6 => json!(f64::from_be_bytes(self.bytes()?)),
            7 | 11 | 12 => {
                let count = self.count()?;
                let width = match kind {
                    7 => 1,
                    11 => 4,
                    _ => 8,
                };
                let skip = count * width;
                let position = self.data.position() as usize;
                if position
                    .checked_add(skip)
                    .is_none_or(|end| end > self.data.get_ref().len())
                {
                    bail!("Неполный NBT");
                }
                self.data.set_position((position + skip) as u64);
                Value::Null
            }
            8 => json!(self.text()?),
            9 => {
                let child = self.bytes::<1>()?[0];
                let count = self.count()?;
                let mut list = vec![];
                for n in 0..count {
                    let value = self.tag(child, depth + 1)?;
                    if n < 128 {
                        list.push(value);
                    }
                }
                json!(list)
            }
            10 => {
                let mut map = serde_json::Map::new();
                loop {
                    let child = self.bytes::<1>()?[0];
                    if child == 0 {
                        break;
                    }
                    let name = self.text()?;
                    let value = self.tag(child, depth + 1)?;
                    map.insert(name, value);
                }
                Value::Object(map)
            }
            _ => bail!("Неподдерживаемый тип NBT"),
        })
    }
}
fn metadata(path: &Path) -> Result<Value> {
    let mut bytes = vec![];
    flate2::read::GzDecoder::new(fs::File::open(path)?)
        .take(8 * 1024 * 1024 + 1)
        .read_to_end(&mut bytes)?;
    if bytes.len() > 8 * 1024 * 1024 {
        bail!("Файл мира слишком большой для предпросмотра");
    }
    let mut reader = Nbt {
        data: Cursor::new(bytes),
        nodes: 0,
    };
    let kind = reader.bytes::<1>()?[0];
    if kind != 10 {
        bail!("Нет корневого тега NBT");
    }
    reader.text()?;
    Ok(reader.tag(kind, 0)?["Data"].clone())
}
fn world(store: &Store, id: &str, name: &str) -> Result<PathBuf> {
    if s(&store.get(id)?, "loader") == "Rust" {
        bail!("Миры Horizon управляются отдельно");
    }
    let path = safe(&store.dir(id)?.join("saves"), leaf(name)?)?;
    if !path.is_dir() || path.join("level.dat").is_symlink() || !path.join("level.dat").is_file() {
        bail!("Мир не найден");
    }
    Ok(path)
}
pub fn list(store: &Store, id: &str) -> Result<Value> {
    let root = store.dir(id)?.join("saves");
    let mut worlds = vec![];
    if s(&store.get(id)?, "loader") == "Rust" {
        return Ok(json!([]));
    }
    if root.is_dir() {
        for entry in fs::read_dir(root)? {
            let entry = entry?;
            let path = entry.path();
            if !entry.file_type()?.is_dir() || !path.join("level.dat").is_file() {
                continue;
            }
            let name = entry.file_name().to_string_lossy().to_string();
            let data = metadata(&path.join("level.dat"));
            let valid = data.is_ok();
            let data = data.unwrap_or(Value::Null);
            let last = data["LastPlayed"]
                .as_i64()
                .and_then(chrono::DateTime::<chrono::Utc>::from_timestamp_millis)
                .or_else(|| {
                    fs::metadata(path.join("level.dat"))
                        .ok()?
                        .modified()
                        .ok()
                        .map(chrono::DateTime::<chrono::Utc>::from)
                })
                .map(|v| v.to_rfc3339());
            let icon = fs::metadata(path.join("icon.png"))
                .ok()
                .filter(|m| m.len() <= 2 * 1024 * 1024)
                .and_then(|_| fs::read(path.join("icon.png")).ok())
                .filter(|b| b.len() <= 2 * 1024 * 1024 && b.starts_with(b"\x89PNG\r\n\x1a\n"))
                .map(|b| format!("data:image/png;base64,{}", STANDARD.encode(b)));
            worlds.push(json!({"id":name,"name":if s(&data,"LevelName").is_empty(){name.as_str()}else{s(&data,"LevelName")},"version":s(&data["Version"],"Name"),"lastPlayed":last,"bytes":safety::tree_size(&path)?,"icon":icon,"metadataReadable":valid,"gameType":data["GameType"],"hardcore":data["hardcore"]}));
        }
    }
    worlds.sort_by(|a, b| s(b, "lastPlayed").cmp(s(a, "lastPlayed")));
    Ok(json!(worlds))
}
pub fn open(store: &Store, id: &str, name: &str) -> Result<Value> {
    open::that(world(store, id, name)?)?;
    Ok(Value::Null)
}
pub fn duplicate(store: &Store, id: &str, name: &str) -> Result<Value> {
    let source = world(store, id, name)?;
    let parent = source.parent().unwrap();
    let mut target = None;
    for n in 1..=1000 {
        let label = if n == 1 {
            format!("{name} — копия")
        } else {
            format!("{name} — копия {n}")
        };
        let p = safe(parent, &label)?;
        if !p.exists() {
            target = Some(p);
            break;
        }
    }
    let target = target.context("Не удалось подобрать имя копии")?;
    let stage = parent.join(format!(".nova-world-{}", uuid::Uuid::new_v4()));
    let result = (|| {
        copy_tree(&source, &stage, &["session.lock"])?;
        fs::rename(&stage, &target)?;
        Ok(json!({"id":target.file_name().unwrap().to_string_lossy()}))
    })();
    if result.is_err() {
        let _ = fs::remove_dir_all(stage);
    }
    result
}
pub fn backups(store: &Store, id: &str, name: &str) -> Result<Value> {
    world(store, id, name)?;
    let root = store.dir(id)?.join("backups/worlds-single");
    let mut items = vec![];
    if root.is_dir() {
        for entry in fs::read_dir(root)? {
            let entry = entry?;
            if entry.file_type()?.is_dir() {
                if let Ok(mut data) = read_json(&entry.path().join("manifest.json")) {
                    if s(&data, "world") == name {
                        data["id"] = json!(entry.file_name().to_string_lossy());
                        items.push(data);
                    }
                }
            }
        }
    }
    items.sort_by(|a, b| s(b, "at").cmp(s(a, "at")));
    Ok(json!(items))
}
pub fn backup(store: &Store, id: &str, name: &str) -> Result<Value> {
    let source = world(store, id, name)?;
    let bytes = safety::tree_size(&source)?;
    if safety::disk_free(&source).is_some_and(|free| free < bytes + 128 * 1024 * 1024) {
        bail!("Недостаточно места для копии мира");
    }
    let key = uuid::Uuid::new_v4().to_string();
    let target = store.dir(id)?.join("backups/worlds-single").join(&key);
    let data = json!({"id":key,"world":name,"at":now(),"bytes":bytes});
    let result = (|| {
        copy_tree(&source, &target.join("files"), &["session.lock"])?;
        write_json(&target.join("manifest.json"), &data)?;
        Ok(data)
    })();
    if result.is_err() {
        let _ = fs::remove_dir_all(target);
    }
    result
}
pub fn restore(store: &Store, id: &str, name: &str, key: &str) -> Result<Value> {
    let target = world(store, id, name)?;
    let root = store.dir(id)?;
    let copy = safe(&root.join("backups/worlds-single"), leaf(key)?)?;
    let manifest = read_json(&copy.join("manifest.json"))?;
    if s(&manifest, "world") != name || !copy.join("files/level.dat").is_file() {
        bail!("Копия не относится к этому миру или повреждена");
    }
    let stage = root.join(format!(".nova-world-stage-{}", uuid::Uuid::new_v4()));
    let old = root.join(format!(".nova-world-previous-{}", uuid::Uuid::new_v4()));
    let result = (|| {
        copy_tree(&copy.join("files"), &stage, &["session.lock"])?;
        let before = backup(store, id, name)?;
        fs::rename(&target, &old)?;
        if let Err(error) = fs::rename(&stage, &target) {
            fs::rename(&old, &target)?;
            return Err(error.into());
        }
        let _ = fs::remove_dir_all(&old);
        Ok(json!({"before":before,"restored":true}))
    })();
    if result.is_err() {
        let _ = fs::remove_dir_all(stage);
    }
    result
}
#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    #[test]
    fn reads_nbt_world_title_and_version() {
        let temp = tempfile::tempdir().unwrap();
        let p = temp.path().join("level.dat");
        let mut bytes = vec![10, 0, 0, 10, 0, 4];
        bytes.extend(b"Data");
        bytes.extend([8, 0, 9]);
        bytes.extend(b"LevelName");
        bytes.extend([0, 7]);
        bytes.extend(b"MyWorld");
        bytes.extend([10, 0, 7]);
        bytes.extend(b"Version");
        bytes.extend([8, 0, 4]);
        bytes.extend(b"Name");
        bytes.extend([0, 6]);
        bytes.extend(b"1.21.1");
        bytes.extend([0, 0, 0]);
        let mut encoder = flate2::write::GzEncoder::new(
            fs::File::create(&p).unwrap(),
            flate2::Compression::default(),
        );
        encoder.write_all(&bytes).unwrap();
        encoder.finish().unwrap();
        let data = metadata(&p).unwrap();
        assert_eq!(data["LevelName"], "MyWorld");
        assert_eq!(data["Version"]["Name"], "1.21.1");
    }
    #[test]
    fn restores_one_world_and_preserves_others() {
        let temp = tempfile::tempdir().unwrap();
        let store = Store::new(temp.path().into()).unwrap();
        store
            .add(json!({"id":"a","name":"A","version":"1.21.1","loader":"Vanilla"}))
            .unwrap();
        let root = store.dir("a").unwrap();
        for name in ["One", "Two"] {
            fs::create_dir_all(root.join("saves").join(name)).unwrap();
            fs::write(root.join("saves").join(name).join("level.dat"), name).unwrap();
        }
        let copy = backup(&store, "a", "One").unwrap();
        fs::write(root.join("saves/One/level.dat"), b"changed").unwrap();
        restore(&store, "a", "One", s(&copy, "id")).unwrap();
        assert_eq!(fs::read(root.join("saves/One/level.dat")).unwrap(), b"One");
        assert_eq!(fs::read(root.join("saves/Two/level.dat")).unwrap(), b"Two");
        let duplicate = duplicate(&store, "a", "One").unwrap();
        assert!(root
            .join("saves")
            .join(s(&duplicate, "id"))
            .join("level.dat")
            .is_file());
        assert!(backup(&store, "a", "../Two").is_err());
        assert!(restore(&store, "a", "Two", s(&copy, "id")).is_err());
    }
}
