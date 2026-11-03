//! `wgr` — WireGuard client for Linux, written in Rust.
//!
//! This is a port of <https://github.com/gary113/wireguard-gui-python>: the
//! interface is the same, the configurations are the same
//! (`/etc/wireguard/*.conf`) and the interfaces are still started and stopped
//! through `wg-quick@<name>.service`.
//!
//! Like the Python version, the application manages system services and the
//! configurations of `/etc/wireguard`, so it has to run as root: when it is
//! started by a normal user it asks for the elevation and starts again, see
//! [`privilege`].

mod app;
mod config;
mod dialogs;
mod file_picker;
mod fonts;
mod i18n;
mod icons;
mod log_watcher;
mod privilege;
mod settings;
mod tray;
mod wg;
mod widgets;

use eframe::egui;
use privilege::{Elevation, Failure};

fn main() -> eframe::Result<()> {
    let language_preference = i18n::saved_preference();
    let lang = i18n::resolve(&language_preference);

    // Nothing to show before the elevation: the dialog of `pkexec` is the
    // only thing the user has to see at that point.
    match privilege::ensure_root() {
        Elevation::Root => run(),
        Elevation::Relaunched(_) => Ok(()),
        Elevation::Failed(failure) => run_error(lang, &failure),
    }
}

fn viewport() -> egui::ViewportBuilder {
    egui::ViewportBuilder::default()
        .with_title("wgr")
        .with_app_id("wgr")
        .with_icon(icons::window_icon())
        .with_inner_size([900.0, 600.0])
        .with_min_inner_size([760.0, 520.0])
}

fn run() -> eframe::Result<()> {
    let options = eframe::NativeOptions {
        viewport: viewport(),
        ..Default::default()
    };

    eframe::run_native(
        "wgr",
        options,
        Box::new(move |creation_context| {
            fonts::install(&creation_context.egui_ctx);

            Ok(Box::new(app::WgpApp::new(creation_context)))
        }),
    )
}

/// The elevation was refused or failed: explain why, and how to start wgr.
fn run_error(lang: i18n::Lang, failure: &Failure) -> eframe::Result<()> {
    let detail = app::elevation_failure_message(failure, lang);

    let options = eframe::NativeOptions {
        viewport: viewport().with_inner_size([620.0, 320.0]),
        ..Default::default()
    };

    eframe::run_native(
        "wgr",
        options,
        Box::new(move |creation_context| {
            fonts::install(&creation_context.egui_ctx);

            Ok(Box::new(app::PrivilegeErrorApp::new(lang, detail)))
        }),
    )
}
