//! Microsoft token → Xbox Live → XSTS → Minecraft session, entitlements and profile.

use super::{AuthEndpoints, AuthError};
use crate::http::Http;
use serde::Deserialize;
use serde_json::json;

const XERR_NO_XBOX_ACCOUNT: u64 = 2148916233;
const XERR_CHILD: u64 = 2148916238;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct McSession {
    pub access_token: String,
    /// Without dashes.
    pub uuid: String,
    pub name: String,
    pub owns_game: bool,
    pub xuid: String,
}

#[derive(Deserialize)]
struct XboxToken {
    #[serde(rename = "Token")]
    token: String,
    #[serde(rename = "DisplayClaims")]
    claims: DisplayClaims,
}

#[derive(Deserialize)]
struct DisplayClaims {
    xui: Vec<Xui>,
}

#[derive(Deserialize)]
struct Xui {
    uhs: String,
    #[serde(default)]
    xid: Option<String>,
}

pub async fn minecraft_login(http: &Http, ep: &AuthEndpoints, ms_access_token: &str) -> Result<McSession, AuthError> {
    let xbl: XboxToken = post_json(http, &ep.xbl, json!({
        "Properties": {
            "AuthMethod": "RPS",
            "SiteName": "user.auth.xboxlive.com",
            "RpsTicket": format!("d={ms_access_token}")
        },
        "RelyingParty": "http://auth.xboxlive.com",
        "TokenType": "JWT"
    }))
    .await?;

    let xsts_resp = http.client().post(&ep.xsts).json(&json!({
        "Properties": { "SandboxId": "RETAIL", "UserTokens": [xbl.token] },
        "RelyingParty": "rp://api.minecraftservices.com/",
        "TokenType": "JWT"
    }))
    .send()
    .await?;
    if xsts_resp.status().as_u16() == 401 {
        #[derive(Deserialize)]
        struct XErr {
            #[serde(rename = "XErr")]
            xerr: u64,
        }
        let code = xsts_resp.json::<XErr>().await.map(|e| e.xerr).unwrap_or(0);
        return Err(match code {
            XERR_NO_XBOX_ACCOUNT => AuthError::NoXboxAccount,
            XERR_CHILD => AuthError::ChildAccount,
            other => AuthError::Other(format!("XSTS XErr {other}")),
        });
    }
    let xsts: XboxToken = parse(xsts_resp, "xsts").await?;
    let user = xsts.claims.xui.first().ok_or_else(|| AuthError::Other("XSTS ohne Benutzer".into()))?;
    let xuid = user.xid.clone().unwrap_or_else(|| "0".into());

    let mc_resp = http.client().post(&ep.mc_login)
        .json(&json!({ "identityToken": format!("XBL3.0 x={};{}", user.uhs, xsts.token) }))
        .send()
        .await?;
    // Mojang answers 403 for Azure apps that are not (yet) approved for the Minecraft API.
    if mc_resp.status().as_u16() == 403 {
        return Err(AuthError::AppNotApproved);
    }
    #[derive(Deserialize)]
    struct McLogin {
        access_token: String,
    }
    let mc: McLogin = parse(mc_resp, "login_with_xbox").await?;

    #[derive(Deserialize)]
    struct Entitlements {
        #[serde(default)]
        items: Vec<Item>,
    }
    #[derive(Deserialize)]
    struct Item {
        name: String,
    }
    let ent: Entitlements = parse(http.client().get(&ep.entitlements).bearer_auth(&mc.access_token).send().await?, "entitlements").await?;
    let owns_game = ent.items.iter().any(|i| i.name == "product_minecraft" || i.name == "game_minecraft");

    let profile_resp = http.client().get(&ep.profile).bearer_auth(&mc.access_token).send().await?;
    if profile_resp.status().as_u16() == 404 {
        return Err(AuthError::NoProfile);
    }
    #[derive(Deserialize)]
    struct Profile {
        id: String,
        name: String,
    }
    let profile: Profile = parse(profile_resp, "profile").await?;

    Ok(McSession { access_token: mc.access_token, uuid: profile.id, name: profile.name, owns_game, xuid })
}

async fn post_json<T: serde::de::DeserializeOwned>(http: &Http, url: &str, body: serde_json::Value) -> Result<T, AuthError> {
    let resp = http.client().post(url).header("Accept", "application/json").json(&body).send().await?;
    parse(resp, url).await
}

async fn parse<T: serde::de::DeserializeOwned>(resp: reqwest::Response, what: &str) -> Result<T, AuthError> {
    let status = resp.status();
    if !status.is_success() {
        return Err(AuthError::Other(format!("{what}: HTTP {status}")));
    }
    resp.json::<T>().await.map_err(|e| AuthError::Other(format!("{what}: {e}")))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::auth::tests::endpoints;
    use wiremock::matchers::{body_string_contains, header, method, path};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    async fn mount_chain(server: &MockServer, mc_status: u16, entitlements: &str) {
        Mock::given(method("POST")).and(path("/xbl")).and(body_string_contains("d=MS-ACCESS"))
            .respond_with(ResponseTemplate::new(200).set_body_string(
                r#"{"Token":"XBL","DisplayClaims":{"xui":[{"uhs":"UHS"}]}}"#))
            .mount(server).await;
        Mock::given(method("POST")).and(path("/xsts")).and(body_string_contains("rp://api.minecraftservices.com/"))
            .respond_with(ResponseTemplate::new(200).set_body_string(
                r#"{"Token":"XSTS","DisplayClaims":{"xui":[{"uhs":"UHS"}]}}"#))
            .mount(server).await;
        Mock::given(method("POST")).and(path("/mc_login")).and(body_string_contains("XBL3.0 x=UHS;XSTS"))
            .respond_with(ResponseTemplate::new(mc_status).set_body_string(r#"{"access_token":"MC","expires_in":86400}"#))
            .mount(server).await;
        Mock::given(method("GET")).and(path("/entitlements")).and(header("authorization", "Bearer MC"))
            .respond_with(ResponseTemplate::new(200).set_body_string(entitlements.to_string()))
            .mount(server).await;
        Mock::given(method("GET")).and(path("/profile")).and(header("authorization", "Bearer MC"))
            .respond_with(ResponseTemplate::new(200).set_body_string(r#"{"id":"b50ad385829d3141a2167e7d7539ba7f","name":"FrogMain"}"#))
            .mount(server).await;
    }

    const OWNS: &str = r#"{"items":[{"name":"product_minecraft"},{"name":"game_minecraft"}]}"#;

    #[tokio::test]
    async fn full_chain_success() {
        let server = MockServer::start().await;
        mount_chain(&server, 200, OWNS).await;
        let s = minecraft_login(&Http::new(), &endpoints(&server), "MS-ACCESS").await.unwrap();
        assert_eq!(s, McSession {
            access_token: "MC".into(),
            uuid: "b50ad385829d3141a2167e7d7539ba7f".into(),
            name: "FrogMain".into(),
            owns_game: true,
            xuid: "0".into(),
        });
    }

    #[tokio::test]
    async fn mc_login_403_is_app_not_approved() {
        let server = MockServer::start().await;
        mount_chain(&server, 403, OWNS).await;
        let err = minecraft_login(&Http::new(), &endpoints(&server), "MS-ACCESS").await.unwrap_err();
        assert!(matches!(err, AuthError::AppNotApproved), "{err:?}");
        assert!(err.user_message().contains("Mojang"));
    }

    #[tokio::test]
    async fn xsts_no_xbox_account() {
        let server = MockServer::start().await;
        Mock::given(method("POST")).and(path("/xbl"))
            .respond_with(ResponseTemplate::new(200).set_body_string(r#"{"Token":"XBL","DisplayClaims":{"xui":[{"uhs":"UHS"}]}}"#))
            .mount(&server).await;
        Mock::given(method("POST")).and(path("/xsts"))
            .respond_with(ResponseTemplate::new(401).set_body_string(r#"{"XErr":2148916233}"#))
            .mount(&server).await;
        let err = minecraft_login(&Http::new(), &endpoints(&server), "MS-ACCESS").await.unwrap_err();
        assert!(matches!(err, AuthError::NoXboxAccount), "{err:?}");
    }

    #[tokio::test]
    async fn empty_entitlements_owns_game_false() {
        let server = MockServer::start().await;
        mount_chain(&server, 200, r#"{"items":[]}"#).await;
        let s = minecraft_login(&Http::new(), &endpoints(&server), "MS-ACCESS").await.unwrap();
        assert!(!s.owns_game);
    }

    #[tokio::test]
    async fn missing_profile_is_no_profile() {
        let server = MockServer::start().await;
        Mock::given(method("GET")).and(path("/profile"))
            .respond_with(ResponseTemplate::new(404)).with_priority(1).mount(&server).await;
        mount_chain(&server, 200, OWNS).await;
        let err = minecraft_login(&Http::new(), &endpoints(&server), "MS-ACCESS").await.unwrap_err();
        assert!(matches!(err, AuthError::NoProfile), "{err:?}");
    }
}
