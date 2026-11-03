//! The system tray icon and its menu.
//!
//! The Python version used `QSystemTrayIcon`; this one implements the
//! `StatusNotifierItem` specification through the `ksni` crate, which is what
//! every Linux desktop (KDE, GNOME with an extension, Xfce, DDE, ...) provides
//! today.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};

use eframe::egui;
use ksni::{Icon, MenuItem, Tray};

use crate::i18n::{t, Lang, Msg};
use crate::icons;

/// Something the user asked from the tray menu.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Action {
    /// Activate or deactivate an interface, by configuration file name.
    Turn(String),
    /// Show the main window.
    Show,
    /// Quit the application.
    Quit,
}

struct State {
    interfaces: Vec<(String, bool)>,
    active_count: usize,
    lang: Lang,
    pending: Vec<Action>,
    context: Option<egui::Context>,
}

struct WgTray {
    state: Arc<Mutex<State>>,
}

impl WgTray {
    fn push(&self, action: Action) {
        let context = {
            let mut state = match self.state.lock() {
                Ok(state) => state,
                Err(_) => return,
            };

            state.pending.push(action);
            state.context.clone()
        };

        if let Some(context) = context {
            context.request_repaint();
        }
    }

    fn pixmap(&self) -> Vec<Icon> {
        let count = self
            .state
            .lock()
            .map(|state| state.active_count)
            .unwrap_or(0);

        let (data, width, height) = icons::tray_pixmap(count);

        vec![Icon {
            width,
            height,
            data,
        }]
    }
}

impl Tray for WgTray {
    fn id(&self) -> String {
        "wgr".to_owned()
    }

    fn title(&self) -> String {
        "wgr".to_owned()
    }

    fn icon_pixmap(&self) -> Vec<Icon> {
        self.pixmap()
    }

    fn tool_tip(&self) -> ksni::ToolTip {
        let (title, description) = self
            .state
            .lock()
            .map(|state| {
                (
                    format!(
                        "{} — {}",
                        t(Msg::AppTitle, state.lang),
                        t(Msg::TrayTooltip, state.lang)
                    ),
                    format!(
                        "{}{}",
                        t(Msg::ActiveCount, state.lang),
                        state.active_count
                    ),
                )
            })
            .unwrap_or_else(|_| ("wgr".to_owned(), String::new()));

        ksni::ToolTip {
            icon_name: String::new(),
            icon_pixmap: self.pixmap(),
            title,
            description,
        }
    }

    fn menu(&self) -> Vec<MenuItem<Self>> {
        let Ok(state) = self.state.lock() else {
            return Vec::new();
        };

        let lang = state.lang;
        let mut items = Vec::new();

        for (file, active) in &state.interfaces {
            let name = crate::wg::interface_name(file).to_owned();
            let label = if *active {
                format!("✓ {name}")
            } else {
                name
            };

            let target = file.clone();

            items.push(MenuItem::Standard(ksni::menu::StandardItem {
                label,
                activate: Box::new(move |tray: &mut WgTray| {
                    tray.push(Action::Turn(target.clone()));
                }),
                ..Default::default()
            }));
        }

        if !state.interfaces.is_empty() {
            items.push(MenuItem::Separator);
        }

        items.push(MenuItem::Standard(ksni::menu::StandardItem {
            label: t(Msg::Show, lang).to_owned(),
            activate: Box::new(|tray: &mut WgTray| tray.push(Action::Show)),
            ..Default::default()
        }));

        items.push(MenuItem::Standard(ksni::menu::StandardItem {
            label: t(Msg::Quit, lang).to_owned(),
            activate: Box::new(|tray: &mut WgTray| tray.push(Action::Quit)),
            ..Default::default()
        }));

        items
    }

    fn activate(&mut self, _x: i32, _y: i32) {
        self.push(Action::Show);
    }
}

/// The application side of the tray: it publishes what the menu should show
/// and collects what the user clicked.
pub struct TrayBridge {
    state: Arc<Mutex<State>>,
    refresh: Box<dyn Fn() + Send>,
    failed: Arc<AtomicBool>,
}

impl TrayBridge {
    /// Registers the tray icon. When the session has no status notifier the
    /// application keeps working without a tray icon.
    pub fn spawn(lang: Lang) -> Self {
        let state = Arc::new(Mutex::new(State {
            interfaces: Vec::new(),
            active_count: 0,
            lang,
            pending: Vec::new(),
            context: None,
        }));

        let service = ksni::TrayService::new(WgTray {
            state: state.clone(),
        });

        let handle = service.handle();
        let failed = Arc::new(AtomicBool::new(false));
        let flag = failed.clone();

        std::thread::Builder::new()
            .name("wgr-tray".to_owned())
            .spawn(move || {
                if service.run().is_err() {
                    flag.store(true, Ordering::Relaxed);
                }
            })
            .ok();

        Self {
            state,
            refresh: Box::new(move || {
                handle.update(|_| ());
            }),
            failed,
        }
    }

    /// Whether the tray icon could be registered.
    pub fn is_available(&self) -> bool {
        !self.failed.load(Ordering::Relaxed)
    }

    /// Lets the tray wake the interface up when the user clicks on it.
    pub fn set_context(&self, context: &egui::Context) {
        if let Ok(mut state) = self.state.lock() {
            state.context = Some(context.clone());
        }
    }

    /// Publishes the interfaces shown in the tray menu.
    pub fn publish(&self, interfaces: Vec<(String, bool)>, active_count: usize, lang: Lang) {
        {
            let Ok(mut state) = self.state.lock() else {
                return;
            };

            state.interfaces = interfaces;
            state.active_count = active_count;
            state.lang = lang;
        }

        // The lock is released before updating the tray: the service thread
        // takes it while it reads the menu, and the update takes the tray lock.
        (self.refresh)();
    }

    /// Takes the actions requested from the tray menu.
    pub fn take_actions(&self) -> Vec<Action> {
        match self.state.lock() {
            Ok(mut state) => std::mem::take(&mut state.pending),
            Err(_) => Vec::new(),
        }
    }
}
