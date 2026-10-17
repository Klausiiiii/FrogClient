#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod accounts;
mod auth;
mod fabric;
mod fsutil;
mod http;
mod modrinth;
mod mods;
mod mojang;
mod launch;
mod paths;
mod platform;
mod settings;

slint::include_modules!();

fn main() -> Result<(), slint::PlatformError> {
    let window = AppWindow::new()?;
    window.run()
}
