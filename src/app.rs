//! The main window, the counterpart of `views/main.py`.

use std::collections::{BTreeSet, HashMap};
use std::time::{Duration, Instant};

use eframe::egui::{self, Color32, RichText};

use crate::dialogs::{self, Action as DialogAction, ConfigDialog};
use crate::file_picker::{FilePicker, Outcome};
use crate::fonts;
use crate::i18n::{self, t, Lang, Msg};
use crate::icons::{Icons, LIST_ICON_SIZE, TOOLBAR_ICON_SIZE};
use crate::log_watcher::LogWatcher;
use crate::tray::{Action as TrayAction, TrayBridge};
use crate::wg;
use crate::widgets::{self, ReadOnlyText};

/// Version displayed in the status bar.
pub const VERSION: &str = "v1.6-rust";

/// How often the interfaces are refreshed, in milliseconds.
const UPDATE_INTERVAL: Duration = Duration::from_millis(1000);

/// How many log lines are kept in memory.
const MAX_LOG_LINES: usize = 5000;

/// How long a status message stays displayed.
const STATUS_TIMEOUT: Duration = Duration::from_secs(8);

#[derive(Clone, Copy, PartialEq, Eq)]
enum Tab {
    Configs,
    Logs,
}

/// What a file picker is used for.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Picker {
    Add,
    Import,
    Export,
}

/// An action waiting for a confirmation.
enum Pending {
    Delete(Vec<String>),
}

/// The window displayed on top of the main one.
enum Modal {
    Form(ConfigDialog),
    Message {
        title: Msg,
        body: String,
    },
    Confirm {
        title: Msg,
        body: String,
        pending: Pending,
    },
}

pub struct WgpApp {
    lang_preference: String,
    lang: Lang,
    tab: Tab,
    files: Vec<String>,
    active: HashMap<String, bool>,
    checked: BTreeSet<String>,
    selected: Option<String>,
    interface_text: ReadOnlyText,
    peers_text: ReadOnlyText,
    interface_title: String,
    logs: Vec<String>,
    log_watcher: LogWatcher,
    icons: Icons,
    modal: Option<Modal>,
    picker: Option<(FilePicker, Picker)>,
    menu_open: bool,
    menu_opened_at: Option<Instant>,
    menu_above: bool,
    menu_offset: egui::Vec2,
    last_refresh: Instant,
    tray: TrayBridge,
    status: Option<(String, Instant)>,
    allow_close: bool,
    show_window: bool,
    missing_font_warned: bool,
}

impl WgpApp {
    pub fn new(creation_context: &eframe::CreationContext<'_>) -> Self {
        let ctx = &creation_context.egui_ctx;
        let lang_preference = i18n::saved_preference();
        let lang = i18n::resolve(&lang_preference);
        let tray = TrayBridge::spawn(lang);
        tray.set_context(ctx);

        let mut app = Self {
            lang_preference,
            lang,
            tab: Tab::Configs,
            files: Vec::new(),
            active: HashMap::new(),
            checked: BTreeSet::new(),
            selected: None,
            interface_text: ReadOnlyText::default(),
            peers_text: ReadOnlyText::default(),
            interface_title: String::new(),
            logs: Vec::new(),
            log_watcher: LogWatcher::spawn(),
            icons: Icons::load(ctx),
            modal: None,
            picker: None,
            menu_open: false,
            menu_opened_at: None,
            menu_above: false,
            menu_offset: egui::Vec2::ZERO,
            last_refresh: Instant::now(),
            tray,
            status: None,
            allow_close: false,
            show_window: false,
            missing_font_warned: false,
        };

        app.refresh();
        app
    }

    // ---------------------------------------------------------------- state

    /// Reads the interfaces and refreshes everything that depends on them.
    fn refresh(&mut self) {
        self.files = wg::interfaces();
        self.active = self
            .files
            .iter()
            .map(|file| (file.clone(), wg::is_active(file)))
            .collect();

        self.checked.retain(|file| self.active.contains_key(file));

        if let Some(selected) = self.selected.clone() {
            if !self.active.contains_key(&selected) {
                self.selected = None;
            }
        }

        self.refresh_details();
        self.publish_tray();
    }

    /// Refreshes the two text areas of the selected interface.
    fn refresh_details(&mut self) {
        let Some(file) = self.selected.clone() else {
            self.interface_title = t(Msg::Interface, self.lang).to_owned();
            self.interface_text.set(String::new());
            self.peers_text.set(String::new());
            return;
        };

        let is_active = self.active.get(&file).copied().unwrap_or(false);

        let parts = if is_active {
            wg::active_config_parts(&file).unwrap_or_default()
        } else {
            wg::config_parts(&file).unwrap_or_default()
        };

        self.interface_title = format!(
            "{}{}",
            t(Msg::InterfacePrefix, self.lang),
            wg::interface_name(&file)
        );

        if self.interface_text.text() != parts.interface {
            self.interface_text.set(parts.interface);
        }

        if self.peers_text.text() != parts.peers {
            self.peers_text.set(parts.peers);
        }
    }

    fn publish_tray(&mut self) {
        let interfaces: Vec<(String, bool)> = self
            .files
            .iter()
            .map(|file| (file.clone(), self.active.get(file).copied().unwrap_or(false)))
            .collect();

        let active_count = interfaces.iter().filter(|(_, active)| *active).count();

        self.tray.publish(interfaces, active_count, self.lang);
    }

    fn set_status(&mut self, message: impl Into<String>) {
        self.status = Some((message.into(), Instant::now()));
    }

    fn selection(&self) -> Vec<String> {
        if !self.checked.is_empty() {
            self.checked.iter().cloned().collect()
        } else {
            self.selected.iter().cloned().collect()
        }
    }

    fn show_error(&mut self, title: Msg, body: impl Into<String>) {
        self.modal = Some(Modal::Message {
            title,
            body: body.into(),
        });
    }

    fn report_error(&mut self, error: wg::Error) {
        let body = match error {
            wg::Error::Validation(validation) => t(dialogs::validation_message(validation), self.lang).to_owned(),
            other => other.to_string(),
        };

        self.show_error(Msg::Error, body);
    }

    // ----------------------------------------------------------------- tray

    fn handle_tray_actions(&mut self) {
        for action in self.tray.take_actions() {
            match action {
                TrayAction::Turn(file) => {
                    wg::turn_interface(&file);
                    self.refresh();
                }
                TrayAction::Show => self.show_window = true,
                TrayAction::Quit => {
                    self.allow_close = true;
                    self.quit();
                }
            }
        }
    }

    fn quit(&mut self) {
        self.allow_close = true;
    }

    // -------------------------------------------------------------- actions

    fn open_new_dialog(&mut self) {
        self.modal = Some(Modal::Form(ConfigDialog::new_interface()));
    }

    fn open_edit_dialog(&mut self) {
        let Some(file) = self.selected.clone() else {
            self.show_error(Msg::Error, t(Msg::SelectInterface, self.lang));
            return;
        };

        let content = wg::read_config(&file).unwrap_or_default();
        let dialog = ConfigDialog::edit(&file, wg::interface_name(&file), &content);
        self.modal = Some(Modal::Form(dialog));
    }

    fn turn_selected(&mut self) {
        let targets = self.selection();

        if targets.is_empty() {
            self.show_error(Msg::Error, t(Msg::SelectInterface, self.lang));
            return;
        }

        for file in targets {
            wg::turn_interface(&file);
        }

        self.refresh();
    }

    fn ask_delete(&mut self) {
        let targets = self.selection();

        if targets.is_empty() {
            self.show_error(Msg::Error, t(Msg::SelectInterface, self.lang));
            return;
        }

        let body = format!(
            "{}{}{}",
            t(Msg::ConfirmDeletePrefix, self.lang),
            targets.len(),
            t(Msg::ConfirmDeleteSuffix, self.lang)
        );

        self.modal = Some(Modal::Confirm {
            title: Msg::ConfirmDeleting,
            body,
            pending: Pending::Delete(targets),
        });
    }

    fn delete_interfaces(&mut self, files: Vec<String>) {
        for file in files {
            if let Err(error) = wg::delete_interface(&file) {
                self.report_error(error);
                break;
            }
        }

        self.checked.clear();
        self.selected = None;
        self.refresh();
    }

    fn open_picker(&mut self, picker: Picker) {
        let file_picker = match picker {
            Picker::Add => FilePicker::file("conf"),
            Picker::Import => FilePicker::file("zip"),
            Picker::Export => FilePicker::directory(),
        };

        self.picker = Some((file_picker, picker));
    }

    fn handle_picked(&mut self, picker: Picker, outcome: Outcome) {
        let Outcome::Picked(path) = outcome else {
            return;
        };

        let result = match picker {
            Picker::Add => wg::add_interface(&path).map(|_| ()),
            Picker::Import => wg::import_interfaces(&path).map(|_| ()),
            Picker::Export => wg::export_interfaces(&path).map(|_| ()),
        };

        match result {
            Ok(()) => {
                let message = match picker {
                    Picker::Add => Msg::InterfaceAdded,
                    Picker::Import => Msg::Imported,
                    Picker::Export => Msg::Exported,
                };

                self.set_status(t(message, self.lang));
                self.refresh();
            }
            Err(error) => self.report_error(error),
        }
    }

    fn apply_dialog(&mut self, dialog: ConfigDialog) -> bool {
        match dialog.kind.clone() {
            dialogs::Kind::New => match wg::new_interface(&dialog.name, &dialog.content) {
                Ok(_) => {
                    let message = format!(
                        "{}{}{}",
                        t(Msg::Interface, self.lang),
                        dialog.name.trim(),
                        t(Msg::AddedSuccessfullySuffix, self.lang)
                    );

                    self.set_status(message);
                    self.refresh();
                    true
                }
                Err(wg::Error::Validation(validation)) => {
                    let mut dialog = dialog;
                    dialog.set_error(dialogs::validation_message(validation));
                    self.modal = Some(Modal::Form(dialog));
                    false
                }
                Err(error) => {
                    self.report_error(error);
                    false
                }
            },
            dialogs::Kind::Edit { file } => {
                let old_content = wg::read_config(&file).unwrap_or_default();

                match wg::edit_interface(&file, &dialog.name, &old_content, &dialog.content) {
                    Ok(()) => {
                        self.set_status(t(Msg::ConfigUpdated, self.lang));

                        let renamed = wg::sanitize_name(&dialog.name);
                        if !renamed.is_empty() && renamed != wg::interface_name(&file) {
                            self.selected = Some(format!("{renamed}.conf"));
                            self.checked.clear();
                        } else {
                            self.selected = Some(file);
                        }

                        self.refresh();
                        true
                    }
                    Err(wg::Error::Validation(validation)) => {
                        self.set_status(t(Msg::ConfigInvalidRollback, self.lang));
                        let mut dialog = dialog;
                        dialog.set_error(dialogs::validation_message(validation));
                        self.modal = Some(Modal::Form(dialog));
                        self.refresh();
                        false
                    }
                    Err(error) => {
                        self.report_error(error);
                        false
                    }
                }
            }
        }
    }

    // ------------------------------------------------------------------ ui

    fn top_bar(&mut self, ctx: &egui::Context) {
        egui::TopBottomPanel::top("wgp_tabs")
            .frame(
                egui::Frame::side_top_panel(&ctx.style())
                    .inner_margin(egui::Margin::symmetric(10.0, 6.0)),
            )
            .show(ctx, |ui| {
                ui.horizontal(|ui| {
                    let lang = self.lang;

                    ui.add(egui::Image::from_texture(egui::load::SizedTexture::new(
                        self.icons.app.id(),
                        egui::vec2(20.0, 20.0),
                    )));
                    ui.add_space(4.0);

                    ui.selectable_value(&mut self.tab, Tab::Configs, t(Msg::Configs, lang));
                    ui.selectable_value(&mut self.tab, Tab::Logs, t(Msg::Logs, lang));
                });
            });
    }

    fn status_bar(&mut self, ctx: &egui::Context) {
        egui::TopBottomPanel::bottom("wgp_status").show(ctx, |ui| {
            ui.horizontal(|ui| {
                ui.label(RichText::new(VERSION).small().weak());

                if let Some((message, instant)) = &self.status {
                    if instant.elapsed() < STATUS_TIMEOUT {
                        ui.separator();
                        ui.label(RichText::new(message).small());
                    }
                }

                if self.lang == Lang::ZhCn && !fonts::cjk_available() && !self.missing_font_warned {
                    self.missing_font_warned = true;
                    ui.separator();
                    ui.colored_label(
                        Color32::from_rgb(180, 110, 20),
                        RichText::new(t(Msg::MissingCjkFont, self.lang)).small(),
                    );
                }

                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    let count = self.active.values().filter(|active| **active).count();
                    ui.label(
                        RichText::new(format!(
                            "{}{}",
                            t(Msg::ActiveCount, self.lang),
                            count
                        ))
                        .small()
                        .weak(),
                    );
                });
            });
        });
    }

    fn interfaces_panel(&mut self, ctx: &egui::Context) {
        egui::SidePanel::left("wgp_interfaces")
            .resizable(true)
            .default_width(430.0)
            .min_width(300.0)
            .show(ctx, |ui| {
                egui::ScrollArea::vertical()
                    .id_salt("wgp_interfaces_list")
                    .auto_shrink([false, false])
                    .show(ui, |ui| self.interface_list(ui));
            });
    }

    /// The toolbar, above the status bar.
    fn toolbar_panel(&mut self, ctx: &egui::Context) {
        egui::TopBottomPanel::bottom("wgp_toolbar")
            .frame(
                egui::Frame::side_top_panel(&ctx.style())
                    .inner_margin(egui::Margin::symmetric(8.0, 4.0)),
            )
            .show(ctx, |ui| {
                ui.horizontal(|ui| self.toolbar(ui));
            });
    }

    fn interface_list(&mut self, ui: &mut egui::Ui) {
        if self.files.is_empty() {
            ui.add_space(8.0);
            ui.weak("—");
            return;
        }

        let multi_select = ui.input(|input| input.modifiers.ctrl || input.modifiers.command);
        let files = self.files.clone();

        for file in files {
            let is_active = self.active.get(&file).copied().unwrap_or(false);
            let icon = if is_active {
                &self.icons.activated
            } else {
                &self.icons.deactivated
            };

            let checked = self.checked.contains(&file);

            let response = ui
                .horizontal(|ui| {
                    let image = egui::Image::from_texture(egui::load::SizedTexture::new(
                        icon.id(),
                        egui::vec2(LIST_ICON_SIZE, LIST_ICON_SIZE),
                    ));

                    ui.add(image);

                    ui.selectable_label(checked, wg::interface_name(&file))
                })
                .inner;

            if response.clicked() {
                if multi_select {
                    if checked {
                        self.checked.remove(&file);
                    } else {
                        self.checked.insert(file.clone());
                        self.selected = Some(file.clone());
                    }
                } else {
                    self.checked.clear();
                    self.checked.insert(file.clone());
                    self.selected = Some(file.clone());
                }

                self.refresh_details();
            }

            if response.double_clicked() {
                wg::turn_interface(&file);
                self.refresh();
            }
        }
    }

    fn toolbar(&mut self, ui: &mut egui::Ui) {
        let lang = self.lang;
        let has_selection = !self.selection().is_empty();
        let selected_active = self
            .selected
            .as_ref()
            .and_then(|file| self.active.get(file))
            .copied()
            .unwrap_or(false);

        ui.add_space(4.0);

        ui.horizontal_wrapped(|ui| {
            let menu_button = tool_button(
                ui,
                &self.icons.add,
                t(Msg::SetIfLang, lang),
                true,
            );

            if menu_button.clicked() {
                self.menu_open = !self.menu_open;
                self.menu_opened_at = Some(Instant::now());

                let screen = ui.ctx().screen_rect();
                let space_below = screen.bottom() - menu_button.rect.bottom();
                let space_above = menu_button.rect.top() - screen.top();

                self.menu_above = space_above > space_below;
                self.menu_offset = if self.menu_above {
                    egui::vec2(
                        menu_button.rect.left(),
                        screen.bottom() - menu_button.rect.top() + 4.0,
                    )
                } else {
                    egui::vec2(
                        menu_button.rect.left(),
                        menu_button.rect.bottom() - screen.top() + 4.0,
                    )
                };
            }

            if tool_button(ui, &self.icons.remove, t(Msg::Delete, lang), has_selection).clicked() {
                self.ask_delete();
            }

            if tool_button(ui, &self.icons.archive, t(Msg::Export, lang), true).clicked() {
                self.open_picker(Picker::Export);
            }

            let (icon, label) = if selected_active {
                (&self.icons.down, t(Msg::Deactivate, lang))
            } else {
                (&self.icons.up, t(Msg::ActivateDeactivate, lang))
            };

            if tool_button(ui, icon, label, has_selection).clicked() {
                self.turn_selected();
            }

            if tool_button(ui, &self.icons.edit, t(Msg::Edit, lang), self.selected.is_some())
                .clicked()
            {
                self.open_edit_dialog();
            }
        });
    }

    fn details_panel(&mut self, ui: &mut egui::Ui) {
        let available = ui.available_height();
        let interface_height = (available * 0.42).clamp(120.0, available - 120.0);
        let interface_title = self.interface_title.clone();

        widgets::group_box(ui, &interface_title, interface_height, |ui| {
            egui::ScrollArea::vertical()
                .id_salt("wgp_interface_view")
                .auto_shrink([false, false])
                .show(ui, |ui| {
                    widgets::code_view(ui, &mut self.interface_text, 6);
                });
        });

        ui.add_space(6.0);

        let peers_height = (ui.available_height() - 6.0).max(80.0);
        let title = t(Msg::Peers, self.lang).to_owned();

        widgets::group_box(ui, &title, peers_height, |ui| {
            egui::ScrollArea::vertical()
                .id_salt("wgp_peers_view")
                .auto_shrink([false, false])
                .show(ui, |ui| {
                    widgets::code_view(ui, &mut self.peers_text, 10);
                });
        });
    }

    fn logs_panel(&mut self, ui: &mut egui::Ui) {
        egui::ScrollArea::vertical()
            .id_salt("wgp_logs_view")
            .auto_shrink([false, false])
            .stick_to_bottom(true)
            .show(ui, |ui| {
                if self.logs.is_empty() {
                    ui.weak("—");
                }

                for line in &self.logs {
                    ui.monospace(line);
                }
            });
    }

    fn show_options_menu(&mut self, ctx: &egui::Context) {
        if !self.menu_open {
            return;
        }

        let lang = self.lang;
        let mut close = false;

        let anchor = if self.menu_above {
            egui::Align2::LEFT_BOTTOM
        } else {
            egui::Align2::LEFT_TOP
        };

        let area = egui::Area::new(egui::Id::new("wgp_options_menu"))
            .order(egui::Order::Foreground)
            .anchor(anchor, self.menu_offset)
            .show(ctx, |ui| {
                egui::Frame::popup(ui.style()).show(ui, |ui| {
                    ui.set_min_width(190.0);

                    if ui.button(t(Msg::NewInterface, lang)).clicked() {
                        self.open_new_dialog();
                        close = true;
                    }

                    if ui.button(t(Msg::AddInterfaceFile, lang)).clicked() {
                        self.open_picker(Picker::Add);
                        close = true;
                    }

                    if ui.button(t(Msg::ImportInterfaces, lang)).clicked() {
                        self.open_picker(Picker::Import);
                        close = true;
                    }

                    ui.separator();
                    ui.label(RichText::new(t(Msg::Language, lang)).small().strong());

                    for (code, name) in i18n::choices() {
                        let label = match name {
                            Some(name) => name.to_owned(),
                            None => t(Msg::SystemDefault, lang).to_owned(),
                        };

                        if ui.radio(self.lang_preference == code, label).clicked() {
                            self.lang_preference = code.to_owned();
                            i18n::save_preference(code);
                            self.lang = i18n::resolve(code);
                            self.interface_title = String::new();
                            self.refresh_details();
                            self.publish_tray();
                            close = true;
                        }
                    }

                    ui.separator();

                    if ui.button(t(Msg::Quit, lang)).clicked() {
                        self.allow_close = true;
                        close = true;
                    }
                });
            });

        let rect = area.response.rect;

        // Ignore the click that opened the menu: it is still reported as a
        // click during the frame the menu was opened.
        let opened_recently = self
            .menu_opened_at
            .map(|instant| instant.elapsed() < Duration::from_millis(300))
            .unwrap_or(true);

        if !opened_recently && ctx.input(|input| input.pointer.any_click()) {
            if let Some(position) = ctx.pointer_latest_pos() {
                if !rect.contains(position) {
                    close = true;
                }
            }
        }

        if close {
            self.menu_open = false;
        }
    }

    fn show_modal(&mut self, ctx: &egui::Context) {
        let Some(modal) = self.modal.take() else {
            return;
        };

        match modal {
            Modal::Form(mut dialog) => match dialog.show(ctx, self.lang) {
                DialogAction::None => self.modal = Some(Modal::Form(dialog)),
                DialogAction::Cancel => {}
                DialogAction::Accept => {
                    self.apply_dialog(dialog);
                }
            },
            Modal::Message { title, body } => {
                let mut close = false;

                widgets::dim_background(ctx);

                egui::Window::new(t(title, self.lang))
                    .id(egui::Id::new("wgp_message"))
                    .order(egui::Order::Foreground)
                    .collapsible(false)
                    .resizable(false)
                    .default_width(380.0)
                    .anchor(egui::Align2::CENTER_CENTER, egui::vec2(0.0, 0.0))
                    .show(ctx, |ui| {
                        ui.label(body.clone());
                        ui.add_space(8.0);

                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            if ui.button(t(Msg::Ok, self.lang)).clicked() {
                                close = true;
                            }
                        });
                    });

                if !close {
                    self.modal = Some(Modal::Message { title, body });
                }
            }
            Modal::Confirm {
                title,
                body,
                pending,
            } => {
                let mut confirmed = None;

                widgets::dim_background(ctx);

                egui::Window::new(t(title, self.lang))
                    .id(egui::Id::new("wgp_confirm"))
                    .order(egui::Order::Foreground)
                    .collapsible(false)
                    .resizable(false)
                    .default_width(400.0)
                    .anchor(egui::Align2::CENTER_CENTER, egui::vec2(0.0, 0.0))
                    .show(ctx, |ui| {
                        ui.label(body.clone());
                        ui.add_space(8.0);

                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            if ui.button(t(Msg::Yes, self.lang)).clicked() {
                                confirmed = Some(true);
                            }

                            if ui.button(t(Msg::No, self.lang)).clicked() {
                                confirmed = Some(false);
                            }
                        });
                    });

                match confirmed {
                    Some(true) => match pending {
                        Pending::Delete(files) => self.delete_interfaces(files),
                    },
                    Some(false) => {}
                    None => {
                        self.modal = Some(Modal::Confirm {
                            title,
                            body,
                            pending,
                        })
                    }
                }
            }
        }
    }

    fn show_picker(&mut self, ctx: &egui::Context) {
        let Some((mut picker, purpose)) = self.picker.take() else {
            return;
        };

        let title = match purpose {
            Picker::Add => Msg::SelectConfigFile,
            Picker::Import => Msg::SelectZipFile,
            Picker::Export => Msg::SelectDirectory,
        };

        match picker.show(ctx, self.lang, title) {
            Some(Outcome::Picked(path)) => self.handle_picked(purpose, Outcome::Picked(path)),
            Some(Outcome::Cancelled) => {}
            None => self.picker = Some((picker, purpose)),
        }
    }

    fn collect_logs(&mut self) {
        let lines = self.log_watcher.drain();

        if lines.is_empty() {
            return;
        }

        self.logs.extend(lines);

        if self.logs.len() > MAX_LOG_LINES {
            let excess = self.logs.len() - MAX_LOG_LINES;
            self.logs.drain(..excess);
        }
    }

    fn handle_close_request(&mut self, ctx: &egui::Context) {
        if !ctx.input(|input| input.viewport().close_requested()) {
            return;
        }

        if !self.allow_close && self.tray.is_available() {
            ctx.send_viewport_cmd(egui::ViewportCommand::CancelClose);
            ctx.send_viewport_cmd(egui::ViewportCommand::Visible(false));
        }
    }

    fn handle_show_request(&mut self, ctx: &egui::Context) {
        if !self.show_window {
            return;
        }

        self.show_window = false;
        ctx.send_viewport_cmd(egui::ViewportCommand::Visible(true));
        ctx.send_viewport_cmd(egui::ViewportCommand::Focus);
        ctx.send_viewport_cmd(egui::ViewportCommand::Minimized(false));
    }
}

impl eframe::App for WgpApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        self.handle_tray_actions();
        self.handle_show_request(ctx);
        self.handle_close_request(ctx);
        self.collect_logs();

        if self.last_refresh.elapsed() >= UPDATE_INTERVAL {
            self.last_refresh = Instant::now();
            self.refresh();
        }

        if self.allow_close {
            ctx.send_viewport_cmd(egui::ViewportCommand::Close);
        }

        self.top_bar(ctx);
        self.status_bar(ctx);
        self.toolbar_panel(ctx);

        match self.tab {
            Tab::Configs => {
                self.interfaces_panel(ctx);

                egui::CentralPanel::default()
                    .frame(egui::Frame::central_panel(&ctx.style()).inner_margin(egui::Margin::same(8.0)))
                    .show(ctx, |ui| self.details_panel(ui));
            }
            Tab::Logs => {
                egui::CentralPanel::default()
                    .frame(egui::Frame::central_panel(&ctx.style()).inner_margin(egui::Margin::same(8.0)))
                    .show(ctx, |ui| self.logs_panel(ui));
            }
        }

        if self.modal.is_none() && self.picker.is_none() {
            if ctx.input(|input| input.key_pressed(egui::Key::Escape)) {
                self.checked.clear();
            }
        }

        self.show_modal(ctx);
        self.show_picker(ctx);
        self.show_options_menu(ctx);

        ctx.request_repaint_after(Duration::from_millis(250));
    }
}

/// A toolbar button: an icon above its label.
fn tool_button(
    ui: &mut egui::Ui,
    icon: &egui::TextureHandle,
    label: &str,
    enabled: bool,
) -> egui::Response {
    let image = egui::Image::from_texture(egui::load::SizedTexture::new(
        icon.id(),
        egui::vec2(TOOLBAR_ICON_SIZE, TOOLBAR_ICON_SIZE),
    ));

    let button = egui::Button::image_and_text(image, label);

    ui.add_enabled(enabled, button)
}

/// The window displayed when the application is not started as root.
pub struct PrivilegeErrorApp {
    lang: Lang,
}

impl PrivilegeErrorApp {
    pub fn new(lang: Lang) -> Self {
        Self { lang }
    }
}

impl eframe::App for PrivilegeErrorApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        egui::CentralPanel::default().show(ctx, |ui| {
            ui.vertical_centered(|ui| {
                ui.add_space(40.0);
                ui.heading(t(Msg::RootRequired, self.lang));
                ui.add_space(8.0);
                ui.label(t(Msg::RootRequiredDetail, self.lang));
                ui.add_space(20.0);

                if ui.button(t(Msg::Close, self.lang)).clicked() {
                    ctx.send_viewport_cmd(egui::ViewportCommand::Close);
                }
            });
        });
    }
}
