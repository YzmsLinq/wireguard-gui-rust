//! Icons of the interface.
//!
//! The PNG files of the Python project are embedded in the binary, so the
//! executable can be moved anywhere without its resources.

use eframe::egui;

const APP_ICON: &[u8] = include_bytes!("../resources/icons/wireguard-icon.png");
const ACTIVATED_ICON: &[u8] = include_bytes!("../resources/icons/activated-icon.png");
const DEACTIVATED_ICON: &[u8] = include_bytes!("../resources/icons/deactivated-icon.png");
const ADD_ICON: &[u8] = include_bytes!("../resources/icons/add-icon.png");
const REMOVE_ICON: &[u8] = include_bytes!("../resources/icons/remove-icon.png");
const ARCHIVE_ICON: &[u8] = include_bytes!("../resources/icons/zip-icon.png");
const UP_ICON: &[u8] = include_bytes!("../resources/icons/up-icon.png");
const DOWN_ICON: &[u8] = include_bytes!("../resources/icons/down-icon.png");
const EDIT_ICON: &[u8] = include_bytes!("../resources/icons/edit-icon.png");

/// Every texture used by the interface.
pub struct Icons {
    pub app: egui::TextureHandle,
    pub activated: egui::TextureHandle,
    pub deactivated: egui::TextureHandle,
    pub add: egui::TextureHandle,
    pub remove: egui::TextureHandle,
    pub archive: egui::TextureHandle,
    pub up: egui::TextureHandle,
    pub down: egui::TextureHandle,
    pub edit: egui::TextureHandle,
}

impl Icons {
    pub fn load(ctx: &egui::Context) -> Self {
        Self {
            app: texture(ctx, "app", APP_ICON),
            activated: texture(ctx, "activated", ACTIVATED_ICON),
            deactivated: texture(ctx, "deactivated", DEACTIVATED_ICON),
            add: texture(ctx, "add", ADD_ICON),
            remove: texture(ctx, "remove", REMOVE_ICON),
            archive: texture(ctx, "archive", ARCHIVE_ICON),
            up: texture(ctx, "up", UP_ICON),
            down: texture(ctx, "down", DOWN_ICON),
            edit: texture(ctx, "edit", EDIT_ICON),
        }
    }
}

/// The window icon, as expected by the viewport builder.
pub fn window_icon() -> egui::IconData {
    let (rgba, width, height) = decode(APP_ICON);

    egui::IconData {
        rgba,
        width: width as u32,
        height: height as u32,
    }
}

fn decode(png: &[u8]) -> (Vec<u8>, usize, usize) {
    let image = image::load_from_memory_with_format(png, image::ImageFormat::Png)
        .expect("the icons embedded in the binary are valid PNG files")
        .to_rgba8();

    let (width, height) = image.dimensions();

    (image.into_raw(), width as usize, height as usize)
}

fn texture(ctx: &egui::Context, name: &str, png: &[u8]) -> egui::TextureHandle {
    let (rgba, width, height) = decode(png);
    let image = egui::ColorImage::from_rgba_unmultiplied([width, height], &rgba);

    ctx.load_texture(name, image, egui::TextureOptions::LINEAR)
}

/// Size of the icons shown next to the interface names.
pub const LIST_ICON_SIZE: f32 = 16.0;

/// Size of the icons shown in the toolbar.
pub const TOOLBAR_ICON_SIZE: f32 = 24.0;

/// Number of digits of the badge, drawn with a 5x7 bitmap font.
const DIGITS: [[u8; 7]; 10] = [
    [0b01110, 0b10001, 0b10001, 0b10001, 0b10001, 0b10001, 0b01110],
    [0b00100, 0b01100, 0b00100, 0b00100, 0b00100, 0b00100, 0b01110],
    [0b01110, 0b10001, 0b00001, 0b00010, 0b00100, 0b01000, 0b11111],
    [0b11111, 0b00010, 0b00100, 0b00010, 0b00001, 0b10001, 0b01110],
    [0b00010, 0b00110, 0b01010, 0b10010, 0b11111, 0b00010, 0b00010],
    [0b11111, 0b10000, 0b11110, 0b00001, 0b00001, 0b10001, 0b01110],
    [0b00110, 0b01000, 0b10000, 0b11110, 0b10001, 0b10001, 0b01110],
    [0b11111, 0b00001, 0b00010, 0b00100, 0b01000, 0b01000, 0b01000],
    [0b01110, 0b10001, 0b10001, 0b01110, 0b10001, 0b10001, 0b01110],
    [0b01110, 0b10001, 0b10001, 0b01111, 0b00001, 0b00010, 0b01100],
];

/// The tray icon, with a red badge showing how many interfaces are up.
///
/// The pixels are returned in the ARGB32 network byte order expected by the
/// status notifier protocol, along with the size of the icon.
pub fn tray_pixmap(active_count: usize) -> (Vec<u8>, i32, i32) {
    const SIZE: u32 = 32;

    let mut pixels = image::load_from_memory_with_format(APP_ICON, image::ImageFormat::Png)
        .map(|image| image.to_rgba8())
        .map(|image| image::imageops::resize(&image, SIZE, SIZE, image::imageops::FilterType::Lanczos3))
        .map(|image| image.into_raw())
        .unwrap_or_else(|_| vec![0u8; (SIZE * SIZE * 4) as usize]);

    if active_count > 0 {
        draw_badge(&mut pixels, SIZE as usize, active_count);
    }

    let mut argb = Vec::with_capacity(pixels.len());

    for pixel in pixels.chunks_exact(4) {
        argb.extend_from_slice(&[pixel[3], pixel[0], pixel[1], pixel[2]]);
    }

    (argb, SIZE as i32, SIZE as i32)
}

fn draw_badge(pixels: &mut [u8], size: usize, count: usize) {
    const BADGE_RADIUS: f32 = 8.5;
    const CENTER: f32 = 22.0;

    let red = [214u8, 40, 40, 255];
    let white = [255u8, 255, 255, 255];

    for y in 0..size {
        for x in 0..size {
            let distance = (((x as f32 + 0.5) - CENTER).powi(2)
                + ((y as f32 + 0.5) - CENTER).powi(2))
            .sqrt();

            if distance > BADGE_RADIUS {
                continue;
            }

            let color = if distance > BADGE_RADIUS - 1.4 {
                white
            } else {
                red
            };

            set_pixel(pixels, size, x, y, color);
        }
    }

    let text = if count > 9 {
        "9+".to_owned()
    } else {
        count.to_string()
    };

    let characters: Vec<u8> = text
        .bytes()
        .filter(|byte| byte.is_ascii_digit())
        .map(|byte| byte - b'0')
        .collect();

    let width = characters.len() * 6 - 1;
    let origin_x = CENTER as isize - (width as isize) / 2;
    let origin_y = CENTER as isize - 3;

    for (index, digit) in characters.iter().enumerate() {
        let x = origin_x + (index as isize) * 6;

        for (row, bits) in DIGITS[*digit as usize].iter().enumerate() {
            for column in 0..5 {
                if bits & (1 << (4 - column)) != 0 {
                    let px = x + column as isize;
                    let py = origin_y + row as isize;

                    if px >= 0 && py >= 0 && (px as usize) < size && (py as usize) < size {
                        set_pixel(pixels, size, px as usize, py as usize, white);
                    }
                }
            }
        }
    }
}

fn set_pixel(pixels: &mut [u8], size: usize, x: usize, y: usize, color: [u8; 4]) {
    let offset = (y * size + x) * 4;

    if let Some(pixel) = pixels.get_mut(offset..offset + 4) {
        pixel.copy_from_slice(&color);
    }
}
