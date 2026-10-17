#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod accounts;
mod app;
mod auth;
mod backend;
mod controller;
mod fabric;
mod fsutil;
mod http;
mod launch;
mod modrinth;
mod mods;
mod mojang;
mod paths;
mod platform;
mod settings;
mod update;

use std::sync::Arc;

slint::include_modules!();

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let started = std::time::Instant::now();
    let window = AppWindow::new()?;
    window.show()?;

    // Nothing touches the network or the disk before the window is up.
    let runtime = tokio::runtime::Builder::new_multi_thread().worker_threads(4).enable_all().build()?;
    let paths = paths::AppPaths::resolve()?;
    let app = app::App::new(&window, runtime.handle().clone(), paths, Arc::new(accounts::KeyringStore));
    wire_callbacks(&window, &app);
    app.start_background();

    if cfg!(debug_assertions) {
        slint::Timer::single_shot(std::time::Duration::ZERO, move || {
            eprintln!("frame_ms={}", started.elapsed().as_millis());
        });
    }
    window.run()?;
    drop(runtime);
    Ok(())
}

fn wire_callbacks(w: &AppWindow, app: &app::App) {
    macro_rules! on {
        ($setter:ident, |$app:ident $(, $arg:ident)*| $body:expr) => {{
            let $app = app.clone();
            w.$setter(move |$($arg),*| $body);
        }};
    }
    on!(on_toggle_account, |a, id| a.toggle_account(id.into()));
    on!(on_stop_account, |a, id| a.stop_account(id.into()));
    on!(on_remove_account, |a, id| a.remove_account(id.into()));
    on!(on_start, |a| a.start());
    on!(on_add_microsoft, |a| a.add_microsoft());
    on!(on_add_offline, |a, name| a.add_offline(name.into()));
    on!(on_search, |a, q| a.search(q.into()));
    on!(on_install_mod, |a, id| a.install_mod(id.into()));
    on!(on_toggle_mod, |a, id, on| a.toggle_mod(id.into(), on));
    on!(on_remove_mod, |a, id| a.remove_mod(id.into()));
    on!(on_check_updates, |a| a.check_updates());
    on!(on_update_mod, |a, id| a.update_mod(id.into()));
    on!(on_confirm_ram, |a, go| a.confirm_ram(go));
    on!(on_close_dialog, |a| a.close_dialog());
    on!(on_open_url, |_a, url| {
        let _ = open::that(url.as_str());
    });

    let weak = w.as_weak();
    let a = app.clone();
    w.on_save_settings(move || {
        if let Some(w) = weak.upgrade() {
            a.save_settings(w.get_heap_mb(), w.get_jvm_args().into(), w.get_minimize_on_launch());
        }
    });
    let weak = w.as_weak();
    let a = app.clone();
    w.on_confirm_defaults(move || {
        if let Some(w) = weak.upgrade() {
            a.confirm_defaults(w.get_d_sodium(), w.get_d_lithium(), w.get_d_ferrite());
        }
    });
}
