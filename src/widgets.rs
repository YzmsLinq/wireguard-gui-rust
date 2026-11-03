//! Small widgets shared by the windows.

use std::ops::Range;

use eframe::egui;

/// A text buffer that can be selected and copied, but not modified.
///
/// `egui` has no read-only text edit, so the mutations are simply ignored:
/// the cursor, the selection and the shortcuts (Ctrl+C) keep working.
#[derive(Debug, Default, Clone)]
pub struct ReadOnlyText(String);

impl ReadOnlyText {
    pub fn new(text: impl Into<String>) -> Self {
        Self(text.into())
    }

    pub fn set(&mut self, text: impl Into<String>) {
        self.0 = text.into();
    }

    pub fn text(&self) -> &str {
        &self.0
    }
}

impl AsRef<str> for ReadOnlyText {
    fn as_ref(&self) -> &str {
        &self.0
    }
}

impl egui::TextBuffer for ReadOnlyText {
    fn is_mutable(&self) -> bool {
        false
    }

    fn as_str(&self) -> &str {
        &self.0
    }

    fn insert_text(&mut self, _text: &str, _char_index: usize) -> usize {
        0
    }

    fn delete_char_range(&mut self, _char_range: Range<usize>) {}
}

/// A monospaced, read-only text area used to display a configuration.
pub fn code_view(ui: &mut egui::Ui, text: &mut ReadOnlyText, rows: usize) {
    ui.add(
        egui::TextEdit::multiline(text)
            .code_editor()
            .desired_width(f32::INFINITY)
            .desired_rows(rows),
    );
}

/// Darkens the window and swallows the clicks, to make a dialog modal.
pub fn dim_background(ctx: &egui::Context) {
    let screen = ctx.screen_rect();

    egui::Area::new(egui::Id::new("wgp_modal_dim"))
        .order(egui::Order::Foreground)
        .fixed_pos(screen.min)
        .show(ctx, |ui| {
            ui.allocate_response(screen.size(), egui::Sense::click_and_drag());
            ui.painter()
                .rect_filled(screen, 0.0, egui::Color32::from_black_alpha(45));
        });
}

/// A framed box with a bold title, the equivalent of a `QGroupBox`.
pub fn group_box<R>(
    ui: &mut egui::Ui,
    title: &str,
    height: f32,
    contents: impl FnOnce(&mut egui::Ui) -> R,
) -> R {
    egui::Frame::group(ui.style())
        .inner_margin(egui::Margin::same(8.0))
        .show(ui, |ui| {
            ui.set_width(ui.available_width());
            ui.set_height(height);
            ui.label(egui::RichText::new(title).strong());
            ui.add_space(4.0);
            contents(ui)
        })
        .inner
}
