#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod http;
mod mojang;
mod paths;

slint::include_modules!();

fn main() -> Result<(), slint::PlatformError> {
    let window = AppWindow::new()?;
    window.run()
}
