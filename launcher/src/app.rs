//! Glue between the Slint window (UI thread) and the tokio runtime: owns the launcher
//! state, runs every action in the background and pushes snapshots back to the UI.

use crate::accounts::{can_add_offline, load_launcher_file, save_launcher_file, Account, AccountKind, LauncherFile, TokenStore};
use crate::auth::offline::{offline_uuid, validate_offline_name};
use crate::auth::{msa, xbox, AuthEndpoints, AuthError};
use crate::backend::{RealBackend, MC_VERSION};
use crate::controller::{launch_accounts, start_button_label, AccountRow, LaunchUpdate};
use crate::http::Http;
use crate::launch::process::{available_memory_mb, ram_warning, InstanceEvent, ProcessManager};
use crate::modrinth::{ModVersion, Modrinth, SearchHit, MODRINTH_API};
use crate::mods::{InstalledMod, ModStore, ModsHandle, DEFAULT_CHOICES};
use crate::mojang::install::MojangEndpoints;
use crate::paths::AppPaths;
use crate::update::{self, Release};
use crate::{AccountItem, AppWindow, ModItem, SearchItem};
use slint::{ComponentHandle, ModelRc, VecModel};
use std::collections::{HashMap, HashSet};
use std::io::Write;
use std::sync::{Arc, Mutex};
use tokio::sync::{mpsc, oneshot};

const AVATAR_COLORS: [u32; 6] = [0x5a8f3c, 0xc9822b, 0x4a7fbf, 0xa35bbf, 0xbf4a5a, 0x3fa59b];

#[derive(Default)]
struct State {
    file: LauncherFile,
    errors: HashMap<String, String>,
    running: HashSet<String>,
    stopped_by_user: HashSet<String>,
    status: String,
    progress: Option<f32>,
    launching: bool,
    /// Snapshot of the mod store for the UI; the store itself lives behind `Shared::mods`.
    mods_view: Vec<InstalledMod>,
    results: Vec<SearchHit>,
    updates: HashMap<String, ModVersion>,
    mods_status: String,
    release: Option<Release>,
    update_notice: Option<(String, String)>,
    dialog: Option<i32>,
    dialog_text: String,
    offline_error: String,
    pending_defaults: Option<oneshot::Sender<Option<Vec<String>>>>,
    pending_ram: Option<oneshot::Sender<bool>>,
    /// Dev-only automation: confirm dialogs with defaults (FROGCLIENT_AUTOLAUNCH).
    auto_confirm: bool,
}

struct Shared {
    http: Http,
    paths: AppPaths,
    mojang: MojangEndpoints,
    auth: AuthEndpoints,
    modrinth: Modrinth,
    mods: ModsHandle,
    tokens: Arc<dyn TokenStore>,
    processes: ProcessManager,
    log: Arc<dyn Fn(&str) + Send + Sync>,
}

#[derive(Clone)]
pub struct App {
    ui: slint::Weak<AppWindow>,
    rt: tokio::runtime::Handle,
    state: Arc<Mutex<State>>,
    shared: Arc<Shared>,
}

fn file_logger(paths: &AppPaths) -> Arc<dyn Fn(&str) + Send + Sync> {
    let path = paths.home.join("launcher.log");
    if std::fs::metadata(&path).map(|m| m.len() > 1_000_000).unwrap_or(false) {
        let _ = std::fs::remove_file(&path);
    }
    let lock = Arc::new(Mutex::new(()));
    Arc::new(move |msg: &str| {
        let _guard = lock.lock();
        let secs = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0);
        if let Ok(mut f) = std::fs::OpenOptions::new().create(true).append(true).open(&path) {
            let _ = writeln!(f, "[{secs}] {msg}");
        }
    })
}

impl App {
    pub fn new(ui: &AppWindow, rt: tokio::runtime::Handle, paths: AppPaths, tokens: Arc<dyn TokenStore>) -> App {
        let _ = paths.ensure_dirs();
        let http = Http::new();
        let (tx, rx) = mpsc::unbounded_channel();
        let shared = Arc::new(Shared {
            modrinth: Modrinth::new(http.clone(), MODRINTH_API),
            mods: ModsHandle::new(ModStore { path: paths.mods_json(), mods: Vec::new() }),
            log: file_logger(&paths),
            http,
            paths,
            mojang: MojangEndpoints::default(),
            auth: AuthEndpoints::default(),
            tokens,
            processes: ProcessManager::new(tx),
        });
        let app = App { ui: ui.as_weak(), rt, state: Arc::default(), shared };
        app.spawn_events(rx);
        app
    }

    fn spawn<F: std::future::Future<Output = ()> + Send + 'static>(&self, f: F) {
        self.rt.spawn(f);
    }

    fn log(&self, msg: &str) {
        (self.shared.log)(msg);
    }

    /// Loads the files (after the window is visible) and checks for a new release.
    pub fn start_background(&self) {
        let app = self.clone();
        self.spawn(async move {
            let file = load_launcher_file(&app.shared.paths.launcher_json());
            let mods = ModStore::load(&app.shared.paths);
            let view = mods.mods.clone();
            *app.shared.mods.lock().await = mods;
            {
                let mut s = app.state.lock().unwrap();
                s.file = file;
                s.mods_view = view;
            }
            app.refresh();
            if cfg!(feature = "dev") && std::env::var_os("FROGCLIENT_AUTOLAUNCH").is_some() {
                app.state.lock().unwrap().auto_confirm = true;
                app.log("dev autolaunch");
                app.start();
            }
            match update::latest_release(&app.shared.http, update::GITHUB_API, update::REPO).await {
                Ok(Some(release)) => {
                    let mut s = app.state.lock().unwrap();
                    if update::is_newer(&release.tag, env!("CARGO_PKG_VERSION")) {
                        s.update_notice = Some((format!("Neue Version {} verfügbar", release.tag), release.html_url.clone()));
                    }
                    s.release = Some(release);
                }
                Ok(None) => {}
                Err(e) => app.log(&format!("release check failed: {e}")),
            }
            app.refresh();
        });
    }

    fn spawn_events(&self, mut rx: mpsc::UnboundedReceiver<InstanceEvent>) {
        let app = self.clone();
        self.spawn(async move {
            while let Some(ev) = rx.recv().await {
                {
                    let mut s = app.state.lock().unwrap();
                    match ev {
                        InstanceEvent::Started { account_id } => {
                            s.errors.remove(&account_id);
                            s.running.insert(account_id);
                        }
                        InstanceEvent::Exited { account_id, code, log } => {
                            s.running.remove(&account_id);
                            let by_user = s.stopped_by_user.remove(&account_id);
                            if !by_user && code != Some(0) {
                                s.errors.insert(account_id, format!("Abgestürzt – Log: {}", log.display()));
                            }
                        }
                    }
                }
                app.refresh();
            }
        });
    }

    fn save_file(&self, s: &State) {
        if let Err(e) = save_launcher_file(&self.shared.paths.launcher_json(), &s.file) {
            self.log(&format!("could not save launcher.json: {e}"));
        }
    }

    /// Pushes a snapshot of the state to the UI thread.
    pub fn refresh(&self) {
        let s = self.state.lock().unwrap();
        let rows: Vec<AccountRow> = s
            .file
            .accounts
            .iter()
            .map(|a| AccountRow {
                id: a.id.clone(),
                name: a.name.clone(),
                offline: a.kind == AccountKind::Offline,
                selected: s.file.selected.contains(&a.id),
                running: s.running.contains(&a.id),
                error: s.errors.get(&a.id).cloned(),
            })
            .collect();
        let label = if s.launching { None } else { start_button_label(&rows) };
        let start_label = label.clone().unwrap_or_else(|| {
            if s.launching {
                "Startet …".into()
            } else if rows.is_empty() {
                "Erst einen Account hinzufügen".into()
            } else if rows.iter().any(|r| r.selected) {
                "Läuft bereits".into()
            } else {
                "Account auswählen".into()
            }
        });
        let accounts: Vec<AccountItem> = rows
            .iter()
            .map(|r| {
                let hash = r.id.bytes().fold(0usize, |h, b| h.wrapping_mul(31).wrapping_add(b as usize));
                let rgb = AVATAR_COLORS[hash % AVATAR_COLORS.len()];
                AccountItem {
                    id: r.id.clone().into(),
                    name: r.name.clone().into(),
                    initial: r.name.chars().next().map(|c| c.to_uppercase().to_string()).unwrap_or_default().into(),
                    color: slint::Color::from_rgb_u8((rgb >> 16) as u8, (rgb >> 8) as u8, rgb as u8),
                    offline: r.offline,
                    selected: r.selected,
                    running: r.running,
                    error: r.error.clone().unwrap_or_default().into(),
                }
            })
            .collect();
        let installed: HashSet<String> = s.mods_view.iter().map(|x| x.project_id.clone()).collect();
        let mods: Vec<ModItem> = s
            .mods_view
            .iter()
            .map(|m| ModItem {
                id: m.project_id.clone().into(),
                title: m.title.clone().into(),
                version: m.version_number.clone().into(),
                enabled: m.enabled,
                required: m.required,
                has_update: s.updates.contains_key(&m.project_id),
            })
            .collect();
        let results: Vec<SearchItem> = s
            .results
            .iter()
            .map(|r| SearchItem {
                id: r.project_id.clone().into(),
                title: r.title.clone().into(),
                description: r.description.clone().into(),
                downloads: format_count(r.downloads).into(),
                installed: installed.contains(&r.project_id),
            })
            .collect();
        let settings = s.file.settings.clone();
        let status = s.status.clone();
        let progress = s.progress.unwrap_or(-1.0);
        let can_offline = can_add_offline(&s.file.accounts);
        let mods_status = s.mods_status.clone();
        let (notice, url) = s.update_notice.clone().unwrap_or_default();
        let dialog = s.dialog;
        let dialog_text = s.dialog_text.clone();
        let offline_error = s.offline_error.clone();
        drop(s);
        // Only reset `dialog` when the state asks for it, so UI-side navigation survives refreshes.
        if dialog.is_some() {
            self.state.lock().unwrap().dialog = None;
        }

        let _ = self.ui.upgrade_in_event_loop(move |w| {
            w.set_accounts(ModelRc::new(VecModel::from(accounts)));
            w.set_start_label(start_label.into());
            w.set_start_enabled(label.is_some());
            w.set_status(status.into());
            w.set_progress(progress);
            w.set_can_add_offline(can_offline);
            w.set_mods(ModelRc::new(VecModel::from(mods)));
            w.set_results(ModelRc::new(VecModel::from(results)));
            w.set_mods_status(mods_status.into());
            w.set_heap_mb(settings.heap_mb as i32);
            w.set_jvm_args(settings.extra_jvm_args.into());
            w.set_minimize_on_launch(settings.minimize_on_launch);
            w.set_version_text(format!("FrogClient-Launcher {} · Minecraft {MC_VERSION}", env!("CARGO_PKG_VERSION")).into());
            w.set_update_notice(notice.into());
            w.set_update_url(url.into());
            w.set_dialog_text(dialog_text.into());
            w.set_offline_error(offline_error.into());
            if let Some(d) = dialog {
                w.set_dialog(d);
            }
        });
    }

    fn update<R>(&self, f: impl FnOnce(&mut State) -> R) -> R {
        let r = f(&mut self.state.lock().unwrap());
        self.refresh();
        r
    }

    // ── Accounts ─────────────────────────────────────────────────────

    pub fn toggle_account(&self, id: String) {
        self.update(|s| {
            if let Some(i) = s.file.selected.iter().position(|x| *x == id) {
                s.file.selected.remove(i);
            } else {
                s.file.selected.push(id);
            }
            self.save_file(s);
        });
    }

    pub fn remove_account(&self, id: String) {
        self.update(|s| {
            if s.running.contains(&id) {
                return;
            }
            s.file.accounts.retain(|a| a.id != id);
            s.file.selected.retain(|x| *x != id);
            s.errors.remove(&id);
            self.save_file(s);
        });
        self.shared.tokens.delete(&id);
    }

    pub fn stop_account(&self, id: String) {
        self.state.lock().unwrap().stopped_by_user.insert(id.clone());
        self.shared.processes.kill(&id);
    }

    pub fn add_offline(&self, name: String) {
        let name = name.trim().to_string();
        self.update(|s| {
            if !can_add_offline(&s.file.accounts) {
                s.offline_error = "Offline-Accounts sind erst mit einem Microsoft-Account möglich, der Minecraft besitzt.".into();
                return;
            }
            if let Err(e) = validate_offline_name(&name) {
                s.offline_error = e.into();
                return;
            }
            let id = offline_uuid(&name);
            if s.file.accounts.iter().any(|a| a.id == id) {
                s.offline_error = "Diesen Account gibt es schon.".into();
                return;
            }
            s.file.accounts.push(Account { id: id.clone(), name, kind: AccountKind::Offline });
            s.file.selected.push(id);
            s.offline_error.clear();
            s.dialog = Some(0);
            self.save_file(s);
        });
    }

    pub fn add_microsoft(&self) {
        self.update(|s| {
            s.dialog = Some(3);
            s.dialog_text = "Im Browser hat sich die Microsoft-Anmeldung geöffnet. Melde dich dort an und kehre dann hierher zurück.".into();
        });
        let app = self.clone();
        self.spawn(async move {
            let sh = &app.shared;
            let tokens = match msa::login_interactive(&sh.http, &sh.auth).await {
                Ok(t) => Ok(t),
                Err(AuthError::Cancelled) => Err(AuthError::Cancelled),
                Err(e) => {
                    app.log(&format!("browser login failed ({e:?}), falling back to device code"));
                    let ui = app.clone();
                    msa::login_device_code(&sh.http, &sh.auth, move |code, uri| {
                        ui.update(|s| {
                            s.dialog = Some(3);
                            s.dialog_text = format!("Öffne {uri} und gib diesen Code ein:\n\n{code}");
                        });
                    })
                    .await
                }
            };
            let result = match tokens {
                Ok(t) => xbox::minecraft_login(&sh.http, &sh.auth, &t.access_token).await.map(|mc| (t, mc)),
                Err(e) => Err(e),
            };
            match result {
                Ok((tokens, mc)) => {
                    if let Err(e) = sh.tokens.set(&mc.uuid, &tokens.refresh_token) {
                        app.log(&format!("could not store refresh token: {e}"));
                    }
                    app.update(|s| {
                        s.file.accounts.retain(|a| a.id != mc.uuid);
                        s.file.accounts.push(Account { id: mc.uuid.clone(), name: mc.name.clone(), kind: AccountKind::Microsoft { owns_game: mc.owns_game } });
                        if !s.file.selected.contains(&mc.uuid) {
                            s.file.selected.push(mc.uuid.clone());
                        }
                        if mc.owns_game {
                            s.dialog = Some(0);
                        } else {
                            s.dialog_text = format!("{} ist angemeldet, besitzt aber Minecraft: Java Edition nicht.", mc.name);
                        }
                        app.save_file(s);
                    });
                }
                Err(e) => {
                    app.log(&format!("microsoft login failed: {e:?}"));
                    app.update(|s| {
                        s.dialog = Some(3);
                        s.dialog_text = e.user_message().into();
                    });
                }
            }
        });
    }

    // ── Launch ───────────────────────────────────────────────────────

    pub fn start(&self) {
        let accounts: Vec<Account> = {
            let mut s = self.state.lock().unwrap();
            if s.launching {
                return;
            }
            let picked: Vec<Account> = s
                .file
                .accounts
                .iter()
                .filter(|a| s.file.selected.contains(&a.id) && !s.running.contains(&a.id))
                .cloned()
                .collect();
            if picked.is_empty() {
                return;
            }
            s.launching = true;
            for a in &picked {
                s.errors.remove(&a.id);
            }
            picked
        };
        self.refresh();
        let app = self.clone();
        self.spawn(async move {
            app.launch(accounts).await;
            app.update(|s| {
                s.launching = false;
                s.progress = None;
            });
        });
    }

    async fn launch(&self, accounts: Vec<Account>) {
        let sh = &self.shared;

        // First setup: let the user pick the performance mods.
        let first_setup = sh.mods.lock().await.needs_first_setup();
        if first_setup {
            let (tx, rx) = oneshot::channel();
            self.update(|s| {
                s.pending_defaults = Some(tx);
                s.dialog = Some(4);
            });
            if self.state.lock().unwrap().auto_confirm {
                self.confirm_defaults(true, true, true);
            }
            let Ok(Some(choices)) = rx.await else { return };
            self.update(|s| s.status = "Installiere Fabric API und Performance-Mods …".into());
            let choice_refs: Vec<&str> = choices.iter().map(String::as_str).collect();
            let result = sh.mods.lock().await.ensure_defaults(&sh.modrinth, &sh.paths, MC_VERSION, &choice_refs).await;
            self.sync_mods().await;
            if let Err(e) = result {
                self.log(&format!("default mods failed: {e}"));
                self.update(|s| s.status = e.user_message());
                return;
            }
        }

        let settings = self.state.lock().unwrap().file.settings.clone();
        if let Some(warning) = ram_warning(settings.heap_mb, accounts.len(), available_memory_mb()) {
            let (tx, rx) = oneshot::channel();
            self.update(|s| {
                s.pending_ram = Some(tx);
                s.dialog_text = warning;
                s.dialog = Some(5);
            });
            if self.state.lock().unwrap().auto_confirm {
                self.confirm_ram(true);
            }
            if !rx.await.unwrap_or(false) {
                self.update(|s| s.status.clear());
                return;
            }
        }

        let release = self.state.lock().unwrap().release.clone();
        let frog_jar = update::frog_mod_jar(&sh.http, &sh.paths, release.as_ref()).await;
        if frog_jar.is_none() {
            self.log("FrogClient mod jar not found");
        }
        // Waits for a running install or update, so a game never starts with a half-updated mod list.
        let mod_jars = sh.mods.enabled_jars(&sh.paths).await;
        let backend = RealBackend {
            http: sh.http.clone(),
            paths: sh.paths.clone(),
            mojang: sh.mojang.clone(),
            auth: sh.auth.clone(),
            tokens: sh.tokens.clone(),
            processes: sh.processes.clone(),
            settings: settings.clone(),
            mod_jars,
            frog_jar: frog_jar.clone(),
            log: sh.log.clone(),
        };
        let started = Arc::new(Mutex::new(0usize));
        let counter = started.clone();
        let app = self.clone();
        let report = move |u: LaunchUpdate| {
            app.update(|s| match u {
                LaunchUpdate::Status(t) => s.status = t,
                LaunchUpdate::Progress(p) => s.progress = p,
                LaunchUpdate::AccountError { id, message } => {
                    s.errors.insert(id, message);
                }
                LaunchUpdate::Started { .. } => *counter.lock().unwrap() += 1,
            });
        };
        launch_accounts(&backend, accounts, &report).await;

        if frog_jar.is_none() {
            self.update(|s| s.status = "Hinweis: FrogClient-Mod nicht gefunden – gestartet ohne Client-Features.".into());
        }
        if settings.minimize_on_launch && *started.lock().unwrap() > 0 {
            let _ = self.ui.upgrade_in_event_loop(|w| w.window().set_minimized(true));
        }
    }

    pub fn confirm_defaults(&self, sodium: bool, lithium: bool, ferrite: bool) {
        let picks: Vec<String> = DEFAULT_CHOICES
            .iter()
            .zip([sodium, lithium, ferrite])
            .filter(|(_, on)| *on)
            .map(|(slug, _)| slug.to_string())
            .collect();
        self.update(|s| {
            s.dialog = Some(0);
            if let Some(tx) = s.pending_defaults.take() {
                let _ = tx.send(Some(picks));
            }
        });
    }

    pub fn confirm_ram(&self, go: bool) {
        self.update(|s| {
            s.dialog = Some(0);
            if let Some(tx) = s.pending_ram.take() {
                let _ = tx.send(go);
            }
        });
    }

    pub fn close_dialog(&self) {
        self.update(|s| {
            s.dialog = Some(0);
            s.offline_error.clear();
            if let Some(tx) = s.pending_defaults.take() {
                let _ = tx.send(None);
            }
            if let Some(tx) = s.pending_ram.take() {
                let _ = tx.send(false);
            }
        });
    }

    // ── Mods ─────────────────────────────────────────────────────────

    pub fn search(&self, query: String) {
        self.update(|s| s.mods_status = "Suche …".into());
        let app = self.clone();
        self.spawn(async move {
            match app.shared.modrinth.search(query.trim(), MC_VERSION, 0).await {
                Ok(hits) => app.update(|s| {
                    s.mods_status = if hits.is_empty() { "Keine passenden Mods für 26.3 gefunden.".into() } else { String::new() };
                    s.results = hits;
                }),
                Err(e) => app.update(|s| s.mods_status = format!("Modrinth ist nicht erreichbar ({e}).")),
            }
        });
    }

    async fn sync_mods(&self) {
        let view = self.shared.mods.snapshot().await;
        self.update(|s| s.mods_view = view);
    }

    pub fn install_mod(&self, project: String) {
        self.update(|s| s.mods_status = "Installiere …".into());
        let app = self.clone();
        self.spawn(async move {
            let sh = app.shared.clone();
            let result = sh.mods.install(&sh.modrinth, &sh.paths, &project, MC_VERSION).await;
            app.sync_mods().await;
            app.update(|s| {
                s.mods_status = match result {
                    Ok(titles) if titles.is_empty() => "Bereits installiert.".into(),
                    Ok(titles) => format!("Installiert: {} (gilt ab dem nächsten Start)", titles.join(", ")),
                    Err(e) => e.user_message(),
                }
            });
        });
    }

    pub fn toggle_mod(&self, project: String, enabled: bool) {
        let app = self.clone();
        self.spawn(async move {
            let result = app.shared.mods.lock().await.set_enabled(&project, enabled);
            if let Err(e) = result {
                app.update(|s| s.mods_status = e.user_message());
            }
            app.sync_mods().await;
        });
    }

    pub fn remove_mod(&self, project: String) {
        let app = self.clone();
        self.spawn(async move {
            let result = app.shared.mods.lock().await.remove(&project, &app.shared.paths);
            app.update(|s| {
                if let Err(e) = result {
                    s.mods_status = e.user_message();
                }
                s.updates.remove(&project);
            });
            app.sync_mods().await;
        });
    }

    pub fn check_updates(&self) {
        self.update(|s| s.mods_status = "Suche nach Updates …".into());
        let app = self.clone();
        self.spawn(async move {
            let sh = app.shared.clone();
            let result = sh.mods.lock().await.check_updates(&sh.modrinth, MC_VERSION).await;
            app.update(|s| match result {
                Ok(list) => {
                    s.mods_status = if list.is_empty() { "Alle Mods sind aktuell.".into() } else { format!("{} Update(s) verfügbar.", list.len()) };
                    s.updates = list.into_iter().collect();
                }
                Err(e) => s.mods_status = e.user_message(),
            });
        });
    }

    pub fn update_mod(&self, project: String) {
        let Some(version) = self.state.lock().unwrap().updates.get(&project).cloned() else { return };
        self.update(|s| s.mods_status = "Aktualisiere …".into());
        let app = self.clone();
        self.spawn(async move {
            let sh = app.shared.clone();
            let result = sh.mods.lock().await.update(&sh.modrinth, &sh.paths, &project, &version).await;
            app.sync_mods().await;
            app.update(|s| match result {
                Ok(()) => {
                    s.updates.remove(&project);
                    s.mods_status = "Aktualisiert (gilt ab dem nächsten Start).".into();
                }
                Err(e) => s.mods_status = e.user_message(),
            });
        });
    }

    // ── Settings ─────────────────────────────────────────────────────

    pub fn save_settings(&self, heap_mb: i32, jvm_args: String, minimize: bool) {
        let mut s = self.state.lock().unwrap();
        s.file.settings.heap_mb = heap_mb.clamp(1024, 16384) as u32;
        s.file.settings.extra_jvm_args = jvm_args;
        s.file.settings.minimize_on_launch = minimize;
        self.save_file(&s);
    }
}

fn format_count(n: u64) -> String {
    match n {
        0..1_000 => n.to_string(),
        1_000..1_000_000 => format!("{:.1} Tsd.", n as f64 / 1_000.0).replace('.', ","),
        _ => format!("{:.1} Mio.", n as f64 / 1_000_000.0).replace('.', ","),
    }
}

