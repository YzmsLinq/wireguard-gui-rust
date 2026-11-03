//! The "new interface" and "edit interface" dialogs, the counterpart of
//! `views/new.py` and `views/edit.py`.

use eframe::egui;

use crate::config;
use crate::i18n::{t, Lang, Msg};
use crate::widgets::{self, ReadOnlyText};

/// Which dialog is displayed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Kind {
    New,
    Edit { file: String },
}

/// What the user did with the dialog.
pub enum Action {
    None,
    Accept,
    Cancel,
}

pub struct ConfigDialog {
    pub kind: Kind,
    pub name: String,
    pub content: String,
    public_key: ReadOnlyText,
    error: Option<Msg>,
}

impl ConfigDialog {
    /// A dialog to create a new interface.
    pub fn new_interface() -> Self {
        Self {
            kind: Kind::New,
            name: String::new(),
            content: String::new(),
            public_key: ReadOnlyText::default(),
            error: None,
        }
    }

    /// A dialog to edit an existing interface.
    pub fn edit(file: &str, name: &str, content: &str) -> Self {
        Self {
            kind: Kind::Edit {
                file: file.to_owned(),
            },
            name: name.to_owned(),
            content: content.to_owned(),
            public_key: ReadOnlyText::new(config::public_key_of(content)),
            error: None,
        }
    }

    /// Shows an error inside the dialog.
    pub fn set_error(&mut self, error: Msg) {
        self.error = Some(error);
    }

    fn title(&self) -> Msg {
        match self.kind {
            Kind::New => Msg::NewDialogTitle,
            Kind::Edit { .. } => Msg::EditDialogTitle,
        }
    }

    pub fn show(&mut self, ctx: &egui::Context, lang: Lang) -> Action {
        let mut action = Action::None;

        widgets::dim_background(ctx);

        egui::Window::new(t(self.title(), lang))
            .id(egui::Id::new("wgp_config_dialog"))
            .order(egui::Order::Foreground)
            .collapsible(false)
            .resizable(true)
            .default_size([680.0, 480.0])
            .min_size([520.0, 380.0])
            .anchor(egui::Align2::CENTER_CENTER, egui::vec2(0.0, 0.0))
            .show(ctx, |ui| {
                ui.horizontal(|ui| {
                    ui.label(t(Msg::InterfaceName, lang));
                    ui.add(
                        egui::TextEdit::singleline(&mut self.name)
                            .desired_width(ui.available_width() - 60.0),
                    );
                    ui.label(t(Msg::ConfSuffix, lang));
                });

                ui.add_space(4.0);

                ui.horizontal(|ui| {
                    ui.label(t(Msg::PublicKey, lang));
                    ui.add_enabled(
                        false,
                        egui::TextEdit::singleline(&mut self.public_key)
                            .font(egui::TextStyle::Monospace)
                            .desired_width(f32::INFINITY),
                    );
                });

                ui.add_space(6.0);
                ui.label(egui::RichText::new(t(Msg::Configuration, lang)).strong());

                let response = egui::ScrollArea::vertical()
                    .id_salt("wgp_dialog_config_view")
                    .auto_shrink([false, false])
                    .max_height(260.0)
                    .show(ui, |ui| {
                        ui.add(
                            egui::TextEdit::multiline(&mut self.content)
                                .code_editor()
                                .desired_width(f32::INFINITY)
                                .desired_rows(12),
                        )
                    })
                    .inner;

                if response.changed() {
                    self.error = None;
                    self.public_key.set(public_key_of_content(&self.content));
                }

                if let Some(error) = self.error {
                    ui.colored_label(egui::Color32::from_rgb(190, 30, 30), t(error, lang));
                }

                ui.add_space(4.0);
                ui.separator();

                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if ui.button(t(Msg::Ok, lang)).clicked() {
                        match self.validate() {
                            Ok(()) => action = Action::Accept,
                            Err(error) => self.error = Some(error),
                        }
                    }

                    if ui.button(t(Msg::Cancel, lang)).clicked() {
                        action = Action::Cancel;
                    }
                });
            });

        if ctx.input(|input| input.key_pressed(egui::Key::Escape)) {
            action = Action::Cancel;
        }

        action
    }

    fn validate(&self) -> Result<(), Msg> {
        if self.name.trim().is_empty() {
            return Err(Msg::EmptyName);
        }

        if self.content.trim().is_empty() {
            return Err(Msg::EmptyConfiguration);
        }

        config::validate(&self.content).map_err(validation_message)
    }
}

/// The public key of a configuration, only computed when a private key is
/// present: `wg pubkey` is a separate process, no need to start it for every
/// keystroke of an incomplete configuration.
fn public_key_of_content(content: &str) -> String {
    if !content.to_ascii_lowercase().contains("privatekey") {
        return String::new();
    }

    config::public_key_of(content)
}

/// Turns a validation error into the message displayed to the user.
pub fn validation_message(error: config::ValidationError) -> Msg {
    match error {
        config::ValidationError::NoInterfaceSection => Msg::ErrNoInterface,
        config::ValidationError::NoPrivateKey => Msg::ErrNoPrivateKey,
        config::ValidationError::PeerWithoutPublicKey => Msg::ErrPeerNoPublicKey,
    }
}
