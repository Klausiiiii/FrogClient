//! Microsoft account OAuth (authorization code + PKCE over a loopback redirect,
//! device code as fallback) for the FrogClient Azure app.

use super::{AuthEndpoints, AuthError};
use crate::http::Http;
use crate::launch::args::AZURE_CLIENT_ID;
use serde::Deserialize;
use std::collections::HashMap;
use std::time::Duration;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpListener;

pub const SCOPE: &str = "XboxLive.signin offline_access";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MsTokens {
    pub access_token: String,
    pub refresh_token: String,
}

/// `(verifier, S256 challenge)`, both base64url without padding.
pub fn pkce_pair() -> (String, String) {
    use base64::Engine;
    use rand::RngCore;
    use sha2::{Digest, Sha256};
    let mut bytes = [0u8; 32];
    rand::rng().fill_bytes(&mut bytes);
    let b64 = base64::engine::general_purpose::URL_SAFE_NO_PAD;
    let verifier = b64.encode(bytes);
    let challenge = b64.encode(Sha256::digest(verifier.as_bytes()));
    (verifier, challenge)
}

pub fn authorize_url(ep: &AuthEndpoints, client_id: &str, redirect: &str, challenge: &str, state: &str) -> String {
    let mut u = url::Url::parse(&ep.authorize).expect("authorize endpoint");
    u.query_pairs_mut()
        .append_pair("client_id", client_id)
        .append_pair("response_type", "code")
        .append_pair("redirect_uri", redirect)
        .append_pair("response_mode", "query")
        .append_pair("scope", SCOPE)
        .append_pair("code_challenge", challenge)
        .append_pair("code_challenge_method", "S256")
        .append_pair("state", state)
        .append_pair("prompt", "select_account");
    u.into()
}

/// Opens the system browser and waits up to five minutes for the redirect.
pub async fn login_interactive(http: &Http, ep: &AuthEndpoints) -> Result<MsTokens, AuthError> {
    login_interactive_with(http, ep, Duration::from_secs(300), |url| {
        let _ = open::that(url);
    })
    .await
}

pub(crate) async fn login_interactive_with(
    http: &Http,
    ep: &AuthEndpoints,
    timeout: Duration,
    open_browser: impl FnOnce(&str),
) -> Result<MsTokens, AuthError> {
    let v4 = TcpListener::bind("127.0.0.1:0").await.map_err(io_err)?;
    let port = v4.local_addr().map_err(io_err)?.port();
    // Browsers may resolve "localhost" to ::1 first; listen there too when possible.
    let v6 = TcpListener::bind(("::1", port)).await.ok();
    let redirect = format!("http://localhost:{port}");
    let (verifier, challenge) = pkce_pair();
    let (state, _) = pkce_pair();
    open_browser(&authorize_url(ep, AZURE_CLIENT_ID, &redirect, &challenge, &state));

    let code = tokio::time::timeout(timeout, wait_for_code(&v4, v6.as_ref(), &state))
        .await
        .map_err(|_| AuthError::Timeout)??;
    let form = [
        ("client_id", AZURE_CLIENT_ID),
        ("grant_type", "authorization_code"),
        ("code", code.as_str()),
        ("redirect_uri", redirect.as_str()),
        ("code_verifier", verifier.as_str()),
        ("scope", SCOPE),
    ];
    token_request(http, ep, &form).await
}

async fn wait_for_code(v4: &TcpListener, v6: Option<&TcpListener>, state: &str) -> Result<String, AuthError> {
    loop {
        let (mut stream, _) = match v6 {
            Some(v6) => tokio::select! { r = v4.accept() => r, r = v6.accept() => r },
            None => v4.accept().await,
        }
        .map_err(io_err)?;
        let mut buf = vec![0u8; 8192];
        let n = stream.read(&mut buf).await.map_err(io_err)?;
        let request = String::from_utf8_lossy(&buf[..n]);
        let target = request.split_whitespace().nth(1).unwrap_or("/");
        let Ok(url) = url::Url::parse(&format!("http://localhost{target}")) else { continue };
        let q: HashMap<String, String> = url.query_pairs().into_owned().collect();
        if !q.contains_key("code") && !q.contains_key("error") {
            // favicon and other noise
            let _ = stream.write_all(b"HTTP/1.1 404 Not Found\r\nContent-Length: 0\r\nConnection: close\r\n\r\n").await;
            continue;
        }
        let result = if q.get("state").map(String::as_str) != Some(state) {
            Err(AuthError::Other("state mismatch".into()))
        } else if let Some(code) = q.get("code") {
            Ok(code.clone())
        } else {
            Err(AuthError::Cancelled)
        };
        let body = if result.is_ok() {
            "<h2>Anmeldung abgeschlossen</h2><p>Du kannst dieses Fenster schlie&szlig;en und zu FrogClient zur&uuml;ckkehren.</p>"
        } else {
            "<h2>Anmeldung fehlgeschlagen</h2><p>Bitte in FrogClient erneut versuchen.</p>"
        };
        let page = format!(
            "<!doctype html><meta charset=utf-8><title>FrogClient</title><body style=\"font-family:sans-serif;background:#121714;color:#e8f5e9\">{body}</body>"
        );
        let resp = format!(
            "HTTP/1.1 200 OK\r\nContent-Type: text/html; charset=utf-8\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{page}",
            page.len()
        );
        let _ = stream.write_all(resp.as_bytes()).await;
        return result;
    }
}

pub async fn login_device_code(
    http: &Http,
    ep: &AuthEndpoints,
    on_code: impl Fn(String, String),
) -> Result<MsTokens, AuthError> {
    #[derive(Deserialize)]
    struct DeviceCode {
        device_code: String,
        user_code: String,
        verification_uri: String,
        interval: u64,
        expires_in: u64,
    }
    let resp = http
        .client()
        .post(&ep.devicecode)
        .form(&[("client_id", AZURE_CLIENT_ID), ("scope", SCOPE)])
        .send()
        .await?;
    if !resp.status().is_success() {
        return Err(AuthError::Other(format!("devicecode HTTP {}", resp.status())));
    }
    let dc: DeviceCode = resp.json().await?;
    on_code(dc.user_code.clone(), dc.verification_uri.clone());

    let deadline = tokio::time::Instant::now() + Duration::from_secs(dc.expires_in);
    let mut interval = dc.interval;
    loop {
        tokio::time::sleep(Duration::from_secs(interval)).await;
        if tokio::time::Instant::now() > deadline {
            return Err(AuthError::Timeout);
        }
        let form = [
            ("client_id", AZURE_CLIENT_ID),
            ("grant_type", "urn:ietf:params:oauth:grant-type:device_code"),
            ("device_code", dc.device_code.as_str()),
        ];
        match token_request(http, ep, &form).await {
            Err(AuthError::Other(e)) if e == "authorization_pending" => continue,
            Err(AuthError::Other(e)) if e == "slow_down" => interval += 5,
            Err(AuthError::Other(e)) if e == "authorization_declined" => return Err(AuthError::Cancelled),
            Err(AuthError::Other(e)) if e == "expired_token" => return Err(AuthError::Timeout),
            other => return other,
        }
    }
}

pub async fn refresh(http: &Http, ep: &AuthEndpoints, refresh_token: &str) -> Result<MsTokens, AuthError> {
    let form = [
        ("client_id", AZURE_CLIENT_ID),
        ("grant_type", "refresh_token"),
        ("refresh_token", refresh_token),
        ("scope", SCOPE),
    ];
    token_request(http, ep, &form).await
}

/// POST to the token endpoint. OAuth error codes come back as `AuthError::Other(code)`,
/// except `invalid_grant`, which means the user has to sign in again.
async fn token_request(http: &Http, ep: &AuthEndpoints, form: &[(&str, &str)]) -> Result<MsTokens, AuthError> {
    #[derive(Deserialize)]
    struct TokenResponse {
        access_token: String,
        refresh_token: String,
    }
    #[derive(Deserialize)]
    struct ErrorResponse {
        error: String,
    }
    let resp = http.client().post(&ep.token).form(form).send().await?;
    let status = resp.status();
    let text = resp.text().await?;
    if status.is_success() {
        let t: TokenResponse = serde_json::from_str(&text).map_err(|e| AuthError::Other(e.to_string()))?;
        return Ok(MsTokens { access_token: t.access_token, refresh_token: t.refresh_token });
    }
    match serde_json::from_str::<ErrorResponse>(&text) {
        Ok(e) if e.error == "invalid_grant" => Err(AuthError::ReloginRequired),
        Ok(e) => Err(AuthError::Other(e.error)),
        Err(_) => Err(AuthError::Other(format!("token HTTP {status}"))),
    }
}

fn io_err(e: std::io::Error) -> AuthError {
    AuthError::Other(e.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::auth::tests::endpoints;
    use base64::Engine;
    use sha2::{Digest, Sha256};
    use wiremock::matchers::{body_string_contains, method, path};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    #[test]
    fn pkce_challenge_is_s256_of_verifier() {
        let (verifier, challenge) = pkce_pair();
        assert!(verifier.len() >= 43);
        let expected = base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(Sha256::digest(verifier.as_bytes()));
        assert_eq!(challenge, expected);
    }

    #[test]
    fn authorize_url_contains_required_params() {
        let ep = AuthEndpoints::default();
        let u = url::Url::parse(&authorize_url(&ep, AZURE_CLIENT_ID, "http://localhost:1234", "CH", "ST")).unwrap();
        let q: std::collections::HashMap<_, _> = u.query_pairs().into_owned().collect();
        assert_eq!(q["client_id"], AZURE_CLIENT_ID);
        assert_eq!(q["response_type"], "code");
        assert!(q["scope"].contains("XboxLive.signin") && q["scope"].contains("offline_access"));
        assert_eq!(q["code_challenge"], "CH");
        assert_eq!(q["code_challenge_method"], "S256");
        assert_eq!(q["state"], "ST");
        assert_eq!(q["redirect_uri"], "http://localhost:1234");
        assert!(u.as_str().starts_with("https://login.microsoftonline.com/consumers/"));
    }

    #[tokio::test]
    async fn refresh_invalid_grant_requires_relogin() {
        let server = MockServer::start().await;
        Mock::given(method("POST")).and(path("/token")).and(body_string_contains("grant_type=refresh_token"))
            .respond_with(ResponseTemplate::new(400).set_body_string(r#"{"error":"invalid_grant"}"#))
            .mount(&server).await;
        let err = refresh(&Http::new(), &endpoints(&server), "old").await.unwrap_err();
        assert!(matches!(err, AuthError::ReloginRequired), "{err:?}");
    }

    #[tokio::test]
    async fn refresh_returns_new_tokens() {
        let server = MockServer::start().await;
        Mock::given(method("POST")).and(path("/token"))
            .respond_with(ResponseTemplate::new(200).set_body_string(r#"{"access_token":"A2","refresh_token":"R2"}"#))
            .mount(&server).await;
        let t = refresh(&Http::new(), &endpoints(&server), "old").await.unwrap();
        assert_eq!(t, MsTokens { access_token: "A2".into(), refresh_token: "R2".into() });
    }

    #[tokio::test]
    async fn device_code_polls_until_token() {
        let server = MockServer::start().await;
        Mock::given(method("POST")).and(path("/devicecode"))
            .respond_with(ResponseTemplate::new(200).set_body_string(
                r#"{"device_code":"DC","user_code":"ABCD-EFGH","verification_uri":"https://microsoft.com/link","interval":0,"expires_in":60}"#))
            .mount(&server).await;
        Mock::given(method("POST")).and(path("/token")).and(body_string_contains("device_code=DC"))
            .respond_with(ResponseTemplate::new(400).set_body_string(r#"{"error":"authorization_pending"}"#))
            .up_to_n_times(2).mount(&server).await;
        Mock::given(method("POST")).and(path("/token"))
            .respond_with(ResponseTemplate::new(200).set_body_string(r#"{"access_token":"A","refresh_token":"R"}"#))
            .mount(&server).await;
        let shown = std::sync::Mutex::new(None);
        let t = login_device_code(&Http::new(), &endpoints(&server), |code, uri| *shown.lock().unwrap() = Some((code, uri)))
            .await.unwrap();
        assert_eq!(t.access_token, "A");
        assert_eq!(shown.lock().unwrap().clone().unwrap().0, "ABCD-EFGH");
    }

    #[tokio::test]
    async fn loopback_exchanges_code() {
        let server = MockServer::start().await;
        Mock::given(method("POST")).and(path("/token")).and(body_string_contains("code=THE-CODE"))
            .and(body_string_contains("code_verifier="))
            .respond_with(ResponseTemplate::new(200).set_body_string(r#"{"access_token":"A","refresh_token":"R"}"#))
            .mount(&server).await;
        let ep = endpoints(&server);
        let t = login_interactive_with(&Http::new(), &ep, Duration::from_secs(10), |url| {
            let u = url::Url::parse(url).unwrap();
            let q: std::collections::HashMap<_, _> = u.query_pairs().into_owned().collect();
            let redirect = format!("{}/?code=THE-CODE&state={}", q["redirect_uri"], q["state"]);
            tokio::spawn(async move { let _ = reqwest::get(redirect).await; });
        }).await.unwrap();
        assert_eq!(t.refresh_token, "R");
    }

    #[tokio::test]
    async fn loopback_rejects_wrong_state() {
        let server = MockServer::start().await;
        let ep = endpoints(&server);
        let err = login_interactive_with(&Http::new(), &ep, Duration::from_secs(10), |url| {
            let u = url::Url::parse(url).unwrap();
            let q: std::collections::HashMap<_, _> = u.query_pairs().into_owned().collect();
            let redirect = format!("{}/?code=X&state=WRONG", q["redirect_uri"]);
            tokio::spawn(async move { let _ = reqwest::get(redirect).await; });
        }).await.unwrap_err();
        assert!(matches!(err, AuthError::Other(_)), "{err:?}");
    }

    #[tokio::test]
    async fn loopback_times_out() {
        let server = MockServer::start().await;
        let err = login_interactive_with(&Http::new(), &endpoints(&server), Duration::from_millis(200), |_| {})
            .await.unwrap_err();
        assert!(matches!(err, AuthError::Timeout));
    }
}
