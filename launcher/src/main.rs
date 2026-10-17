#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod fabric;
mod http;
mod mojang;
mod launch;
mod paths;
mod platform;

slint::include_modules!();

fn main() -> Result<(), slint::PlatformError> {
    let window = AppWindow::new()?;
    window.run()
}
