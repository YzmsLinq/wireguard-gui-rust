//! `wgp` — WireGuard client for Linux, written in Rust.
//!
//! This is a port of <https://github.com/gary113/wireguard-gui-python>: the
//! interface is the same, the configurations are the same
//! (`/etc/wireguard/*.conf`) and the interfaces are still started and stopped
//! through `wg-quick@<name>.service`.
//!
//! Like the Python version, the application manages system services and the
//! configurations of `/etc/wireguard`, so it has to run as root.

mod app;
mod config;
mod dialogs;
mod file_picker;
mod fonts;
mod i18n;
mod icons;
mod log_watcher;
mod settings;
mod tray;
mod wg;
mod widgets;

use eframe::egui;

fn main() -> eframe::Result<()> {
    let language_preference = i18n::saved_preference();
    let lang = i18n::resolve(&language_preference);
    let is_root = unsafe { libc::geteuid() } == 0;

    let viewport = egui::ViewportBuilder::default()
        .with_title("wgp")
        .with_app_id("wgp")
        .with_icon(icons::window_icon())
        .with_inner_size([900.0, 600.0])
        .with_min_inner_size([760.0, 520.0]);

    let options = eframe::NativeOptions {
        viewport,
        ..Default::default()
    };

    eframe::run_native(
        "wgp",
        options,
        Box::new(move |creation_context| {
            fonts::install(&creation_context.egui_ctx);

            let application: Box<dyn eframe::App> = if is_root {
                Box::new(app::WgpApp::new(creation_context))
            } else {
                Box::new(app::PrivilegeErrorApp::new(lang))
            };

            Ok(application)
        }),
    )
}
