# FrogClient Launcher – Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Ein nativer Rust-Launcher (Slint-UI), der Minecraft 26.3 + Fabric + Java 25 selbst installiert, Microsoft- und Offline-Accounts verwaltet, mehrere Accounts gleichzeitig startet und Fabric-Mods über Modrinth installiert.

**Architecture:** Eine Cargo-Binary in `/launcher`. Backend-Module sind reine Logik bzw. HTTP-Clients mit injizierbaren Basis-URLs (getestet mit `wiremock` und Fixture-JSONs); ein `AppController` hält den UI-Zustand und verbindet Slint (UI-Thread) mit einer tokio-Runtime (Hintergrund-Thread). Beim Start wird nichts aus dem Netz geladen, bevor das Fenster sichtbar ist.

**Tech Stack:** Rust stable (Edition 2024, MSVC-Toolchain), slint 1.18, tokio 1, reqwest 0.13 (json, stream), serde/serde_json, sha1 0.11 + sha2 + hex + base64 + md-5, rand, url, keyring 4, directories 6, sysinfo 0.39, open 5, junction (nur Windows), thiserror; dev: wiremock, tempfile.

**Spec:** `docs/superpowers/specs/2026-10-17-frogclient-design.md` (Abschnitt 6.2, Erfolgskriterien 1–5)

## Global Constraints

- Minecraft `26.3`, Fabric Loader `0.19.5` (Default, aus Fabric-Meta änderbar), Java-Runtime-Komponente aus dem Version-JSON (`java-runtime-epsilon`, Java 25).
- Azure Client-ID `2eb1f5a4-f452-4b42-acd4-880340c33f4c`, Authority `consumers`, Scope `XboxLive.signin offline_access`, Redirect `http://localhost:<zufälliger Port>`; kein Client-Secret.
- Datenordner: `directories::BaseDirs::config_dir()/FrogClient` (Windows: `%APPDATA%\FrogClient`); Override per Env `FROGCLIENT_HOME` (Tests/Entwicklung).
- Verzeichnisaufbau exakt wie Spec 6.2: `shared/{versions,libraries,assets,runtimes}`, `mods/` + `mods/mods.json`, `config/` (FrogClient-Config, an die Mod per `-Dfrogclient.configDir=<home>/config`), `shared-gamedata/{servers.dat,resourcepacks,shaderpacks,screenshots,config}`, `instances/<account-id>/`, `launcher.json`.
- Tokens: Microsoft-Refresh-Token nur im OS-Credential-Store (keyring, Service `FrogClient`, User = Account-ID); niemals in Dateien oder Logs.
- Offline-Accounts in Release-Builds nur anlegbar, wenn ein Microsoft-Account mit Minecraft-Besitz existiert; Cargo-Feature `dev` hebt das auf.
- Standard-Heap 3072 MB pro Instanz. Standard-Mods bei Ersteinrichtung: `fabric-api` (Pflicht), `sodium`, `lithium`, `ferrite-core` (abwählbar).
- Kein Netzwerkzugriff und kein Blockieren vor dem ersten Fenster-Frame; Ziel Fenster sichtbar < 500 ms.
- Code plattformneutral halten: Windows-Spezifisches nur hinter `#[cfg(windows)]` in `platform.rs`.
- UI-Texte deutsch.

## Review Focus

1. **Abgebrochener/teilweiser Download** (Netz weg, Launcher geschlossen) → nächster Start lädt nur fehlende/kaputte Dateien nach (SHA1-Prüfung), nie eine halbe Datei als gültig. (Task 2)
2. **Zwei Instanzen ändern gleichzeitig die Serverliste** → keine Instanz verliert ihre Änderung komplett; die zuletzt beendete Instanz gewinnt, Datei nie korrupt. (Task 9)
3. **Abgelaufener/widerrufener Refresh-Token oder Mojang-App noch nicht freigegeben** → klare deutsche Meldung („App-ID noch nicht von Mojang freigegeben“ bzw. „Bitte neu anmelden“), Account bleibt in der Liste, andere Accounts starten trotzdem. (Task 8, Task 14)
4. **Modrinth-Mod ohne 26.3-Version oder mit Abhängigkeit, die es nicht gibt** → nicht installierbar mit Begründung; nie eine halbe Installation. (Task 11)
5. **Instanz stürzt ab oder wird extern beendet** → Status „läuft“ verschwindet, Hinweis mit Pfad zu `latest.log`; andere Instanzen unbeeinflusst. (Task 10)

---

## Dateistruktur

```
launcher/
  Cargo.toml, build.rs (slint-build)
  ui/app.slint                    Layout A: Nav-Leiste, Tabs Spielen/Mods/Einstellungen
  src/main.rs                     Start: Fenster zeigen, dann Runtime + Controller
  src/paths.rs                    AppPaths
  src/platform.rs                 Junction/Symlink, Hardlink, Klassenpfad-Trenner
  src/http.rs                     Http-Client, Downloader mit SHA1 + Parallelität
  src/mojang/{mod,model,rules,install,java}.rs
  src/fabric.rs                   Profil laden + mit Vanilla mergen
  src/launch/{mod,args,instance,process}.rs
  src/auth/{mod,msa,xbox,offline}.rs
  src/accounts.rs                 Account-Liste + TokenStore
  src/modrinth.rs                 API-Client
  src/mods.rs                     ModStore (mods.json), Install/Update/Defaults
  src/settings.rs                 LauncherSettings
  src/update.rs                   GitHub-Release-Prüfung
  src/controller.rs               AppController (UI-Zustand, Aktionen)
  tests/fixtures/                 26.3.json, fabric-profile.json, manifest.json (aus Recon), Modrinth-Antworten
mod/ (Task 13)                    Fenstertitel aus -Dfrogclient.instanceName
```

---

### Task 1: Toolchain, Crate-Gerüst, AppPaths, leeres Fenster

**Files:**
- Create: `launcher/Cargo.toml`, `launcher/build.rs`, `launcher/ui/app.slint`, `launcher/src/main.rs`, `launcher/src/paths.rs`
- Modify: `/.gitignore` (`launcher/target/`)

**Interfaces:**
- Produces: `pub struct AppPaths { pub home: PathBuf }` mit `fn resolve() -> io::Result<AppPaths>` (Env `FROGCLIENT_HOME` vor `BaseDirs`), `fn ensure_dirs(&self) -> io::Result<()>` und Pfad-Gettern: `versions()`, `libraries()`, `assets()`, `runtimes()`, `mods()`, `mods_json()`, `config()`, `shared_gamedata()`, `instances()`, `instance(id: &str)`, `launcher_json()`.

- [ ] **Step 1: Rust installieren.** `winget install --id Rustlang.Rustup -e`, danach `rustup default stable-x86_64-pc-windows-msvc`. Run: `cargo --version` → Version ≥ 1.90.
- [ ] **Step 2: Crate anlegen.** `cargo new launcher --name frogclient-launcher`; Dependencies aus Tech Stack; Feature `dev = []`; `[profile.release] lto = "thin", codegen-units = 1, strip = true`; Windows-Subsystem `windows` in Release (`#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]`).
- [ ] **Step 3: Failing Test** in `paths.rs`:

```rust
#[test] fn layout_matches_spec() {
    let p = AppPaths { home: PathBuf::from("H") };
    assert_eq!(p.versions(), PathBuf::from("H/shared/versions"));
    assert_eq!(p.mods_json(), PathBuf::from("H/mods/mods.json"));
    assert_eq!(p.instance("abc"), PathBuf::from("H/instances/abc"));
    assert_eq!(p.shared_gamedata(), PathBuf::from("H/shared-gamedata"));
}
#[test] fn env_override_wins() { /* FROGCLIENT_HOME=tmp → resolve().home == tmp */ }
#[test] fn ensure_dirs_creates_all() { /* tempdir → alle Ordner existieren */ }
```

- [ ] **Step 4:** `cargo test` → FAIL; implementieren; `cargo test` → PASS.
- [ ] **Step 5: Fenster.** `app.slint` mit `AppWindow` (Größe 900×560, Hintergrund `#121714`, Titel „FrogClient“, Nav-Leiste links 46 px mit drei Icons ▶ 🧩 ⚙, Platzhalter-Inhalt). `main.rs`: `AppWindow::new()?.run()`. Run: `cargo run` → Fenster erscheint.
- [ ] **Step 6: Commit** `feat(launcher): scaffold crate, app paths, window`

---

### Task 2: HTTP-Downloader mit SHA1, Wiederaufnahme, Parallelität

**Files:**
- Create: `launcher/src/http.rs`
- Test: in-file `#[cfg(test)]` mit `wiremock`

**Interfaces:**
- Produces:
  - `pub struct Http { client: reqwest::Client }` – `fn new() -> Http` (User-Agent `FrogClient-Launcher/<version>`, Timeout 30 s), `async fn get_json<T: DeserializeOwned>(&self, url: &str) -> Result<T, HttpError>`.
  - `pub struct DownloadJob { pub url: String, pub dest: PathBuf, pub sha1: Option<String>, pub size: Option<u64> }`
  - `async fn download_all(&self, jobs: Vec<DownloadJob>, parallel: usize, progress: impl Fn(Progress) + Send + Sync) -> Result<(), HttpError>` – überspringt Jobs, deren Ziel existiert und deren SHA1 passt; schreibt nach `<dest>.part`, prüft SHA1, dann `rename`; 3 Versuche pro Job; `Progress { done: usize, total: usize, bytes: u64 }`.
  - `fn sha1_file(path: &Path) -> io::Result<String>` (hex, lowercase).
  - `enum HttpError { Network(String), Status(u16, String), Hash { url: String }, Io(String) }` via thiserror.

- [ ] **Step 1: Failing Tests:**

```rust
#[tokio::test] async fn downloads_and_verifies()          // Server liefert "hello", sha1 aaf4c61d… → Datei vorhanden, kein .part
#[tokio::test] async fn skips_existing_valid_file()        // Datei vorhanden mit richtigem Hash → Server erwartet 0 Requests (wiremock .expect(0))
#[tokio::test] async fn replaces_corrupt_existing_file()   // falscher Inhalt → neu geladen
#[tokio::test] async fn hash_mismatch_fails_and_leaves_no_file() // Server liefert falschen Inhalt → Err(Hash), weder dest noch .part existiert
#[tokio::test] async fn retries_then_succeeds()            // 2× 500, dann 200 → Ok
#[tokio::test] async fn reports_progress_total()           // 3 Jobs → letzter Progress done==3,total==3
```

- [ ] **Step 2:** `cargo test http` → FAIL; implementieren (Streaming in Datei, `futures::stream::iter(..).buffer_unordered(parallel)`); → PASS.
- [ ] **Step 3: Commit** `feat(launcher): verified parallel downloader`

---

### Task 3: Mojang-Modell + Regel-Auswertung

**Files:**
- Create: `launcher/src/mojang/{mod,model,rules}.rs`, `launcher/tests/fixtures/{manifest.json,26.3.json}` (aus `.superpowers/recon/`)

**Interfaces:**
- Produces:
  - serde-Modelle: `VersionManifest { versions: Vec<ManifestEntry { id, url, sha1 }> }`, `VersionJson { id, main_class, arguments: Arguments { game: Vec<Arg>, jvm: Vec<Arg> }, asset_index: AssetIndexRef { id, url, sha1 }, assets, downloads: { client: Artifact }, java_version: { component, major_version }, libraries: Vec<Library>, logging: Option<Logging>, r#type }`; `enum Arg { Plain(String), Conditional { rules: Vec<Rule>, value: OneOrMany } }` (untagged); `Library { name, downloads: Option<{ artifact: Option<Artifact> }>, url: Option<String>, sha1: Option<String>, rules: Option<Vec<Rule>> }`; `Artifact { path: Option<String>, url, sha1, size }`.
  - `struct Env { os: &'static str /* "windows"|"osx"|"linux" */, arch: &'static str /* "x86_64"|"x86"|"arm64" */ }` + `fn current() -> Env`.
  - `fn rules_allow(rules: &[Rule], env: &Env, features: &Features) -> bool` – Mojang-Semantik: Default deny wenn Regeln vorhanden; letzte passende Regel gewinnt; `os.arch` `"x86"` passt nur auf 32-Bit.
  - `struct Features` – alle Felder `false` (is_demo_user, has_custom_resolution, has_quick_plays_support, is_quick_play_*).
  - `fn select_libraries(v: &VersionJson, env: &Env) -> Vec<&Library>`.

- [ ] **Step 1: Failing Tests:**

```rust
#[test] fn parses_26_3_fixture()              // id "26.3", main_class "net.minecraft.client.main.Main", java component "java-runtime-epsilon", major 25
#[test] fn windows_x64_skips_osx_and_linux_natives() // keine Library mit "natives-macos"/"natives-linux"; enthält "natives-windows" (falls in Fixture vorhanden) – keine "java-objc-bridge"
#[test] fn x86_rule_only_on_32bit()           // jvm Arg "-Xss1M" nur bei arch "x86"
#[test] fn feature_rules_off_by_default()     // "--demo" nicht in aufgelösten Game-Args
#[test] fn no_rules_means_allow()
```

- [ ] **Step 2:** FAIL → implementieren → PASS. **Step 3: Commit** `feat(launcher): Mojang version model and rules`

---

### Task 4: Vanilla-Installation + Java-Runtime

**Files:**
- Create: `launcher/src/mojang/{install,java}.rs`

**Interfaces:**
- Consumes: `Http`, `DownloadJob`, `AppPaths`, Modelle aus Task 3.
- Produces:
  - `struct MojangEndpoints { pub manifest: String, pub java_index: String }` mit `Default` = `https://piston-meta.mojang.com/mc/game/version_manifest_v2.json` bzw. `https://launchermeta.mojang.com/v1/products/java-runtime/2ec0cc96c44e5a76b9c8b7c39df7210883d12871/all.json`; Asset-Objekte von `https://resources.download.minecraft.net/<2>/<hash>`.
  - `async fn install_version(http, paths, ep, version_id: &str, progress) -> Result<InstalledVersion, InstallError>` – lädt/cached Version-JSON nach `versions/<id>/<id>.json` (SHA1 aus Manifest), Client-JAR `versions/<id>/<id>.jar`, alle erlaubten Libraries nach `libraries/<path>`, Asset-Index `assets/indexes/<id>.json`, Assets `assets/objects/<2>/<hash>`, Logging-Config `assets/log_configs/<id>`. Liefert `InstalledVersion { json: VersionJson, client_jar: PathBuf, library_paths: Vec<PathBuf>, logging_config: Option<PathBuf> }`.
  - `async fn install_java(http, paths, ep, component: &str, progress) -> Result<PathBuf /* javaw.exe bzw. java */, InstallError>` – Plattform-Schlüssel `windows-x64`/`windows-arm64`/`linux`/`mac-os`/`mac-os-arm64`; lädt Manifest, legt Verzeichnisse an, lädt `file`-Einträge (raw-Download, SHA1), ignoriert `link` auf Windows; Ziel `runtimes/<component>/`. Executable: `bin/javaw.exe` (Windows), sonst `bin/java`.
  - Offline-Fall: ist `versions/<id>/<id>.json` lokal vorhanden und Manifest nicht erreichbar → lokale Datei verwenden.

- [ ] **Step 1: Failing Tests** (wiremock mit Mini-Manifest: 1 Version, 1 Library, 2 Assets, Java-Manifest mit 1 Ordner + 1 Datei):

```rust
#[tokio::test] async fn installs_minimal_version_layout()   // alle Dateien an den Spec-Pfaden, Hashes stimmen
#[tokio::test] async fn second_install_makes_no_downloads() // .expect(0) auf alle Datei-Mocks außer Manifest
#[tokio::test] async fn uses_cached_json_when_offline()     // Manifest-Mock 500 → Ok mit lokaler JSON
#[tokio::test] async fn java_install_returns_executable()   // Pfad endet auf bin/javaw.exe (cfg windows)
```

- [ ] **Step 2:** FAIL → implementieren → PASS. **Step 3: Commit** `feat(launcher): install vanilla 26.3 and Java runtime`

---

### Task 5: Fabric-Profil

**Files:**
- Create: `launcher/src/fabric.rs`, `launcher/tests/fixtures/fabric-profile.json`

**Interfaces:**
- Consumes: `VersionJson`, `Http`, `install`-Download-Logik.
- Produces:
  - `const FABRIC_META: &str = "https://meta.fabricmc.net"`; `async fn fetch_profile(http, base: &str, mc: &str, loader: &str) -> Result<FabricProfile, HttpError>` (`/v2/versions/loader/{mc}/{loader}/profile/json`).
  - `FabricProfile { id, main_class, arguments: { game: Vec<String>, jvm: Vec<String> }, libraries: Vec<FabricLibrary { name, url, sha1: Option<String>, size: Option<u64> }> }`.
  - `fn maven_path(coord: &str) -> String` – `group:artifact:version[:classifier]` → `group/als/pfad/artifact/version/artifact-version[-classifier].jar`.
  - `fn merge(vanilla: &InstalledVersion, fabric: &FabricProfile, fabric_lib_paths: Vec<PathBuf>) -> LaunchProfile` – `LaunchProfile { main_class (Fabric), classpath: Vec<PathBuf> (Fabric-Libs zuerst, dann Vanilla-Libs ohne Duplikate gleicher group:artifact – Fabric gewinnt, zuletzt Client-JAR), extra_jvm_args (Fabric), json (Vanilla) , logging_config }`.

- [ ] **Step 1: Failing Tests:**

```rust
#[test] fn maven_path_basic()      // "org.ow2.asm:asm:9.10.1" → "org/ow2/asm/asm/9.10.1/asm-9.10.1.jar"
#[test] fn maven_path_classifier() // "a.b:c:1:natives-windows" → "a/b/c/1/c-1-natives-windows.jar"
#[test] fn parses_fixture_main_class() // "net.fabricmc.loader.impl.launch.knot.KnotClient"
#[test] fn merge_dedups_by_group_artifact_fabric_wins() // Vanilla asm 9.6 + Fabric asm 9.10.1 → nur 9.10.1 im Classpath
#[test] fn merge_puts_client_jar_last()
```

- [ ] **Step 2:** FAIL → implementieren → PASS. Fabric-Libraries über `Http::download_all` nach `libraries/<maven_path>` (URL = `url` + Pfad; SHA1 falls vorhanden). **Step 3: Commit** `feat(launcher): Fabric profile merge`

---

### Task 6: Start-Argumente (rein)

**Files:**
- Create: `launcher/src/launch/{mod,args}.rs`

**Interfaces:**
- Consumes: `LaunchProfile`, `rules_allow`, `Env`, `Features`.
- Produces:
  - `struct LaunchContext { pub java: PathBuf, pub game_dir: PathBuf, pub assets_root: PathBuf, pub natives_dir: PathBuf, pub heap_mb: u32, pub extra_jvm: Vec<String>, pub player_name: String, pub uuid: String /* ohne Bindestriche */, pub access_token: String, pub xuid: String, pub user_type: String /* "msa" | "legacy" */, pub config_dir: PathBuf, pub instance_name: String }`.
  - `fn build_command(p: &LaunchProfile, c: &LaunchContext, env: &Env) -> Vec<String>` – Reihenfolge: `-Xmx{heap}M`, `-Xms512M`, Vanilla-JVM-Args (Regeln ausgewertet, Platzhalter ersetzt), Fabric-JVM-Args, Logging-Arg (`${path}` → Config-Pfad), `-Dfrogclient.configDir=…`, `-Dfrogclient.instanceName=…`, `extra_jvm`, Main-Class, Game-Args. Platzhalter: `auth_player_name, version_name, game_directory, assets_root, assets_index_name, auth_uuid, auth_access_token, clientid (Azure-ID), auth_xuid, user_type, version_type, natives_directory, launcher_name ("FrogClient"), launcher_version, classpath` (Trenner aus `platform::CLASSPATH_SEP`). Unbekannte Platzhalter bleiben unverändert und werden geloggt.
  - `fn redact(cmd: &[String]) -> Vec<String>` – ersetzt den Wert nach `--accessToken` durch `***` (für Logs).

- [ ] **Step 1: Failing Tests** (mit Fixture-Profil):

```rust
#[test] fn heap_and_main_class_present()          // "-Xmx3072M" vor "net.fabricmc...KnotClient"
#[test] fn placeholders_replaced()                // keine "${auth_" / "${classpath}" mehr; "--username" gefolgt von player_name
#[test] fn classpath_uses_platform_separator()    // enthält ";" auf Windows
#[test] fn frogclient_properties_set()            // "-Dfrogclient.configDir=<..>", "-Dfrogclient.instanceName=<name>"
#[test] fn no_demo_flag()
#[test] fn redact_hides_token()                   // Token-Wert taucht im Ergebnis nicht auf
```

- [ ] **Step 2:** FAIL → implementieren → PASS. **Step 3: Commit** `feat(launcher): launch argument builder`

---

### Task 7: Accounts, Offline-UUID, TokenStore, Offline-Sperre

**Files:**
- Create: `launcher/src/accounts.rs`, `launcher/src/auth/{mod,offline}.rs`, `launcher/src/settings.rs`

**Interfaces:**
- Produces:
  - `enum AccountKind { Microsoft { owns_game: bool }, Offline }`; `struct Account { pub id: String /* UUID ohne Bindestriche */, pub name: String, pub kind: AccountKind }` (serde).
  - `struct LauncherFile { accounts: Vec<Account>, selected: Vec<String>, settings: LauncherSettings }`; `LauncherSettings { heap_mb: u32 /* 3072 */, extra_jvm_args: String, minimize_on_launch: bool /* false */, loader_version: String /* "0.19.5" */ }`.
  - `fn load_launcher_file(path) -> LauncherFile` (fehlend/kaputt → Default, kaputt zusätzlich `.bak`), `fn save_launcher_file(path, &LauncherFile) -> io::Result<()>` (atomar, eindeutige Temp-Datei wie Mod-Config).
  - `fn offline_uuid(name: &str) -> String` – Java `UUID.nameUUIDFromBytes("OfflinePlayer:"+name)` (MD5, Version 3, Variante IETF), Ausgabe ohne Bindestriche.
  - `fn validate_offline_name(name: &str) -> Result<(), &'static str>` – 3–16 Zeichen `[A-Za-z0-9_]`.
  - `fn can_add_offline(accounts: &[Account]) -> bool` – `cfg!(feature = "dev") || ∃ Microsoft { owns_game: true }`.
  - `trait TokenStore: Send + Sync { fn get(&self, id: &str) -> Option<String>; fn set(&self, id: &str, token: &str) -> Result<(), String>; fn delete(&self, id: &str); }` mit `KeyringStore` (Service `FrogClient`) und `MemoryStore` (Tests).

- [ ] **Step 1: Failing Tests:**

```rust
#[test] fn offline_uuid_matches_java()        // offline_uuid("Notch") == "b50ad385829d3141a2167e7d7539ba7f"
#[test] fn offline_name_validation()          // "ab" Err, "Frog_Main" Ok, "äöü" Err, 17 Zeichen Err
#[test] fn offline_gated_without_owned_msa()  // #[cfg(not(feature="dev"))]: [] → false; [Microsoft{owns_game:false}] → false; [..true] → true
#[test] fn launcher_file_roundtrip(tmp)
#[test] fn corrupt_launcher_file_defaults_and_backup(tmp)
#[test] fn defaults()                         // heap 3072, loader "0.19.5"
```

- [ ] **Step 2:** FAIL → implementieren → PASS. **Step 3: Commit** `feat(launcher): accounts, offline UUID, token store`

---

### Task 8: Microsoft-Login-Kette

**Files:**
- Create: `launcher/src/auth/{msa,xbox}.rs`

**Interfaces:**
- Consumes: `Http`, `TokenStore`, `Account`.
- Produces:
  - `struct AuthEndpoints { pub authorize, pub token, pub devicecode, pub xbl, pub xsts, pub mc_login, pub entitlements, pub profile }` mit `Default` = `https://login.microsoftonline.com/consumers/oauth2/v2.0/{authorize,token,devicecode}`, `https://user.auth.xboxlive.com/user/authenticate`, `https://xsts.auth.xboxlive.com/xsts/authorize`, `https://api.minecraftservices.com/authentication/login_with_xbox`, `…/entitlements/mcstore`, `…/minecraft/profile`.
  - `fn pkce_pair() -> (String /*verifier*/, String /*S256 challenge, base64url ohne Padding*/)`; `fn authorize_url(ep, client_id, redirect, challenge, state) -> String` (`prompt=select_account`).
  - `async fn login_interactive(http, ep) -> Result<MsTokens, AuthError>` – lokalen `TcpListener` auf `127.0.0.1:0`, Browser via `open::that`, wartet max. 5 min auf `GET /?code=…&state=…`, prüft `state`, antwortet mit kleiner HTML-Seite („Anmeldung abgeschlossen, du kannst dieses Fenster schließen“), tauscht Code gegen Tokens.
  - `async fn login_device_code(http, ep, on_code: impl Fn(String /*user_code*/, String /*verification_uri*/)) -> Result<MsTokens, AuthError>` – Polling gemäß `interval`, behandelt `authorization_pending`/`slow_down`.
  - `async fn refresh(http, ep, refresh_token) -> Result<MsTokens, AuthError>`.
  - `async fn minecraft_login(http, ep, ms_access_token) -> Result<McSession, AuthError>` – XBL (`RpsTicket = "d=<token>"`) → XSTS (`RelyingParty "rp://api.minecraftservices.com/"`) → `login_with_xbox` (`identityToken "XBL3.0 x=<uhs>;<xsts>"`) → Entitlements → Profil. `McSession { access_token, uuid, name, owns_game: bool, xuid: String /* "0" wenn nicht verfügbar */ }`.
  - `enum AuthError { Cancelled, Timeout, NoXboxAccount /* XErr 2148916233 */, ChildAccount /* 2148916238 */, AppNotApproved /* 403 von login_with_xbox */, NoProfile /* 404 Profil */, ReloginRequired /* invalid_grant */, Network(String), Other(String) }` mit `fn user_message(&self) -> &'static str` (deutsch; `AppNotApproved` → „Die FrogClient-App wurde von Mojang noch nicht freigegeben. Bis dahin funktionieren nur Offline-Accounts.“).

- [ ] **Step 1: Failing Tests** (wiremock für alle Endpunkte):

```rust
#[test] fn pkce_challenge_is_s256_of_verifier()
#[test] fn authorize_url_contains_required_params()   // client_id, response_type=code, scope enthält XboxLive.signin und offline_access, code_challenge_method=S256
#[tokio::test] async fn full_chain_success()          // McSession{name:"FrogMain", owns_game:true}
#[tokio::test] async fn mc_login_403_is_app_not_approved()
#[tokio::test] async fn xsts_no_xbox_account()        // 401 mit XErr 2148916233 → NoXboxAccount
#[tokio::test] async fn empty_entitlements_owns_game_false()
#[tokio::test] async fn refresh_invalid_grant_requires_relogin()
#[tokio::test] async fn device_code_polls_until_token() // 2× authorization_pending, dann Token
#[tokio::test] async fn loopback_rejects_wrong_state()  // Request an Listener mit falschem state → Err(Other)
```

- [ ] **Step 2:** FAIL → implementieren → PASS. **Step 3: Commit** `feat(launcher): Microsoft/Xbox/Minecraft authentication`

---

### Task 9: Instanz vorbereiten (geteilte Daten, Mods, Serverliste)

**Files:**
- Create: `launcher/src/platform.rs`, `launcher/src/launch/instance.rs`

**Interfaces:**
- Consumes: `AppPaths`, `ModStore`-Liste aktiver JAR-Pfade (Task 11 liefert `ModStore::enabled_jars(&self, paths) -> Vec<PathBuf>`; hier nur Parameter `&[PathBuf]`).
- Produces:
  - `platform`: `const CLASSPATH_SEP: &str`; `fn link_dir(target: &Path, link: &Path) -> io::Result<()>` (Windows: Junction, sonst Symlink; existiert `link` als echter Ordner mit Inhalt → Inhalt in `target` verschieben, dann verlinken); `fn link_or_copy_file(src, dst) -> io::Result<()>` (Hardlink, Fallback Kopie).
  - `struct PreparedInstance { pub game_dir: PathBuf, pub natives_dir: PathBuf, servers_snapshot: Option<SystemTime> }`.
  - `fn prepare_instance(paths: &AppPaths, account_id: &str, mod_jars: &[PathBuf], frog_jar: Option<&Path>, options_template: Option<&Path>) -> io::Result<PreparedInstance>`:
    - `instances/<id>/` anlegen; `resourcepacks`, `shaderpacks`, `screenshots`, `config` → `link_dir` auf `shared-gamedata/…`.
    - `mods/` leeren und alle `mod_jars` + `frog_jar` hineinlinken.
    - `servers.dat`: `shared-gamedata/servers.dat` → Instanz kopieren (Kopie, weil Minecraft die Datei per Ersetzen schreibt und Hardlinks bricht); mtime merken.
    - `options.txt` nur wenn fehlend: Kopie von `options_template`.
  - `fn finish_instance(paths: &AppPaths, prepared: &PreparedInstance) -> io::Result<()>` – nach Prozessende: wenn Instanz-`servers.dat` neuer als Snapshot → atomar nach `shared-gamedata/servers.dat` kopieren (Temp + rename).
  - `fn newest_options(paths: &AppPaths) -> Option<PathBuf>` – zuletzt geänderte `instances/*/options.txt`.

- [ ] **Step 1: Failing Tests** (tempdir):

```rust
#[test] fn shared_dirs_are_linked()          // Datei in shared-gamedata/resourcepacks sichtbar in instances/a/resourcepacks
#[test] fn mods_folder_mirrors_enabled_jars()// alte JAR entfernt, neue vorhanden, frog_jar vorhanden
#[test] fn existing_real_folder_is_migrated() // instances/a/screenshots mit Datei → danach in shared-gamedata/screenshots
#[test] fn servers_copied_in_and_back_when_changed()
#[test] fn servers_not_written_back_when_unchanged() // andere Instanz hat inzwischen geschrieben → bleibt erhalten
#[test] fn options_only_copied_when_missing()
#[test] fn newest_options_picks_latest_mtime()
```

- [ ] **Step 2:** FAIL → implementieren → PASS. **Step 3: Commit** `feat(launcher): instance preparation with shared game data`

---

### Task 10: Prozessverwaltung + RAM-Prüfung

**Files:**
- Create: `launcher/src/launch/process.rs`

**Interfaces:**
- Consumes: `build_command`, `PreparedInstance`, `finish_instance`.
- Produces:
  - `enum InstanceEvent { Started { account_id: String }, Exited { account_id: String, code: Option<i32>, log: PathBuf } }`.
  - `struct ProcessManager` – `fn new(events: tokio::sync::mpsc::UnboundedSender<InstanceEvent>) -> Self`; `async fn spawn(&self, account_id: String, cmd: Vec<String>, cwd: PathBuf, on_exit: impl FnOnce() + Send + 'static) -> io::Result<()>` (Windows: `CREATE_NO_WINDOW`; stdout/stderr verwerfen – Minecraft schreibt eigene Logs); `fn is_running(&self, id: &str) -> bool`; `fn kill(&self, id: &str)`; `fn running(&self) -> Vec<String>`.
  - `fn ram_warning(heap_mb_each: u32, count: usize, available_mb: u64) -> Option<String>` → `Some("Für N Instanzen werden X GB benötigt, frei sind nur Y GB.")` wenn `count*heap + count*512 > available`.
  - `fn available_memory_mb() -> u64` (sysinfo).

- [ ] **Step 1: Failing Tests:**

```rust
#[test] fn ram_ok()        // ram_warning(3072, 2, 16000) == None
#[test] fn ram_too_low()   // ram_warning(3072, 4, 10000) enthält "4 Instanzen"
#[cfg(windows)] #[tokio::test] async fn exit_code_reported() // cmd: ["cmd","/C","exit","3"] → Exited{code:Some(3)}, is_running false danach, on_exit aufgerufen
#[cfg(windows)] #[tokio::test] async fn kill_stops_process()   // ["cmd","/C","ping","-n","30","127.0.0.1"] → kill → Exited
#[tokio::test] async fn two_processes_independent()            // einer endet, anderer läuft weiter
```

- [ ] **Step 2:** FAIL → implementieren → PASS. **Step 3: Commit** `feat(launcher): process manager and RAM check`

---

### Task 11: Modrinth + ModStore

**Files:**
- Create: `launcher/src/modrinth.rs`, `launcher/src/mods.rs`, Fixtures `tests/fixtures/modrinth/*.json`

**Interfaces:**
- Consumes: `Http`, `AppPaths`.
- Produces:
  - `const MODRINTH_API: &str = "https://api.modrinth.com/v2"`; `struct Modrinth { http, base }`:
    - `async fn search(&self, query: &str, mc: &str, offset: u32) -> Result<Vec<SearchHit { project_id, slug, title, description, downloads, icon_url }>, HttpError>` (Facetten `[["versions:<mc>"],["categories:fabric"],["project_type:mod"]]`, `limit=20`).
    - `async fn best_version(&self, project: &str, mc: &str) -> Result<Option<ModVersion>, HttpError>` – `/project/{p}/version?loaders=["fabric"]&game_versions=["<mc>"]`; bevorzugt `release` vor `beta` vor `alpha`, dann neuestes `date_published`; `ModVersion { id, project_id, version_number, files: Vec<{ url, filename, sha1, primary }>, dependencies: Vec<{ project_id: Option<String>, version_id: Option<String>, dependency_type }> }`.
  - `struct InstalledMod { project_id, slug, title, version_id, version_number, filename, sha1, enabled: bool, required: bool /* fabric-api, nicht abwählbar */ }`; `struct ModStore { path: PathBuf, mods: Vec<InstalledMod> }`:
    - `fn load(paths) -> ModStore`, `fn save(&self) -> io::Result<()>`.
    - `async fn install(&mut self, mr: &Modrinth, http: &Http, paths, project: &str, mc: &str) -> Result<Vec<String> /* installierte Titel */, ModError>` – löst `required`-Abhängigkeiten rekursiv auf (Zyklen-Schutz), lädt erst **alle** Dateien, trägt erst danach in `mods.json` ein (alles oder nichts); bereits installierte Projekte werden übersprungen.
    - `fn set_enabled(&mut self, project_id, bool) -> Result<(), ModError>` (required → Err), `fn remove(&mut self, project_id, paths) -> Result<(), ModError>` (required → Err; Datei löschen).
    - `async fn check_updates(&self, mr, mc) -> Result<Vec<(String /*project*/, ModVersion)>, ModError>`; `async fn update(&mut self, …, project)`.
    - `async fn ensure_defaults(&mut self, …, mc, choices: &[&str])` – Erstinstallation: `fabric-api` immer, plus gewählte aus `["sodium","lithium","ferrite-core"]`.
    - `fn enabled_jars(&self, paths) -> Vec<PathBuf>`.
    - `async fn missing_for(&self, mr, mc_other: &str) -> Result<Vec<String>, ModError>` (Versionswechsel-Vorbereitung, Spec).
  - `enum ModError { NoCompatibleVersion(String), MissingDependency(String), Required(String), Http(HttpError), Io(String) }` mit deutscher `user_message`.

- [ ] **Step 1: Failing Tests** (wiremock + Fixtures):

```rust
#[tokio::test] async fn search_sends_26_3_fabric_facets()     // Query-Parameter-Matcher
#[tokio::test] async fn best_version_prefers_release()
#[tokio::test] async fn install_pulls_required_dependency()   // sodium-like mit required dep "fabric-api" → beide in mods.json
#[tokio::test] async fn install_is_all_or_nothing()           // Dep-Datei 404 → Err, mods.json unverändert, keine neue Datei
#[tokio::test] async fn no_compatible_version_error()
#[test] fn required_mod_cannot_be_disabled_or_removed()
#[tokio::test] async fn check_updates_finds_newer_version()
#[tokio::test] async fn defaults_install_fabric_api_always()
```

- [ ] **Step 2:** FAIL → implementieren → PASS. **Step 3: Commit** `feat(launcher): Modrinth search and mod store`

---

### Task 12: Update-Hinweis + FrogClient-Mod-Bezug

**Files:**
- Create: `launcher/src/update.rs`

**Interfaces:**
- Produces:
  - `async fn latest_release(http, api_base: &str /* "https://api.github.com" */, repo: &str /* "Klausiiiii/FrogClient" */) -> Result<Option<Release { tag: String, html_url: String, assets: Vec<{ name, browser_download_url }> }>, HttpError>` (404 = keine Releases → `None`).
  - `fn is_newer(tag: &str, current: &str) -> bool` – `v`-Präfix ignorieren, numerischer Vergleich `major.minor.patch`.
  - `async fn frog_mod_jar(http, paths, release: Option<&Release>) -> Option<PathBuf>` – Reihenfolge: Env `FROGCLIENT_MOD_JAR` (Datei) → Release-Asset `frogclient-*.jar` (Cache `shared/frogclient/<tag>.jar`) → zuletzt gecachte JAR → `None` (Launcher startet trotzdem, UI zeigt Hinweis „FrogClient-Mod nicht gefunden“).

- [ ] **Step 1: Failing Tests:**

```rust
#[test] fn version_compare()  // is_newer("v0.2.0","0.1.9"), !is_newer("0.1.0","0.1.0"), is_newer("1.0.0","0.9.12")
#[tokio::test] async fn no_releases_is_none()
#[tokio::test] async fn env_jar_wins()
#[tokio::test] async fn release_asset_downloaded_and_cached()
```

- [ ] **Step 2:** FAIL → implementieren → PASS. **Step 3: Commit** `feat(launcher): release check and FrogClient mod source`

---

### Task 13: Mod – Fenstertitel pro Instanz

**Files:**
- Create: `mod/src/main/java/dev/frogclient/WindowTitle.java`, `mod/src/main/java/dev/frogclient/mixin/MinecraftTitleMixin.java`
- Modify: `mod/src/main/resources/frogclient.mixins.json`
- Test: `mod/src/test/java/dev/frogclient/WindowTitleTest.java`

**Interfaces:**
- Produces: `final class WindowTitle` – `static String format(String vanillaTitle, String instanceName)` → `instanceName` leer/null ⇒ `vanillaTitle`, sonst `"FrogClient – " + instanceName`. Mixin auf die Methode, die in 26.3 den Fenstertitel baut (per `mcsrc`/genSources prüfen, 1.21.x: `Minecraft#createTitle`), `@ModifyReturnValue` mit `System.getProperty("frogclient.instanceName")`.

- [ ] **Step 1: Failing Test:** `format("Minecraft* 26.3", "FrogMain") == "FrogClient – FrogMain"`, `format("X", null) == "X"`, `format("X", " ") == "X"`.
- [ ] **Step 2:** FAIL → implementieren → PASS (`./gradlew test`), Mixin eintragen, `./gradlew build`.
- [ ] **Step 3: Commit** `feat(mod): per-instance window title`

---

### Task 14: AppController + Slint-Oberfläche

**Files:**
- Create: `launcher/src/controller.rs`
- Modify: `launcher/ui/app.slint`, `launcher/src/main.rs`

**Interfaces:**
- Consumes: alle vorherigen Module.
- Produces:
  - `struct AccountRow { id, name, offline: bool, selected: bool, running: bool, error: Option<String> }`; `struct UiState { accounts: Vec<AccountRow>, status: String, progress: Option<f32>, can_add_offline: bool, update_notice: Option<String>, mods: Vec<ModRow>, search: Vec<SearchRow>, settings: LauncherSettings }`.
  - `fn start_button_label(rows: &[AccountRow]) -> Option<String>` – Anzahl = angehakt ∧ nicht laufend; `0` ⇒ `None` (Knopf deaktiviert), `1` ⇒ „▶ 1 Instanz starten“, sonst „▶ N Instanzen starten“.
  - `struct AppController` – hält `LauncherFile`, `ModStore`, `ProcessManager`, `TokenStore`, Http-Clients; Aktionen (alle async, Ergebnis per `slint::invoke_from_event_loop` an UI): `toggle_selected(id)`, `add_microsoft()` (interaktiv, Fallback Device-Code-Dialog), `add_offline(name)`, `remove_account(id)`, `launch_selected()`, `stop(id)`, `search_mods(q)`, `install_mod(project)`, `toggle_mod`, `remove_mod`, `check_mod_updates`, `update_mod`, `save_settings`.
  - `launch_selected` pro Account: Token auffrischen (Microsoft) → bei Fehler Zeile mit `user_message` markieren und **mit den übrigen Accounts weitermachen** → einmalig `install_version` + `fabric` + `install_java` + `ensure_defaults` (erster Start; Auswahl-Dialog für Sodium/Lithium/FerriteCore, alle vorausgewählt) → `ram_warning` (Dialog „Trotzdem starten“) → `prepare_instance` → `build_command` → `spawn`; `on_exit` ruft `finish_instance`; `Exited` mit Code ≠ 0 setzt in der Zeile „Abgestürzt – Log: <Pfad>“. Statuszeile zeigt Fortschritt („Lade Bibliotheken 34/120“).
  - Slint (`app.slint`, Layout A aus dem Mockup): Tab **Spielen** – Kopf „🐸 FrogClient · 26.3 Fabric“, Account-Liste (Checkbox, farbiges Initial-Quadrat als Avatar [Builder-Wahl laut Spec, keine Drittanbieter-Avatare], Name, Badge „Offline“/„läuft“, Stopp-Knopf bei laufender Instanz, Fehlerzeile rot), „+ Account hinzufügen“ (Dialog: „Mit Microsoft anmelden“ / „Offline-Account“ – letzterer ausgegraut mit Erklärung, wenn `!can_add_offline`), unten großer grüner Start-Knopf mit `start_button_label`, Statuszeile + Fortschrittsbalken. Tab **Mods** – installierte Mods (Schalter, Entfernen, „Update verfügbar“), Suchfeld + Ergebnisliste mit „Installieren“, Knopf „Nach Updates suchen“. Tab **Einstellungen** – Heap (MB, SpinBox 1024–16384, Schritt 512), zusätzliche JVM-Argumente, „Nach Start minimieren“, Versionsanzeige + Update-Hinweis mit Link.
  - `main.rs`: Fenster erzeugen und `show()` **vor** Runtime-Start, Dateien laden, dann tokio-Runtime in eigenem Thread starten; Update-Prüfung erst danach im Hintergrund. In Debug-Builds Zeit bis erstem Frame loggen (`frame_ms=`).

- [ ] **Step 1: Failing Tests** (Controller-Logik ohne UI):

```rust
#[test] fn label_counts_selected_not_running() // 3 angehakt, 1 läuft → "▶ 2 Instanzen starten"
#[test] fn label_singular()                    // "▶ 1 Instanz starten"
#[test] fn label_none_disables()               // None
#[tokio::test] async fn failing_account_does_not_block_others() // Fake-Auth: A AppNotApproved, B Offline → B wird gestartet (Fake-Spawner), A.error == AppNotApproved-Text
```

- [ ] **Step 2:** FAIL → Controller implementieren (Spawner/Installer hinter Traits für den Test) → PASS.
- [ ] **Step 3: UI bauen** und verdrahten. Run: `cargo run --features dev` → alle drei Tabs bedienbar.
- [ ] **Step 4: Commit** `feat(launcher): controller and Slint UI`

---

### Task 15: Abnahme gegen Spec

**Files:**
- Modify: `README.md` (Abschnitt „Building the launcher“)

- [ ] **Step 1:** `cargo test` (alle grün) und `cargo build --release`.
- [ ] **Step 2: Kriterium 1.** Release-Binary 5× starten, `frame_ms` (Debug) bzw. Stoppuhr-Messung per Skript (Zeit bis Fenster-Handle existiert, PowerShell `Get-Process … MainWindowHandle`) → Median < 500 ms.
- [ ] **Step 3: Kriterium 2+3.** Mit leerem `FROGCLIENT_HOME`, `--features dev`, Env `FROGCLIENT_MOD_JAR=mod/build/libs/frogclient-0.1.0.jar`: zwei Offline-Accounts anlegen, beide anhaken, starten → beide Minecraft-Prozesse laufen parallel, `instances/<a>/logs/latest.log` und `<b>/…` enthalten „FrogClient 0.1.0 geladen“ und „Sodium“, keine Datei-Fehler; Fenstertitel „FrogClient – <Name>“.
- [ ] **Step 4: Kriterium 5.** Im Mods-Tab „Mod Menu“ (oder andere 26.3-Mod mit Abhängigkeit) suchen und installieren → erscheint in `mods.json`, beim nächsten Start in beiden Instanzen geladen.
- [ ] **Step 5: Kriterium 4** (nur nach Mojang-Freigabe, sonst als offen dokumentieren): Microsoft-Login, Neustart Launcher, Start ohne erneuten Login.
- [ ] **Step 6: Commit** `docs: launcher build and usage`
