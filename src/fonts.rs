//! Fonts and global style of the interface.
//!
//! `egui` only ships Latin fonts, so a system font able to display CJK glyphs
//! is looked for at startup. When none is found the interface still works, the
//! Chinese strings are just not rendered correctly.

use std::sync::atomic::{AtomicBool, Ordering};

use eframe::egui;

static CJK_FONT_AVAILABLE: AtomicBool = AtomicBool::new(false);

/// Fonts able to display the Chinese interface, tried in order.
const CJK_FONT_CANDIDATES: &[&str] = &[
    "/usr/share/fonts/opentype/source-han-cjk/SourceHanSansSC-Regular.otf",
    "/usr/share/fonts/opentype/source-han-sans/SourceHanSansSC-Regular.otf",
    "/usr/share/fonts/truetype/source-han-sans/SourceHanSansSC-Regular.otf",
    "/usr/share/fonts/opentype/noto/NotoSansSC-Regular.otf",
    "/usr/share/fonts/truetype/noto/NotoSansSC-Regular.otf",
    "/usr/share/fonts/truetype/droid/DroidSansFallbackFull.ttf",
    "/usr/share/fonts/truetype/arphic/uming.ttf",
    "/usr/share/fonts/win/msyh.ttc",
];

/// File names of the fonts that are accepted when scanning the font
/// directories, and that are a single font (not a collection).
const CJK_FONT_NAME_HINTS: &[&str] = &[
    "sourcehansans",
    "sourcehanserif",
    "notosanssc",
    "notoserifsc",
    "droidsansfallback",
    "wqy-microhei",
    "wqy-zenhei",
    "microsoftyahei",
    "msyh",
];

/// Whether a font able to display Chinese was found.
pub fn cjk_available() -> bool {
    CJK_FONT_AVAILABLE.load(Ordering::Relaxed)
}

/// Installs the fonts and the style of the application.
pub fn install(ctx: &egui::Context) {
    let mut fonts = egui::FontDefinitions::default();

    if let Some(data) = load_cjk_font() {
        CJK_FONT_AVAILABLE.store(true, Ordering::Relaxed);

        const CJK_FONT: &str = "cjk";
        fonts
            .font_data
            .insert(CJK_FONT.to_owned(), egui::FontData::from_owned(data));

        for family in [egui::FontFamily::Proportional, egui::FontFamily::Monospace] {
            fonts
                .families
                .entry(family)
                .or_default()
                .push(CJK_FONT.to_owned());
        }
    }

    ctx.set_fonts(fonts);
    install_style(ctx);
}

fn load_cjk_font() -> Option<Vec<u8>> {
    for candidate in CJK_FONT_CANDIDATES {
        if let Ok(data) = std::fs::read(candidate) {
            if !data.is_empty() {
                return Some(data);
            }
        }
    }

    for directory in ["/usr/share/fonts", "/usr/local/share/fonts"] {
        if let Some(data) = scan_font_directory(std::path::Path::new(directory), 0) {
            return Some(data);
        }
    }

    fontconfig_match()
}

/// Looks for a single font file whose name hints at a CJK font.
fn scan_font_directory(directory: &std::path::Path, depth: usize) -> Option<Vec<u8>> {
    if depth > 4 {
        return None;
    }

    let entries = std::fs::read_dir(directory).ok()?;

    for entry in entries.flatten() {
        let path = entry.path();

        if path.is_dir() {
            if let Some(data) = scan_font_directory(&path, depth + 1) {
                return Some(data);
            }
            continue;
        }

        let extension = path
            .extension()
            .and_then(|extension| extension.to_str())
            .unwrap_or("")
            .to_ascii_lowercase();

        if extension != "otf" && extension != "ttf" {
            continue;
        }

        let name = path
            .file_name()
            .and_then(|name| name.to_str())
            .unwrap_or("")
            .to_ascii_lowercase()
            .replace([' ', '-', '_'], "");

        if CJK_FONT_NAME_HINTS.iter().any(|hint| name.contains(hint)) {
            if let Ok(data) = std::fs::read(&path) {
                if !data.is_empty() {
                    return Some(data);
                }
            }
        }
    }

    None
}

/// Last resort: ask fontconfig for a font supporting Simplified Chinese.
fn fontconfig_match() -> Option<Vec<u8>> {
    let output = std::process::Command::new("fc-match")
        .args(["--format", "%{file}", "sans-serif:lang=zh-cn"])
        .output()
        .ok()?;

    let path = String::from_utf8_lossy(&output.stdout).trim().to_owned();

    if path.is_empty() || path.to_ascii_lowercase().ends_with(".ttc") {
        return None;
    }

    std::fs::read(path).ok().filter(|data| !data.is_empty())
}

fn install_style(ctx: &egui::Context) {
    let mut style = (*ctx.style()).clone();

    style.visuals = egui::Visuals::light();
    style.visuals.window_rounding = 6.0.into();
    style.visuals.menu_rounding = 6.0.into();
    style.spacing.item_spacing = egui::vec2(8.0, 6.0);
    style.spacing.button_padding = egui::vec2(8.0, 4.0);
    style.spacing.window_margin = egui::Margin::same(10.0);

    style.text_styles.insert(
        egui::TextStyle::Body,
        egui::FontId::proportional(14.0),
    );
    style.text_styles.insert(
        egui::TextStyle::Button,
        egui::FontId::proportional(14.0),
    );
    style.text_styles.insert(
        egui::TextStyle::Monospace,
        egui::FontId::monospace(13.0),
    );

    ctx.set_style(style);
}
