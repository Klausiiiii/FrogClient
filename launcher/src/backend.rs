//! The real launch steps: Microsoft/offline sessions, game install, instance start.

use crate::accounts::{Account, AccountKind, TokenStore};
use crate::auth::{msa, xbox, AuthEndpoints};
use crate::controller::{Backend, LaunchUpdate, Session};
use crate::fabric::{self, LaunchProfile};
use crate::http::{Http, Progress};
use crate::launch::args::{build_command, redact, LaunchContext};
use crate::launch::instance::{finish_instance, newest_options, prepare_instance};
use crate::launch::process::ProcessManager;
use crate::mojang::install::{install_version, MojangEndpoints};
use crate::mojang::java::install_java;
use crate::mojang::rules::Env;
use crate::paths::AppPaths;
use crate::settings::LauncherSettings;
use std::path::PathBuf;
use std::sync::Arc;

pub const MC_VERSION: &str = "26.3";

pub struct RealBackend {
    pub http: Http,
    pub paths: AppPaths,
    pub mojang: MojangEndpoints,
    pub auth: AuthEndpoints,
    pub tokens: Arc<dyn TokenStore>,
    pub processes: ProcessManager,
    pub settings: LauncherSettings,
    pub mod_jars: Vec<PathBuf>,
    pub frog_jar: Option<PathBuf>,
    pub log: Arc<dyn Fn(&str) + Send + Sync>,
}

pub struct Prepared {
    profile: LaunchProfile,
    java: PathBuf,
}

fn progress_reporter<'a>(report: &'a (dyn Fn(LaunchUpdate) + Send + Sync), what: &'a str) -> impl Fn(Progress) + Send + Sync + 'a {
    move |p: Progress| {
        if p.total > 0 {
            report(LaunchUpdate::Progress(Some(p.done as f32 / p.total as f32)));
            report(LaunchUpdate::Status(format!("{what} {}/{}", p.done, p.total)));
        }
    }
}

impl Backend for RealBackend {
    type Prepared = Prepared;

    fn stagger(&self) -> std::time::Duration {
        std::time::Duration::from_secs(6)
    }

    async fn session(&self, account: &Account) -> Result<Session, String> {
        match account.kind {
            AccountKind::Offline => Ok(Session {
                name: account.name.clone(),
                uuid: account.id.clone(),
                access_token: "0".into(),
                xuid: "0".into(),
                user_type: "legacy".into(),
            }),
            AccountKind::Microsoft { .. } => {
                let refresh_token = self
                    .tokens
                    .get(&account.id)
                    .ok_or_else(|| crate::auth::AuthError::ReloginRequired.user_message().to_string())?;
                let tokens = msa::refresh(&self.http, &self.auth, &refresh_token).await.map_err(|e| {
                    (self.log)(&format!("refresh failed for {}: {e:?}", account.name));
                    e.user_message().to_string()
                })?;
                if let Err(e) = self.tokens.set(&account.id, &tokens.refresh_token) {
                    (self.log)(&format!("could not store refresh token: {e}"));
                }
                let mc = xbox::minecraft_login(&self.http, &self.auth, &tokens.access_token).await.map_err(|e| {
                    (self.log)(&format!("minecraft login failed for {}: {e:?}", account.name));
                    e.user_message().to_string()
                })?;
                if !mc.owns_game {
                    return Err("Dieses Microsoft-Konto besitzt Minecraft: Java Edition nicht.".into());
                }
                Ok(Session { name: mc.name, uuid: mc.uuid, access_token: mc.access_token, xuid: mc.xuid, user_type: "msa".into() })
            }
        }
    }

    async fn prepare(&self, report: &(dyn Fn(LaunchUpdate) + Send + Sync)) -> Result<Prepared, String> {
        let fail = |e: &dyn std::fmt::Display| {
            (self.log)(&format!("install failed: {e}"));
            format!("Installation fehlgeschlagen: {e}")
        };
        report(LaunchUpdate::Status(format!("Lade Minecraft {MC_VERSION} …")));
        let vanilla = install_version(&self.http, &self.paths, &self.mojang, MC_VERSION, progress_reporter(report, "Lade Spieldateien"))
            .await
            .map_err(|e| fail(&e))?;
        report(LaunchUpdate::Status("Lade Fabric …".into()));
        let (fabric_profile, fabric_libs) = fabric::install_fabric(
            &self.http,
            &self.paths,
            fabric::FABRIC_META,
            MC_VERSION,
            &self.settings.loader_version,
            progress_reporter(report, "Lade Fabric"),
        )
        .await
        .map_err(|e| fail(&e))?;
        report(LaunchUpdate::Status("Lade Java …".into()));
        let java = install_java(&self.http, &self.paths, &self.mojang, &vanilla.json.java_version.component, progress_reporter(report, "Lade Java"))
            .await
            .map_err(|e| fail(&e))?;
        let profile = fabric::merge(&self.paths, &vanilla, &fabric_profile, fabric_libs);
        Ok(Prepared { profile, java })
    }

    async fn spawn(&self, account: &Account, session: Session, prepared: &Prepared) -> Result<(), String> {
        let template = newest_options(&self.paths);
        let instance = prepare_instance(&self.paths, &account.id, &self.mod_jars, self.frog_jar.as_deref(), template.as_deref())
            .map_err(|e| format!("Instanz konnte nicht vorbereitet werden: {e}"))?;
        let ctx = LaunchContext {
            java: prepared.java.clone(),
            game_dir: instance.game_dir.clone(),
            assets_root: self.paths.assets(),
            natives_dir: instance.natives_dir.clone(),
            heap_mb: self.settings.heap_mb,
            extra_jvm: self.settings.extra_jvm_args.split_whitespace().map(String::from).collect(),
            player_name: session.name,
            uuid: session.uuid,
            access_token: session.access_token,
            xuid: session.xuid,
            user_type: session.user_type,
            config_dir: self.paths.config(),
            instance_name: account.name.clone(),
        };
        let cmd = build_command(&prepared.profile, &ctx, &Env::current());
        (self.log)(&format!("launch {}: {}", account.name, redact(&cmd).join(" ")));
        let paths = self.paths.clone();
        let log = self.log.clone();
        self.processes
            .spawn(account.id.clone(), cmd, instance.game_dir.clone(), move || {
                if let Err(e) = finish_instance(&paths, &instance) {
                    log(&format!("finish_instance failed: {e}"));
                }
            })
            .await
            .map_err(|e| format!("Java konnte nicht gestartet werden: {e}"))
    }
}
