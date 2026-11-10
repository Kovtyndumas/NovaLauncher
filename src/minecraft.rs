use crate::{auth, net::Net, runtime, store::Store, util::*};
use anyhow::{bail, Context, Result};
use serde_json::{json, Value};
use std::{
    collections::{HashMap, HashSet},
    fs,
    io::{BufRead, BufReader},
    path::{Path, PathBuf},
    process::{Command, Stdio},
    sync::{
        atomic::{AtomicUsize, Ordering},
        Arc, Mutex,
    },
};
pub const MANIFEST: &str = "https://piston-meta.mojang.com/mc/game/version_manifest_v2.json";
pub fn manifest(store: &Store, net: &Net) -> Result<Value> {
    let cache = store.root.join("versions-cache.json");
    match net.json(MANIFEST) {
        Ok(m) => {
            write_json(&cache, &m)?;
            Ok(m)
        }
        Err(e) => {
            if cache.exists() {
                read_json(&cache)
            } else {
                Err(e)
            }
        }
    }
}
pub fn versions(store: &Store, net: &Net) -> Result<Value> {
    let m = manifest(store, net)?;
    Ok(
        json!({"versions":list(&m,"versions").iter().filter(|v|s(v,"type")=="release").map(|v|s(v,"id")).collect::<Vec<_>>(),"cached":false}),
    )
}
pub fn version(store: &Store, net: &Net, id: &str) -> Result<Value> {
    if !id
        .chars()
        .all(|c| c.is_ascii_alphanumeric() || "._+-".contains(c))
    {
        bail!("Неправильная версия");
    }
    let path = store
        .root
        .join("minecraft/versions")
        .join(id)
        .join(format!("{id}.json"));
    if path.exists() {
        return read_json(&path);
    }
    let m = manifest(store, net)?;
    let entry = list(&m, "versions")
        .iter()
        .find(|v| s(v, "id") == id)
        .context("Версия не найдена")?;
    net.download(s(entry, "url"), &path, "sha1", s(entry, "sha1"), 0)?;
    read_json(&path)
}
pub fn allowed(item: &Value, os: &str, arch: &str) -> bool {
    let rules = list(item, "rules");
    if rules.is_empty() {
        return true;
    }
    let mut allow = false;
    for rule in rules {
        let p = &rule["os"];
        let mut matches = (s(p, "name").is_empty() || s(p, "name") == os)
            && (s(p, "arch").is_empty()
                || regex::Regex::new(s(p, "arch"))
                    .map(|r| r.is_match(arch))
                    .unwrap_or(false));
        if !s(p, "version").is_empty() {
            let os_version = std::env::var("OS_VERSION").unwrap_or_else(|_| {
                if cfg!(target_os = "macos") {
                    String::from_utf8_lossy(
                        &Command::new("sw_vers")
                            .arg("-productVersion")
                            .output()
                            .map(|o| o.stdout)
                            .unwrap_or_default(),
                    )
                    .trim()
                    .to_string()
                } else {
                    "".into()
                }
            });
            matches &= regex::Regex::new(s(p, "version"))
                .map(|r| r.is_match(&os_version))
                .unwrap_or(false);
        }
        if rule["features"]
            .as_object()
            .is_some_and(|features| features.values().any(|v| v.as_bool() == Some(true)))
        {
            matches = false;
        }
        if matches {
            allow = s(rule, "action") == "allow";
        }
    }
    allow
}
pub fn maven(name: &str) -> Result<PathBuf> {
    let (name, ext) = name.split_once('@').unwrap_or((name, "jar"));
    let parts = name.split(':').collect::<Vec<_>>();
    if !(3..=4).contains(&parts.len())
        || !parts.iter().all(|v| {
            v.chars()
                .all(|c| c.is_ascii_alphanumeric() || "._+-".contains(c))
        })
    {
        bail!("Некорректная библиотека");
    }
    Ok(PathBuf::from(parts[0].replace('.', "/"))
        .join(parts[1])
        .join(parts[2])
        .join(format!(
            "{}-{}{}.{ext}",
            parts[1],
            parts[2],
            parts.get(3).map(|v| format!("-{v}")).unwrap_or_default()
        )))
}
pub fn split_args(value: &str) -> Result<Vec<String>> {
    let mut args = vec![];
    let mut token = String::new();
    let mut quote = None;
    for ch in value.chars() {
        if ch == '\'' || ch == '"' {
            if quote == Some(ch) {
                quote = None;
            } else if quote.is_none() {
                quote = Some(ch);
            } else {
                token.push(ch);
            }
        } else if ch.is_whitespace() && quote.is_none() {
            if !token.is_empty() {
                args.push(std::mem::take(&mut token));
            }
        } else {
            token.push(ch);
        }
    }
    if quote.is_some() {
        bail!("Незакрытая кавычка в JVM-аргументах");
    }
    if !token.is_empty() {
        args.push(token);
    }
    Ok(args)
}
fn artifact(net: &Net, a: &Value, p: &Path) -> Result<()> {
    if s(a, "url").is_empty() {
        bail!("Нет загрузки {}", p.display());
    }
    net.download(s(a, "url"), p, "sha1", s(a, "sha1"), number(a, "size", 0))
}
pub fn merge(parent: &Value, child: &Value) -> Value {
    let mut result = parent.clone();
    for (k, v) in child.as_object().unwrap() {
        if k != "libraries" && k != "arguments" {
            result[k] = v.clone();
        }
    }
    let child_lib = list(child, "libraries");
    let names = child_lib
        .iter()
        .map(|v| {
            s(v, "name")
                .split(':')
                .take(2)
                .collect::<Vec<_>>()
                .join(":")
        })
        .collect::<HashSet<_>>();
    let mut libs = child_lib.to_vec();
    for v in list(parent, "libraries") {
        if !names.contains(
            &s(v, "name")
                .split(':')
                .take(2)
                .collect::<Vec<_>>()
                .join(":"),
        ) {
            libs.push(v.clone());
        }
    }
    result["libraries"] = json!(libs);
    let mut args = parent["arguments"].as_object().cloned().unwrap_or_default();
    if let Some(child_args) = child["arguments"].as_object() {
        for (k, v) in child_args {
            let mut existing = args
                .get(k)
                .and_then(Value::as_array)
                .cloned()
                .unwrap_or_default();
            existing.extend(v.as_array().cloned().unwrap_or_default());
            args.insert(k.clone(), json!(existing));
        }
    }
    result["arguments"] = json!(args);
    result
}
fn loader(
    store: &Store,
    net: &Net,
    inst: &Value,
    base: &Value,
    java: &Path,
    log: &dyn Fn(String),
) -> Result<Value> {
    let kind = s(inst, "loader");
    let mc = s(inst, "version");
    if kind == "Vanilla" {
        return Ok(base.clone());
    }
    let mut lv = s(inst, "loaderVersion").to_owned();
    let root = store.root.join("minecraft");
    if kind == "Fabric" || kind == "Quilt" {
        let api = if kind == "Fabric" {
            "https://meta.fabricmc.net/v2/versions/loader"
        } else {
            "https://meta.quiltmc.org/v3/versions/loader"
        };
        if lv.is_empty() {
            let list = net.json(&format!("{api}/{mc}"))?;
            lv = s(&list[0]["loader"], "version").into();
            if lv.is_empty() {
                bail!("Загрузчик недоступен для {mc}");
            }
            store.loader_version(s(inst, "id"), &lv)?;
        }
        let cache = root
            .join("loader-profiles")
            .join(format!("{kind}-{mc}-{lv}.json"));
        let profile = if cache.exists() {
            read_json(&cache)?
        } else {
            log(format!("Установка {kind} {lv}"));
            let p = net.json(&format!("{api}/{mc}/{lv}/profile/json"))?;
            write_json(&cache, &p)?;
            p
        };
        return Ok(merge(base, &profile));
    }
    if kind == "Forge" || kind == "NeoForge" {
        let neo = kind == "NeoForge";
        if lv.is_empty() {
            if neo {
                let mut prefix = mc.strip_prefix("1.").unwrap_or(mc).to_owned();
                if !prefix.contains('.') {
                    prefix.push_str(".0");
                }
                let versions=net.json("https://maven.neoforged.net/api/maven/versions/releases/net/neoforged/neoforge")?;
                lv = list(&versions, "versions")
                    .iter()
                    .filter_map(Value::as_str)
                    .filter(|v| v.starts_with(&format!("{prefix}.")))
                    .last()
                    .unwrap_or("")
                    .into();
            } else {
                let p=net.json("https://files.minecraftforge.net/net/minecraftforge/forge/promotions_slim.json")?;
                lv = p["promos"][format!("{mc}-recommended")]
                    .as_str()
                    .or_else(|| p["promos"][format!("{mc}-latest")].as_str())
                    .unwrap_or("")
                    .into();
            }
            if lv.is_empty() {
                bail!("{kind} недоступен для {mc}");
            }
            store.loader_version(s(inst, "id"), &lv)?;
        }
        let id = if neo {
            format!("neoforge-{lv}")
        } else {
            format!("{mc}-forge-{lv}")
        };
        let mut profile = root.join("versions").join(&id).join(format!("{id}.json"));
        if !profile.exists() {
            log(format!("Установка {kind} {lv}"));
            artifact(
                net,
                &base["downloads"]["client"],
                &root.join("versions").join(mc).join(format!("{mc}.jar")),
            )?;
            if !root.join("launcher_profiles.json").exists() {
                write_json(
                    &root.join("launcher_profiles.json"),
                    &json!({"profiles":{}}),
                )?;
            }
            let url = if neo {
                format!("https://maven.neoforged.net/releases/net/neoforged/neoforge/{lv}/neoforge-{lv}-installer.jar")
            } else {
                format!("https://maven.minecraftforge.net/net/minecraftforge/forge/{mc}-{lv}/forge-{mc}-{lv}-installer.jar")
            };
            let file = store
                .root
                .join("downloads")
                .join(format!("{kind}-{mc}-{lv}.jar"));
            net.download(&url, &file, "sha1", "", 0)?;
            let mut child = Command::new(java)
                .arg("-jar")
                .arg(file)
                .arg("--installClient")
                .arg(&root)
                .stdout(Stdio::piped())
                .stderr(Stdio::piped())
                .spawn()?;
            for line in BufReader::new(child.stdout.take().unwrap())
                .lines()
                .map_while(|r| r.ok())
            {
                log(line);
            }
            let output = child.wait_with_output()?;
            if !output.status.success() {
                bail!(
                    "Установщик {kind}: {}",
                    String::from_utf8_lossy(&output.stderr)
                );
            }
        }
        if !profile.exists() {
            for e in fs::read_dir(root.join("versions"))? {
                let p = e?.path();
                let name = p.file_name().unwrap().to_string_lossy();
                if name.contains(&lv) && p.join(format!("{name}.json")).exists() {
                    profile = p.join(format!("{name}.json"));
                    break;
                }
            }
        }
        return Ok(merge(base, &read_json(&profile)?));
    }
    bail!("Неизвестный загрузчик")
}
fn libraries(
    store: &Store,
    net: &Net,
    profile: &Value,
    natives: &Path,
    game_arch: &str,
) -> Result<Vec<PathBuf>> {
    let root = store.root.join("minecraft/libraries");
    fs::create_dir_all(natives)?;
    let mut cp = vec![];
    for lib in list(profile, "libraries") {
        if !allowed(lib, runtime::os(), game_arch) {
            continue;
        }
        let name = s(lib, "name");
        let a = &lib["downloads"]["artifact"];
        if a.is_object() {
            let path = if s(a, "path").is_empty() {
                maven(name)?
            } else {
                PathBuf::from(s(a, "path"))
            };
            let p = safe(&root, &path.to_string_lossy())?;
            artifact(net, a, &p)?;
            cp.push(p);
        } else if !name.is_empty() {
            let rel = maven(name)?;
            let p = root.join(&rel);
            if !p.exists() {
                let base = if s(lib, "url").is_empty() {
                    "https://libraries.minecraft.net/"
                } else {
                    s(lib, "url")
                };
                net.download(
                    &format!(
                        "{}/{}",
                        base.trim_end_matches('/'),
                        rel.to_string_lossy().replace('\\', "/")
                    ),
                    &p,
                    "sha1",
                    "",
                    0,
                )?;
            }
            cp.push(p);
        }
        let classifier = s(&lib["natives"], runtime::os()).replace(
            "${arch}",
            if game_arch.contains("64") { "64" } else { "32" },
        );
        if !classifier.is_empty() {
            let a = &lib["downloads"]["classifiers"][&classifier];
            if a.is_object() {
                let p = safe(&root, s(a, "path"))?;
                artifact(net, a, &p)?;
                unzip(&p, natives, "")?;
            }
        }
    }
    Ok(cp)
}
fn assets(
    store: &Store,
    net: &Net,
    profile: &Value,
    log: &(dyn Fn(String) + Sync),
) -> Result<PathBuf> {
    let a = &profile["assetIndex"];
    let id = s(a, "id");
    let root = store.root.join("minecraft/assets");
    let index = root.join("indexes").join(format!("{id}.json"));
    artifact(net, a, &index)?;
    let data = read_json(&index)?;
    let objects = data["objects"]
        .as_object()
        .context("Нет списка ресурсов")?
        .iter()
        .map(|(k, v)| (k.clone(), v.clone()))
        .collect::<Vec<_>>();
    log(format!("Проверка ресурсов: {}", objects.len()));
    let next = AtomicUsize::new(0);
    let done = AtomicUsize::new(0);
    let errors = Mutex::new(Vec::<String>::new());
    let virtual_assets = flag(&data, "virtual", false) || flag(&data, "map_to_resources", false);
    std::thread::scope(|scope| {
        for _ in 0..8 {
            let objects = &objects;
            let next = &next;
            let done = &done;
            let errors = &errors;
            let root = &root;
            scope.spawn(move || loop {
                let n = next.fetch_add(1, Ordering::Relaxed);
                if n >= objects.len() {
                    break;
                }
                let (key, a) = &objects[n];
                let result = (|| {
                    let hash = s(a, "hash");
                    if hash.len() != 40 || !hash.chars().all(|c| c.is_ascii_hexdigit()) {
                        bail!("Неправильный хеш ресурса");
                    }
                    let p = root.join("objects").join(&hash[..2]).join(hash);
                    net.download(
                        &format!(
                            "https://resources.download.minecraft.net/{}/{}",
                            &hash[..2],
                            hash
                        ),
                        &p,
                        "sha1",
                        hash,
                        number(a, "size", 0),
                    )?;
                    if virtual_assets {
                        let dest = safe(&root.join("virtual").join(id), key)?;
                        if !dest.exists() {
                            fs::create_dir_all(dest.parent().unwrap())?;
                            fs::copy(&p, dest)?;
                        }
                    }
                    Ok::<_, anyhow::Error>(())
                })();
                if let Err(e) = result {
                    errors.lock().unwrap().push(e.to_string());
                    break;
                }
                let n = done.fetch_add(1, Ordering::Relaxed) + 1;
                if n % 200 == 0 {
                    log(format!("Ресурсы: {n} / {}", objects.len()));
                }
            });
        }
    });
    let errors = errors.into_inner().unwrap();
    if !errors.is_empty() {
        bail!("{}", errors.join("\n"));
    }
    Ok(if virtual_assets {
        root.join("virtual").join(id)
    } else {
        root
    })
}
fn expand(value: &str, vars: &HashMap<String, String>) -> String {
    let mut result = value.to_string();
    for (k, v) in vars {
        result = result.replace(&format!("${{{k}}}"), v);
    }
    result
}
pub fn arguments(
    args: &[Value],
    vars: &HashMap<String, String>,
    os: &str,
    arch: &str,
) -> Vec<String> {
    let mut result = vec![];
    for arg in args {
        if let Some(arg) = arg.as_str() {
            result.push(expand(arg, vars));
        } else if allowed(arg, os, arch) {
            if let Some(a) = arg["value"].as_array() {
                for v in a {
                    result.push(expand(v.as_str().unwrap_or(""), vars));
                }
            } else if let Some(a) = arg["value"].as_str() {
                result.push(expand(a, vars));
            }
        }
    }
    result
}
pub fn launch(
    store: &Store,
    net: &Net,
    inst: &Value,
    server: &str,
    log: Arc<dyn Fn(String) + Send + Sync>,
    started: &dyn Fn(),
    child_slot: &Mutex<Option<std::process::Child>>,
) -> Result<i32> {
    let mc = s(inst, "version");
    let game = store.dir(s(inst, "id"))?;
    let account = auth::session(store, net)?;
    let vanilla = version(store, net, mc)?;
    let major = number(&vanilla["javaVersion"], "majorVersion", 8) as u32;
    let target_arch = runtime::game_arch(&vanilla);
    let java = runtime::ensure_for(store, net, inst, major, target_arch, &*log)?;
    log(format!("Java {major} · {}", java.display()));
    let profile = loader(store, net, inst, &vanilla, &java, &*log)?;
    let game_arch = if target_arch == "x64" {
        "x86_64"
    } else {
        "aarch64"
    };
    let natives = game.join("natives").join(mc);
    let mut cp = libraries(store, net, &profile, &natives, game_arch)?;
    let jar = store
        .root
        .join("minecraft/versions")
        .join(mc)
        .join(format!("{mc}.jar"));
    artifact(net, &vanilla["downloads"]["client"], &jar)?;
    cp.push(jar);
    let asset_root = assets(store, net, &vanilla, &*log)?;
    if s(&vanilla, "assets") == "legacy" {
        copy_tree(&asset_root, &game.join("resources"), &[])?;
    }
    let sep = if cfg!(windows) { ";" } else { ":" };
    let classpath = cp
        .iter()
        .map(|p| p.to_string_lossy().to_string())
        .collect::<Vec<_>>()
        .join(sep);
    let mut vars = HashMap::<String, String>::new();
    for (k, v) in [
        ("auth_player_name", s(&account, "name").to_owned()),
        ("auth_uuid", s(&account, "uuid").into()),
        ("auth_access_token", s(&account, "token").into()),
        (
            "auth_session",
            if flag(&account, "online", false) {
                format!("token:{}:{}", s(&account, "token"), s(&account, "uuid"))
            } else {
                "0".into()
            },
        ),
        (
            "user_type",
            if flag(&account, "online", false) {
                "msa".into()
            } else {
                "legacy".into()
            },
        ),
        ("version_name", s(&profile, "id").into()),
        ("game_directory", game.display().to_string()),
        (
            "assets_root",
            store.root.join("minecraft/assets").display().to_string(),
        ),
        ("assets_index_name", s(&vanilla["assetIndex"], "id").into()),
        ("game_assets", asset_root.display().to_string()),
        ("natives_directory", natives.display().to_string()),
        ("launcher_name", "Nova".into()),
        ("launcher_version", "3.0.0-rust".into()),
        ("classpath", classpath.clone()),
        ("classpath_separator", sep.into()),
        (
            "library_directory",
            store.root.join("minecraft/libraries").display().to_string(),
        ),
        ("version_type", "release".into()),
        ("user_properties", "{}".into()),
        ("clientid", "".into()),
        ("auth_xuid", "".into()),
    ] {
        vars.insert(k.into(), v);
    }
    let settings = store.game_settings(inst);
    let mut args = vec![
        "-Xms512M".to_string(),
        format!("-Xmx{}G", number(&settings, "ram", 4).clamp(1, 64)),
    ];
    args.extend(split_args(s(&settings, "jvm"))?);
    let jvm = list(&profile["arguments"], "jvm");
    if jvm.is_empty() {
        if runtime::os() == "osx" {
            args.push("-XstartOnFirstThread".into());
        }
        args.extend([
            format!("-Djava.library.path={}", natives.display()),
            "-cp".into(),
            classpath,
        ]);
    } else {
        args.extend(arguments(jvm, &vars, runtime::os(), game_arch));
    }
    let logging = &vanilla["logging"]["client"];
    if logging["file"].is_object() {
        let file = &logging["file"];
        let p = store
            .root
            .join("minecraft/assets/log_configs")
            .join(leaf(s(file, "id"))?);
        artifact(net, file, &p)?;
        args.push(s(logging, "argument").replace("${path}", &p.display().to_string()));
    }
    args.push(s(&profile, "mainClass").into());
    if !s(&profile, "minecraftArguments").is_empty() {
        args.extend(
            split_args(s(&profile, "minecraftArguments"))?
                .iter()
                .map(|v| expand(v, &vars)),
        );
    } else {
        args.extend(arguments(
            list(&profile["arguments"], "game"),
            &vars,
            runtime::os(),
            game_arch,
        ));
    }
    if !server.is_empty() {
        if profile.to_string().contains("quickPlayMultiplayer") {
            args.extend(["--quickPlayMultiplayer".into(), server.into()]);
        } else {
            let addr = url::Url::parse(&format!("minecraft://{server}"))?;
            args.extend(["--server".into(), addr.host_str().unwrap_or(server).into()]);
            if let Some(port) = addr.port() {
                args.extend(["--port".into(), port.to_string()]);
            }
        }
    }
    if number(&settings, "ww", 0) > 0 && number(&settings, "wh", 0) > 0 {
        args.extend([
            "--width".into(),
            number(&settings, "ww", 0).to_string(),
            "--height".into(),
            number(&settings, "wh", 0).to_string(),
        ]);
    }
    if flag(&settings, "full", false) {
        args.push("--fullscreen".into());
    }
    if args.iter().any(|a| a.contains("${")) {
        bail!("Неподдерживаемые аргументы профиля");
    }
    log(format!(
        "Запуск {} · {} {mc}",
        s(inst, "name"),
        s(inst, "loader")
    ));
    let mut child = Command::new(java)
        .args(args)
        .current_dir(&game)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()?;
    let out = child.stdout.take().unwrap();
    let err = child.stderr.take().unwrap();
    *child_slot.lock().unwrap() = Some(child);
    started();
    let file = Arc::new(Mutex::new(fs::File::create(
        game.join("nova-last-launch.log"),
    )?));
    let token = s(&account, "token").to_string();
    let mut threads = vec![];
    for stream in [
        Box::new(out) as Box<dyn std::io::Read + Send>,
        Box::new(err),
    ] {
        let log = log.clone();
        let file = file.clone();
        let token = token.clone();
        threads.push(std::thread::spawn(move || {
            use std::io::Write;
            let mut level = String::new();
            let mut pending = String::new();
            let mut cdata = false;
            for line in BufReader::new(stream).lines().map_while(|r| r.ok()) {
                let line = if token != "0" && !token.is_empty() {
                    line.replace(&token, "[hidden]")
                } else {
                    line
                };
                let _ = writeln!(file.lock().unwrap(), "{line}");
                if line.contains("<log4j:Event") {
                    level = regex::Regex::new("level=\"([^\"]+)\"")
                        .unwrap()
                        .captures(&line)
                        .map(|c| format!("[{}] ", &c[1]))
                        .unwrap_or_default();
                    continue;
                }
                let mut text = line.as_str();
                if let Some(at) = text.find("<![CDATA[") {
                    cdata = true;
                    text = &text[at + 9..];
                }
                if cdata {
                    if let Some(at) = text.find("]]>") {
                        pending.push_str(&text[..at]);
                        log(format!("{level}{pending}"));
                        pending.clear();
                        cdata = false;
                    } else {
                        pending.push_str(text);
                        pending.push('\n');
                    }
                } else if !text.trim_start().starts_with("<log4j:")
                    && !text.trim_start().starts_with("</log4j:")
                {
                    log(line);
                }
            }
        }));
    }
    let code = loop {
        let status = child_slot
            .lock()
            .unwrap()
            .as_mut()
            .context("Нет процесса")?
            .try_wait()?;
        if let Some(status) = status {
            break status.code().unwrap_or(-1);
        }
        std::thread::sleep(std::time::Duration::from_millis(100));
    };
    child_slot.lock().unwrap().take();
    for thread in threads {
        let _ = thread.join();
    }
    Ok(code)
}
