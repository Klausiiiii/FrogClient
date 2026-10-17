pub mod msa;
pub mod offline;
pub mod xbox;

#[derive(Debug, Clone)]
pub struct AuthEndpoints {
    pub authorize: String,
    pub token: String,
    pub devicecode: String,
    pub xbl: String,
    pub xsts: String,
    pub mc_login: String,
    pub entitlements: String,
    pub profile: String,
}

impl Default for AuthEndpoints {
    fn default() -> Self {
        let ms = "https://login.microsoftonline.com/consumers/oauth2/v2.0";
        let mc = "https://api.minecraftservices.com";
        AuthEndpoints {
            authorize: format!("{ms}/authorize"),
            token: format!("{ms}/token"),
            devicecode: format!("{ms}/devicecode"),
            xbl: "https://user.auth.xboxlive.com/user/authenticate".into(),
            xsts: "https://xsts.auth.xboxlive.com/xsts/authorize".into(),
            mc_login: format!("{mc}/authentication/login_with_xbox"),
            entitlements: format!("{mc}/entitlements/mcstore"),
            profile: format!("{mc}/minecraft/profile"),
        }
    }
}

#[derive(Debug, Clone, thiserror::Error)]
pub enum AuthError {
    #[error("Anmeldung abgebrochen")]
    Cancelled,
    #[error("Zeitüberschreitung bei der Anmeldung")]
    Timeout,
    #[error("Kein Xbox-Profil")]
    NoXboxAccount,
    #[error("Kinderkonto")]
    ChildAccount,
    #[error("App nicht freigegeben")]
    AppNotApproved,
    #[error("Kein Minecraft-Profil")]
    NoProfile,
    #[error("Neu anmelden")]
    ReloginRequired,
    #[error("Netzwerkfehler: {0}")]
    Network(String),
    #[error("{0}")]
    Other(String),
}

impl AuthError {
    pub fn user_message(&self) -> &'static str {
        match self {
            AuthError::Cancelled => "Die Anmeldung wurde abgebrochen.",
            AuthError::Timeout => "Die Anmeldung hat zu lange gedauert. Bitte erneut versuchen.",
            AuthError::NoXboxAccount => "Für dieses Microsoft-Konto gibt es noch kein Xbox-Profil. Bitte einmal auf xbox.com anmelden.",
            AuthError::ChildAccount => "Kinderkonten müssen von einem Erwachsenen zu einer Microsoft-Familie hinzugefügt werden.",
            AuthError::AppNotApproved => "Die FrogClient-App wurde von Mojang noch nicht freigegeben. Bis dahin funktionieren nur Offline-Accounts.",
            AuthError::NoProfile => "Dieses Konto besitzt Minecraft, hat aber noch keinen Spielernamen. Bitte einmal im offiziellen Launcher anlegen.",
            AuthError::ReloginRequired => "Die Anmeldung ist abgelaufen. Bitte neu anmelden.",
            AuthError::Network(_) => "Keine Verbindung zu den Anmeldeservern.",
            AuthError::Other(_) => "Die Anmeldung ist fehlgeschlagen.",
        }
    }
}

impl From<reqwest::Error> for AuthError {
    fn from(e: reqwest::Error) -> Self {
        AuthError::Network(e.to_string())
    }
}

#[cfg(test)]
pub(crate) mod tests {
    use super::AuthEndpoints;
    use wiremock::MockServer;

    pub fn endpoints(server: &MockServer) -> AuthEndpoints {
        let u = server.uri();
        AuthEndpoints {
            authorize: format!("{u}/authorize"),
            token: format!("{u}/token"),
            devicecode: format!("{u}/devicecode"),
            xbl: format!("{u}/xbl"),
            xsts: format!("{u}/xsts"),
            mc_login: format!("{u}/mc_login"),
            entitlements: format!("{u}/entitlements"),
            profile: format!("{u}/profile"),
        }
    }
}
