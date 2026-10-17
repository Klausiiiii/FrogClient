# FrogClient – Design

Stand: 2026-10-17
Status: Entwurf zur Freigabe

## 1. Absicht und Warum

FrogClient ist ein eigener, schlanker Minecraft-Java-Client auf Fabric, bestehend aus einem **nativen Launcher** und einer **Ingame-Mod**.

Warum: Bestehende Clients (Lunar, Feather, Badlion) sind dem Nutzer zu überladen – viele Features, die man nie braucht. FrogClient soll nur enthalten, was echte, eigene Probleme löst, und die Freiheit geben, gezielt eigene Features nachzurüsten. Ein konkretes Problem, das bestehende Launcher schlecht lösen: **mehrere Accounts gleichzeitig starten**.

Nutzerkreis vorerst: der Nutzer selbst und ein paar Freunde. Keine öffentliche Verteilung.

## 2. Ziele

1. **Launcher** (nativ, schnell startend, keine Web-Oberfläche):
   - Eigener Microsoft-Login (eigene Azure-App).
   - Mehrere Accounts verwalten; **mehrere Accounts gleichzeitig starten** (je eine eigene Minecraft-Instanz).
   - Installiert Minecraft 26.3, Fabric Loader und die benötigte Java-Runtime selbst – der Nutzer installiert nichts von Hand.
   - Zusätzliche Fabric-Mods **direkt über Modrinth** suchen und installieren, inkl. Abhängigkeiten, Update-Prüfung.
   - Installiert standardmäßig Performance-Mods mit (Sodium, Lithium, FerriteCore), abwählbar.
2. **Ingame-Mod** (Fabric, MC 26.3):
   - Client-GUI per Keybind, in zwei umschaltbaren Layouts.
   - Erste Features: **Zoom**, **FPS-Anzeige**, **Fullbright**.
   - Architektur so, dass neue Features mit minimalem Aufwand hinzukommen.
3. **Performance:** Eigener Code kostet praktisch nichts; FPS-Gewinne kommen von bewährten Mods (Sodium & Co.), nicht von Eigenbau.

## 3. Nicht-Ziele (bewusst nicht in v1)

- Eigene Rendering-/Logik-Optimierungen, die Sodium/Lithium nachbauen.
- Kosmetik, Capes, Emotes, eigener Backend-Server, Freundeslisten, Telemetrie.
- Mehrere Minecraft-Versionen gleichzeitig (v1 = nur 26.3; Architektur darf es später erlauben).
- Getrennte Mod-Profile pro Instanz.
- Linux-/macOS-Builds (Code muss aber portabel bleiben, siehe Constraints).
- Installer, Auto-Installation von Updates, Code-Signing.
- Manuelles Hineinziehen von .jar-Dateien als primärer Weg (Modrinth ist der Weg).

## 4. Anti-Ziele (wäre ein Fehlschlag, auch wenn es technisch funktioniert)

- Der Client wird selbst „überladen“ – Features ohne konkretes Problem dahinter.
- Langsamer Launcher-Start oder spürbarer FPS-Verlust durch FrogClient-Code.
- Login-Tokens im Klartext auf der Platte.
- Features, die Accounts bannen lassen (X-Ray, Killaura, Reach usw.). Zoom/Fullbright/FPS sind ok.
- Zwei GUI-Layouts, die getrennt gepflegt werden müssen (siehe 6.3).
- Verstoß gegen Minecraft-Usage-Guidelines (kein offizielles Branding; Disclaimer bleibt).

## 5. Constraints

- **Minecraft 26.3** (Release 2026-09-15), **Java 25** erforderlich.
- **Fabric** Loader/API: laut Drittquellen Loader 0.19.5, Fabric API 0.161.0+26.3 – beim Bauen gegen `meta.fabricmc.net` verifizieren.
- **Azure-App** registriert:
  - Client-ID: `2eb1f5a4-f452-4b42-acd4-880340c33f4c` (kein Geheimnis, darf im Code stehen)
  - Kontotyp: nur persönliche Microsoft-Konten (Authority `consumers`)
  - Plattform: öffentlicher Client/nativ, Redirect-URI `http://localhost`; öffentliche Clientflows erlaubt (Device Code möglich)
  - Kein Client-Secret.
- **Mojang-Freigabe** der App-ID ist beantragt bzw. wird beantragt (aka.ms/mce-reviewappid) und kann Wochen dauern. Bis dahin liefert `api.minecraftservices.com` für diese App-ID Fehler → **Offline-Accounts** müssen für Entwicklung und Tests voll funktionieren.
- **Windows zuerst** (Windows 11). Linux/macOS später: keine Windows-only-Abhängigkeiten im Kern; plattformspezifisches hinter Abstraktionen (Pfade, Credential Store, Java-Runtime-Auswahl).
- GitHub-Repo: https://github.com/Klausiiiii/FrogClient (öffentlich). README enthält Pflicht-Disclaimer „NOT AN OFFICIAL MINECRAFT PRODUCT…“.
- RAM: realistisch 3–4 gleichzeitige Instanzen bei 16 GB.

## 6. Entscheidungen

Markierung: **[Nutzer]** = vom Nutzer entschieden, **[Claude]** = Entscheidung von Claude (Nutzer hat Technik delegiert: „du machst alles“).

### 6.1 Repository-Aufbau [Claude]

Monorepo:

```
/launcher   Rust-Projekt (Cargo), Launcher
/mod        Gradle/Fabric-Loom-Projekt, Ingame-Mod (Java 25)
/docs       Specs, Pläne
```

### 6.2 Launcher

**Technik [Claude]:** Rust, UI mit **Slint** (native Darstellung, keine Webview). Eine einzelne .exe. Async-Laufzeit (tokio) für Downloads/HTTP. Ziel: Fenster sichtbar in < 0,5 s auf einem normalen Rechner.

**Oberfläche [Nutzer: Layout A]:**
- Linke schmale Navigationsleiste mit drei Tabs: **Spielen**, **Mods**, **Einstellungen**.
- Tab „Spielen“: Kopfzeile „FrogClient · 26.3 Fabric“; Liste aller Accounts, jede Zeile mit **Checkbox**, Avatar (Skin-Kopf), Name, Status-Badge („läuft“ wenn Instanz aktiv); Zeile „+ Account hinzufügen“; unten großer Knopf **„▶ N Instanzen starten“** (N = Anzahl angehakter, nicht laufender Accounts). Laufende Instanzen sind einzeln beendbar (Kontextaktion an der Zeile) [Claude].
- Tab „Mods“: Liste installierter Mods (an/aus, entfernen, Update verfügbar), Modrinth-Suche.
- Tab „Einstellungen“: RAM pro Instanz, Java-Argumente (erweitert), Launcher-Verhalten (z. B. nach Start minimieren).

**Accounts & Login:**
- Microsoft-Login [Nutzer]: Authorization Code Flow mit PKCE über System-Browser und Loopback-Redirect `http://localhost:<zufälliger Port>` [Claude]; Device-Code-Flow als Fallback [Claude]. Kette: MSA → Xbox Live (XBL) → XSTS → `login_with_xbox` → Entitlement-Prüfung → Profil.
- Refresh-Tokens im OS-Credential-Store (Windows Credential Manager, z. B. via `keyring`-Crate) [Claude]. Minecraft-Access-Token nur im Speicher bzw. kurzlebig gecacht; vor jedem Start bei Bedarf erneuert.
- **Offline-Accounts** (frei wählbarer Name, Offline-UUID), im UI als „Offline“ markiert [Nutzer]:
  - In Release-Builds nur anlegbar, wenn mindestens ein Microsoft-Account mit gültigem Minecraft-Besitz (Entitlement) eingeloggt ist (wie Prism). Schützt auch den Mojang-App-Review.
  - In Entwickler-Builds (Cargo-Feature `dev`) ohne diese Bedingung, damit vor der Mojang-Freigabe getestet werden kann.
  - Bekannte Folge (Nutzer informiert): Bis zur Mojang-Freigabe können Freunde FrogClient nicht nutzen.
- Fehlerfall „App-ID noch nicht freigegeben“ wird klar als solcher gemeldet, nicht als generischer Login-Fehler [Claude].

**Installation von Spiel & Runtime [Claude]:**
- Minecraft: Mojang Version-Manifest (piston-meta) → Version-JSON 26.3 → Client-JAR, Libraries, Natives, Asset-Index + Assets. SHA1-Prüfung aller Downloads, parallele Downloads mit Limit, fortsetzbar.
- Fabric: Profil-JSON von `meta.fabricmc.net` für 26.3 + gewählte Loader-Version, mit Vanilla-JSON zusammenführen.
- Java: Runtime-Komponente aus dem Version-JSON (`javaVersion.component`) über Mojangs Java-Runtime-Manifest herunterladen. Kein System-Java nötig.

**Verzeichnisaufbau [Claude]:** Datenordner `%APPDATA%\FrogClient\` (plattformabhängig über Abstraktion):

```
shared/        versions/, libraries/, assets/, runtimes/   (einmal für alle)
mods/          die gemeinsame Mod-Liste (eine Quelle der Wahrheit) + mods.json (Metadaten)
config/        frogclient.json (gemeinsame FrogClient-Einstellungen)
shared-gamedata/  servers.dat, resourcepacks/, shaderpacks/, screenshots/, config/ (geteilt)
instances/<account-id>/   eigenes Game-Directory pro Account
launcher.json  Launcher-Einstellungen, Account-Liste (ohne Tokens)
```

- Jeder Account bekommt ein **eigenes Game-Directory**, weil gleichzeitig laufende Instanzen sich sonst `options.txt`, Logs, `saves/` usw. teilen und gegenseitig zerschießen.
- **Mods sind für alle Instanzen gleich** [Nutzer bestätigt]: beim Start wird `instances/<id>/mods/` aus `mods/` synchronisiert (Hardlinks, Fallback Kopie).
- **FrogClient-Einstellungen gemeinsam** [Nutzer bestätigt]: Mod liest/schreibt `config/frogclient.json`; Pfad per JVM-Property `-Dfrogclient.configDir=…`. Gleichzeitiges Schreiben: letzte Änderung gewinnt, atomar schreiben (temp + rename).
- `options.txt` (Minecraft-Einstellungen) pro Instanz; neue Instanzen bekommen eine Kopie der zuletzt geänderten [Claude].
- **Ebenfalls über alle Accounts geteilt** [Nutzer]: Serverliste (`servers.dat`), `resourcepacks/`, `shaderpacks/`, `screenshots/` und Konfigurationen anderer Mods (`config/`, z. B. Sodium). Sie liegen zentral unter `shared-gamedata/` im Datenordner; jede Instanz verweist darauf (Ordner: Directory Junction/Symlink; `servers.dat`: Hardlink oder Rück-Sync nach Instanz-Ende – Builder wählt robustes Verfahren, da Minecraft die Datei per Ersetzen schreibt). Bei gleichzeitigen Änderungen gewinnt die letzte.
- **Pro Instanz getrennt:** `options.txt`, `logs/`, `crash-reports/`, `saves/`.

**Mehrfachstart:**
- Jede Instanz = eigener Java-Prozess mit eigenem Token, Game-Dir, Fenster-Titel „FrogClient – <Name>“ [Claude].
- Launcher überwacht Prozesse (Status „läuft“, Absturz → Hinweis mit Pfad zum Log).
- RAM-Warnung, wenn Summe der Heap-Größen der zu startenden Instanzen den freien RAM übersteigt. Default-Heap 3 GB [Claude].

**Mods über Modrinth [Nutzer]:**
- Suche über Modrinth API v2 mit Facetten `versions:26.3`, `categories:fabric`, `project_type:mod`.
- Installation: passende Version wählen, **Pflicht-Abhängigkeiten automatisch** mitinstallieren, Hash prüfen.
- „Nach Updates suchen“-Knopf; Update pro Mod oder alle.
- Vorbereitung Versionswechsel: Funktion, die anzeigt, welche installierten Mods für eine andere MC-Version nicht verfügbar sind (UI dafür erst, wenn mehrere Versionen kommen).
- **Standard-Mods bei Ersteinrichtung** [Nutzer]: Fabric API, Sodium, Lithium, FerriteCore – abwählbar. Fabric API ist Pflicht, solange FrogClient sie braucht.
- FrogClient-Mod selbst wird vom Launcher immer installiert (nicht abwählbar), Bezugsquelle: GitHub-Release des Repos [Claude]; in der Entwicklung lokal gebaute JAR.

**Updates [Claude]:** Launcher prüft beim Start GitHub-Releases und zeigt einen Hinweis „Neue Version verfügbar“ mit Link. Kein Auto-Update in v1. FrogClient-Mod-Update zieht der Launcher selbst.

**Signing [Nutzer, nach Aufklärung]:** Kein Code-Signing; Freunde akzeptieren die SmartScreen-Warnung.

### 6.3 Ingame-Mod

**Technik [Claude]:** Fabric Loom, Java 25, Fabric API. Mixins nur wo nötig. Mappings/Namensschema gemäß aktuellem Fabric-Standard für 26.x (beim Setup prüfen).

**Feature-Gerüst [Claude, Anforderung Nutzer: zwei Layouts ohne Doppelpflege]:**
- Jedes Feature ist ein `Module` mit: id, Name, Kategorie, an/aus, optionalem Keybind und einer **deklarativen Liste von Einstellungen** (Bool, Slider mit Min/Max/Schritt, Enum, Farbe, Keybind).
- Beide GUI-Layouts rendern **ausschließlich** aus diesen Deklarationen. Ein neues Feature erscheint automatisch in beiden.
- Persistenz generisch aus denselben Deklarationen nach `frogclient.json`.
- Module ohne Aktivität kosten nichts: keine Tick-/Render-Hooks, wenn aus; keine Allokationen pro Frame.

**Client-GUI [Nutzer]:**
- Öffnen per Keybind, Default **Rechte Umschalt** [Claude, Nutzer bestätigt], änderbar in den Minecraft-Steuerungseinstellungen (Kategorie „FrogClient“).
- **Zwei Layouts mit Umschalter** in der GUI, zuletzt gewähltes bleibt gespeichert:
  - **A – Schwebende Panels:** eine verschiebbare Spalte pro Kategorie; Linksklick = an/aus, Rechtsklick = Einstellungen aufklappen; Panel-Positionen werden gespeichert.
  - **B – Zentrales Fenster:** Kategorien links, Feature-Kacheln mit Schalter rechts, Klick auf Kachel öffnet Einstellungen.
- Optik: dunkel, halbtransparent, Akzentfarbe Froschgrün (#5fd068), wie in den Mockups.
- Kategorien v1: Render, HUD, Einstellungen.

**Features v1** [Nutzer: welche; Details Claude, Nutzer bestätigt]:
- **Zoom:** Taste halten (Default **C**) → FOV-Zoom; Mausrad ändert Stärke während des Haltens; Loslassen → normal. Maus-Sensitivität wird beim Zoomen proportional reduziert, optional „Smooth Camera“. Einstellungen: Standard-Stärke, Mausrad an/aus, Smooth an/aus.
- **FPS-Anzeige:** HUD-Text, Default oben links. Einstellungen: Größe, Farbe, Hintergrund an/aus.
- **HUD-Editor:** Modus (aus der GUI erreichbar), in dem HUD-Elemente per Drag verschoben werden; Positionen relativ zur Bildschirmgröße speichern.
- **Fullbright:** Schalter; volle Helligkeit **ohne** den Gamma-Wert in `options.txt` zu verändern (z. B. über Lightmap/Night-Vision-Effekt-Ansatz – Builder wählt). Ausschalten stellt sofort den Normalzustand her.

Hinweis an Nutzer (bereits kommuniziert): Zoom/Fullbright sind auf großen Servern üblich, einzelne Server können sie verbieten.

## 7. Erfolgskriterien

1. Launcher-Fenster erscheint in < 0,5 s (Kaltstart nach erstem Lauf, normaler Rechner).
2. Frische Installation: Nutzer legt Offline-Account an, klickt Start → 26.3 mit Fabric, Sodium, Lithium, FerriteCore, FrogClient läuft, ohne manuell etwas zu installieren.
3. Zwei (oder drei) Accounts gleichzeitig angehakt → alle Instanzen starten und laufen parallel ohne Datei-Konflikte.
4. Nach Mojang-Freigabe: Microsoft-Login funktioniert, Token-Erneuerung ohne erneuten Login.
5. Modrinth: eine Mod suchen und installieren, Abhängigkeiten kommen mit, Mod ist in allen Instanzen beim nächsten Start aktiv.
6. Ingame: GUI per Rechte-Umschalt, Layout-Umschalter funktioniert, Zoom/FPS/Fullbright funktionieren wie beschrieben, Einstellungen überleben Neustart.
7. Kein messbarer FPS-Unterschied zwischen „FrogClient installiert, alles aus“ und „ohne FrogClient“.

## 8. Dem Builder überlassen

- Konkrete Crates (HTTP, JSON, Hashing, Prozess-Handling), Projektstruktur innerhalb von `/launcher` und `/mod`.
- Fehlerbehandlung/Retry-Strategie der Downloads, Parallelität.
- Exakte Fullbright-Technik, Zoom-Kurve, GUI-Animationen.
- Avatar-Quelle (Skin-Kopf) für Accounts, Offline-Fallback-Avatar.
- Log-Ansicht im Launcher (nice-to-have).
- Teststrategie im Detail (Unit-Tests für Auth-Kette, Manifest-Parsing, Modrinth-Abhängigkeitsauflösung; manuelle Ingame-Tests).
- Lizenz: offen – Nutzer entscheidet später (bis dahin README ohne Lizenz = alle Rechte vorbehalten).
