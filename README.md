# FrogClient

A lightweight, performance-focused client for **Minecraft: Java Edition**, built on [Fabric](https://fabricmc.net/).

FrogClient consists of two parts:

- **Launcher** – a fast, native desktop launcher (no web UI). Sign in with your Microsoft account, manage multiple accounts, and launch several game instances at the same time. Add your own Fabric mods alongside the client.
- **In-game client** – a small Fabric mod with only the features you actually need, configurable through an in-game menu opened with a keybind.

## Goals

- **Lean by design** – no bloat, only features that solve real problems.
- **Fast** – quick launcher start-up and minimal in-game overhead.
- **Multi-account** – run multiple instances with different accounts simultaneously.

## Planned features

- Zoom
- FPS display
- Fullbright

## Status

Early development. Initial target: Minecraft 26.3 with Fabric. Windows first; Linux and macOS support planned.

## Building the mod

Requires Java 25.

```sh
cd mod
./gradlew build              # unit tests + mod jar in mod/build/libs/
./gradlew runClient          # start a dev client with the mod
./gradlew runClientGameTest  # automated in-game check, screenshots in mod/build/run/clientGameTest/screenshots/
```

In game, press **Right Shift** to open the FrogClient menu and hold **C** to zoom (both rebindable in Controls).

## Building the launcher

Requires Rust (stable, MSVC toolchain on Windows).

```sh
cd launcher
cargo test                    # unit tests (fake Mojang/Microsoft/Modrinth servers)
cargo run --release           # start the launcher
cargo run --features dev      # development build: offline accounts without a Microsoft account
```

Useful environment variables (development):

- `FROGCLIENT_HOME` – data directory (default `%APPDATA%\FrogClient`)
- `FROGCLIENT_MOD_JAR` – use a locally built FrogClient mod jar, e.g. `mod/build/libs/frogclient-0.1.0.jar`

## Authentication

The launcher uses the official Microsoft → Xbox Live → XSTS → Minecraft login flow. Credentials are never sent to any third party; tokens are stored locally in the operating system's credential store.

## Disclaimer

NOT AN OFFICIAL MINECRAFT PRODUCT. NOT APPROVED BY OR ASSOCIATED WITH MOJANG OR MICROSOFT.
