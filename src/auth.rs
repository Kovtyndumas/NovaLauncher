use crate::{net::Net, store::Store, util::*};
use anyhow::{bail, Context, Result};
use md5::{Digest, Md5};
use serde_json::{json, Value};
pub const CLIENT: &str = "00000000402b5328";
pub const REDIRECT: &str = "https://login.live.com/oauth20_desktop.srf";
pub fn link(state: &str) -> String {
    format!("https://login.live.com/oauth20_authorize.srf?client_id={CLIENT}&response_type=code&redirect_uri={}&scope=XboxLive.signin%20offline_access&state={state}",crate::net::enc(REDIRECT))
}
/// Ignore unrelated navigation; validate the exact OAuth redirect and state.
pub fn redirect_code(url: &url::Url, expected: &str) -> Option<Result<String>> {
    let redirect = url::Url::parse(REDIRECT).ok()?;
    if url.scheme() != redirect.scheme()
        || url.host_str() != redirect.host_str()
        || url.path() != redirect.path()
        || url.port_or_known_default() != redirect.port_or_known_default()
    {
        return None;
    }
    let values = url
        .query_pairs()
        .collect::<std::collections::HashMap<_, _>>();
    Some((|| {
        if values.get("state").map(|v| v.as_ref()) != Some(expected) {
            bail!("Неверное состояние авторизации");
        }
        if let Some(error) = values.get("error") {
            if error == "access_denied" {
                bail!("Вход отменён");
            }
            bail!("Microsoft: {error}");
        }
        let code = values
            .get("code")
            .context("Microsoft не вернул код авторизации")?;
        if code.is_empty() {
            bail!("Пустой код авторизации");
        }
        Ok(code.to_string())
    })())
}
pub fn login(store: &Store, net: &Net, code: &str) -> Result<String> {
    let ms = net.form(
        "https://login.live.com/oauth20_token.srf",
        &[
            ("client_id", CLIENT),
            ("redirect_uri", REDIRECT),
            ("grant_type", "authorization_code"),
            ("code", code),
            ("scope", "XboxLive.signin offline_access"),
        ],
    )?;
    Ok(s(&exchange(store, net, &ms)?, "name").to_string())
}
fn exchange(store: &Store, net: &Net, ms: &Value) -> Result<Value> {
    if s(ms, "access_token").is_empty() {
        bail!("Microsoft не вернул токен входа");
    }
    let xbox=net.post("https://user.auth.xboxlive.com/user/authenticate",&json!({"Properties":{"AuthMethod":"RPS","SiteName":"user.auth.xboxlive.com","RpsTicket":format!("d={}",s(ms,"access_token"))},"RelyingParty":"http://auth.xboxlive.com","TokenType":"JWT"}))?;
    let xsts=net.post("https://xsts.auth.xboxlive.com/xsts/authorize",&json!({"Properties":{"SandboxId":"RETAIL","UserTokens":[s(&xbox,"Token")]},"RelyingParty":"rp://api.minecraftservices.com/","TokenType":"JWT"}))?;
    let uhs = xsts["DisplayClaims"]["xui"][0]["uhs"]
        .as_str()
        .unwrap_or("");
    let mc = net.post(
        "https://api.minecraftservices.com/authentication/login_with_xbox",
        &json!({"identityToken":format!("XBL3.0 x={uhs};{}",s(&xsts,"Token"))}),
    )?;
    let response = net
        .0
        .get("https://api.minecraftservices.com/minecraft/profile")
        .bearer_auth(s(&mc, "access_token"))
        .send()?;
    if !response.status().is_success() {
        bail!("У аккаунта нет Minecraft Java Edition или доступ не подтверждён");
    }
    let profile: Value = response.json()?;
    if s(&profile, "name").is_empty()
        || s(&profile, "id").len() != 32
        || s(&mc, "access_token").is_empty()
    {
        bail!("Microsoft вернул неполный профиль Minecraft");
    }
    let previous = read_json(&store.root.join("account.json")).unwrap_or(Value::Null);
    let refresh = if s(ms, "refresh_token").is_empty() {
        s(&previous, "refresh")
    } else {
        s(ms, "refresh_token")
    };
    let account = json!({"name":s(&profile,"name"),"uuid":s(&profile,"id"),"token":s(&mc,"access_token"),"refresh":refresh,"expires":chrono::Utc::now().timestamp()+number(&mc,"expires_in",86400)as i64-120,"online":true});
    write_json(&store.root.join("account.json"), &account)?;
    Ok(account)
}
pub fn session(store: &Store, net: &Net) -> Result<Value> {
    let file = store.root.join("account.json");
    if file.exists() {
        let a = read_json(&file)?;
        if a["expires"].as_i64().unwrap_or(0) > chrono::Utc::now().timestamp() {
            return Ok(a);
        }
        let ms = net.form(
            "https://login.live.com/oauth20_token.srf",
            &[
                ("client_id", CLIENT),
                ("grant_type", "refresh_token"),
                ("refresh_token", s(&a, "refresh")),
                ("scope", "XboxLive.signin offline_access"),
            ],
        )?;
        return exchange(store, net, &ms);
    }
    let settings = store.settings();
    let name = s(&settings, "name");
    if name.len() < 3
        || name.len() > 16
        || !name.chars().all(|c| c.is_ascii_alphanumeric() || c == '_')
    {
        bail!("Ник: 3–16 латинских букв, цифр или _");
    }
    let mut digest: [u8; 16] = Md5::digest(format!("OfflinePlayer:{name}").as_bytes()).into();
    digest[6] = (digest[6] & 0x0f) | 0x30;
    digest[8] = (digest[8] & 0x3f) | 0x80;
    Ok(
        json!({"name":name,"uuid":uuid::Uuid::from_bytes(digest).simple().to_string(),"token":"0","online":false}),
    )
}
