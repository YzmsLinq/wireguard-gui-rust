//! Minimal INI persistence, the counterpart of the `QSettings` calls of the
//! Python version.
//!
//! Only a couple of values are stored, so a very small INI reader/writer is
//! enough: the file keeps its unknown lines, which makes it safe to hand-edit.

use std::fs;
use std::path::PathBuf;

/// Directory and file name, inside `~/.config`.
pub const DIRECTORY: &str = "wireguard-gui-rust";
pub const FILE: &str = "wgr.ini";

/// Path of the settings file, `None` when the home directory is unknown.
pub fn path() -> Option<PathBuf> {
    dirs::config_dir().map(|directory| directory.join(DIRECTORY).join(FILE))
}

/// Reads every `section` / `key = value` pair of the file.
fn load_lines() -> Vec<String> {
    path()
        .and_then(|path| fs::read_to_string(path).ok())
        .map(|content| content.lines().map(str::to_owned).collect())
        .unwrap_or_default()
}

/// Returns the value stored for `key` in `section`.
pub fn get(section: &str, key: &str) -> Option<String> {
    let mut current_section = String::new();

    for line in load_lines() {
        let trimmed = line.trim();

        if let Some(rest) = trimmed.strip_prefix('[') {
            current_section = rest
                .split_once(']')
                .map(|(name, _)| name.trim().to_ascii_lowercase())
                .unwrap_or_default();
            continue;
        }

        if current_section != section.to_ascii_lowercase() {
            continue;
        }

        if let Some((name, value)) = trimmed.split_once('=') {
            if name.trim().eq_ignore_ascii_case(key) {
                return Some(value.trim().to_owned());
            }
        }
    }

    None
}

/// Stores `value` for `key` in `section`, keeping the other values.
pub fn set(section: &str, key: &str, value: &str) {
    let Some(path) = path() else {
        return;
    };

    if let Some(parent) = path.parent() {
        let _ = fs::create_dir_all(parent);
    }

    let mut lines = load_lines();
    let mut current_section = String::new();
    let mut section_line: Option<usize> = None;
    let mut replaced = false;

    for index in 0..lines.len() {
        let trimmed = lines[index].trim().to_owned();

        if let Some(rest) = trimmed.strip_prefix('[') {
            current_section = rest
                .split_once(']')
                .map(|(name, _)| name.trim().to_ascii_lowercase())
                .unwrap_or_default();

            if current_section == section.to_ascii_lowercase() {
                section_line = Some(index);
            }

            continue;
        }

        if current_section != section.to_ascii_lowercase() {
            continue;
        }

        if let Some((name, _)) = trimmed.split_once('=') {
            if name.trim().eq_ignore_ascii_case(key) {
                lines[index] = format!("{key}={value}");
                replaced = true;
            }
        }
    }

    if !replaced {
        match section_line {
            Some(_) => {
                // Insert right after the section header of the section.
                let position = section_line.map(|index| index + 1).unwrap_or(lines.len());
                lines.insert(position, format!("{key}={value}"));
            }
            None => {
                if !lines.is_empty() && !lines.last().map(|line| line.is_empty()).unwrap_or(true) {
                    lines.push(String::new());
                }
                lines.push(format!("[{section}]"));
                lines.push(format!("{key}={value}"));
            }
        }
    }

    let mut content = lines.join("\n");
    content.push('\n');

    let _ = fs::write(path, content);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn settings_path_lives_in_the_config_directory() {
        assert!(path().is_some());
        assert!(path().unwrap().ends_with(format!("{DIRECTORY}/{FILE}")));
    }
}
