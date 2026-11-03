//! A small file picker, drawn with `egui`.
//!
//! `wgr` has to work in the minimal sessions where it is usually started
//! (`sudo` from a desktop icon), where no file chooser portal is guaranteed to
//! be available, so the picker is part of the application.

use std::path::{Path, PathBuf};

use eframe::egui;

use crate::i18n::{t, Lang, Msg};

/// What the picker selects.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Mode {
    /// A file with the given extension (without dot).
    File(&'static str),
    /// A directory.
    Directory,
}

/// How the picker was closed.
#[derive(Debug, Clone)]
pub enum Outcome {
    Picked(PathBuf),
    Cancelled,
}

/// One entry of the listing.
struct Entry {
    path: PathBuf,
    is_directory: bool,
}

pub struct FilePicker {
    mode: Mode,
    directory: PathBuf,
    path_edit: String,
    file_name: String,
    selected: Option<PathBuf>,
    error: Option<Msg>,
}

impl FilePicker {
    pub fn file(extension: &'static str) -> Self {
        Self::new(Mode::File(extension))
    }

    pub fn directory() -> Self {
        Self::new(Mode::Directory)
    }

    fn new(mode: Mode) -> Self {
        let directory = dirs::home_dir()
            .filter(|directory| directory.is_dir())
            .unwrap_or_else(|| PathBuf::from("/"));

        Self {
            mode,
            path_edit: directory.display().to_string(),
            directory,
            file_name: String::new(),
            selected: None,
            error: None,
        }
    }

    fn navigate(&mut self, path: PathBuf) {
        if path.is_dir() {
            self.directory = path;
            self.path_edit = self.directory.display().to_string();
            self.selected = None;
            self.error = None;
        } else {
            self.error = Some(Msg::NotADirectory);
        }
    }

    fn listing(&self) -> Vec<Entry> {
        let Ok(entries) = std::fs::read_dir(&self.directory) else {
            return Vec::new();
        };

        let extension = match self.mode {
            Mode::File(extension) => extension,
            Mode::Directory => "",
        };

        let mut directories = Vec::new();
        let mut files = Vec::new();

        for entry in entries.flatten() {
            let path = entry.path();
            let name = entry.file_name().to_string_lossy().to_string();

            if name.starts_with('.') {
                continue;
            }

            if path.is_dir() {
                directories.push(Entry {
                    path,
                    is_directory: true,
                });
            } else if path
                .extension()
                .and_then(|value| value.to_str())
                .map(|value| value.eq_ignore_ascii_case(extension))
                .unwrap_or(false)
            {
                files.push(Entry {
                    path,
                    is_directory: false,
                });
            }
        }

        directories.sort_by_key(|entry| entry.path.file_name().map(|name| name.to_owned()));
        files.sort_by_key(|entry| entry.path.file_name().map(|name| name.to_owned()));

        directories.extend(files);
        directories
    }

    fn accept(&mut self) -> Option<Outcome> {
        match self.mode {
            Mode::Directory => Some(Outcome::Picked(self.directory.clone())),
            Mode::File(_) => {
                let name = self.file_name.trim();

                if name.is_empty() {
                    return match &self.selected {
                        Some(path) => Some(Outcome::Picked(path.clone())),
                        None => None,
                    };
                }

                let candidate = self.directory.join(name);

                if candidate.is_file() {
                    Some(Outcome::Picked(candidate))
                } else {
                    self.error = Some(Msg::EmptyFolder);
                    None
                }
            }
        }
    }

    /// Draws the picker, returns what the user chose when the dialog closes.
    pub fn show(&mut self, ctx: &egui::Context, lang: Lang, title: Msg) -> Option<Outcome> {
        let mut outcome = None;

        crate::widgets::dim_background(ctx);

        egui::Window::new(t(title, lang))
            .id(egui::Id::new("wgr_file_picker"))
            .order(egui::Order::Foreground)
            .collapsible(false)
            .resizable(true)
            .default_size([600.0, 460.0])
            .min_size([420.0, 320.0])
            .anchor(egui::Align2::CENTER_CENTER, egui::vec2(0.0, 0.0))
            .show(ctx, |ui| {
                ui.horizontal(|ui| {
                    if ui.button("⏶").on_hover_text(t(Msg::Up, lang)).clicked() {
                        if let Some(parent) = self.directory.parent().map(Path::to_path_buf) {
                            self.navigate(parent);
                        }
                    }

                    let response = ui.add(
                        egui::TextEdit::singleline(&mut self.path_edit)
                            .desired_width(f32::INFINITY),
                    );

                    if response.lost_focus() && ui.input(|input| input.key_pressed(egui::Key::Enter))
                    {
                        let path = PathBuf::from(self.path_edit.trim().to_owned());
                        self.navigate(path);
                    }
                });

                ui.separator();

                let entries = self.listing();
                let mut navigate_to = None;
                let mut accept_file = None;

                egui::ScrollArea::vertical()
                    .id_salt("wgr_file_picker_list")
                    .max_height(300.0)
                    .auto_shrink([false, false])
                    .show(ui, |ui| {
                        if entries.is_empty() {
                            ui.weak(t(Msg::EmptyFolder, lang));
                        }

                        for entry in &entries {
                            let name = entry
                                .path
                                .file_name()
                                .map(|name| name.to_string_lossy().to_string())
                                .unwrap_or_default();

                            let label = if entry.is_directory {
                                format!("🗀  {name}")
                            } else {
                                format!("🗎  {name}")
                            };

                            let selected = self.selected.as_deref() == Some(entry.path.as_path());
                            let response = ui.selectable_label(selected, label);

                            if response.clicked() {
                                if entry.is_directory {
                                    navigate_to = Some(entry.path.clone());
                                } else {
                                    self.selected = Some(entry.path.clone());
                                    self.file_name = name.clone();
                                }
                            }

                            if response.double_clicked() && !entry.is_directory {
                                accept_file = Some(entry.path.clone());
                            }
                        }
                    });

                if let Some(path) = navigate_to {
                    self.navigate(path);
                }

                ui.separator();

                ui.horizontal(|ui| {
                    if self.mode == Mode::Directory {
                        ui.label(self.directory.display().to_string());
                    } else {
                        ui.add(
                            egui::TextEdit::singleline(&mut self.file_name)
                                .desired_width(ui.available_width() - 160.0),
                        );
                    }

                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if ui.button(t(Msg::Cancel, lang)).clicked() {
                            outcome = Some(Outcome::Cancelled);
                        }

                        let enabled = match self.mode {
                            Mode::Directory => true,
                            Mode::File(_) => {
                                !self.file_name.trim().is_empty() || self.selected.is_some()
                            }
                        };

                        if ui
                            .add_enabled(enabled, egui::Button::new(t(Msg::Open, lang)))
                            .clicked()
                        {
                            outcome = self.accept();
                        }
                    });
                });

                if let Some(error) = self.error {
                    ui.colored_label(egui::Color32::from_rgb(190, 30, 30), t(error, lang));
                }

                if let Some(path) = accept_file {
                    outcome = Some(Outcome::Picked(path));
                }
            });

        if ctx.input(|input| input.key_pressed(egui::Key::Escape)) {
            outcome = Some(Outcome::Cancelled);
        }

        outcome
    }
}
