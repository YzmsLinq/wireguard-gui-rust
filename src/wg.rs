//! Every operation performed on the WireGuard interfaces.
//!
//! This is the Rust counterpart of `utility/functions.py`: the interfaces are
//! `*.conf` files of `/etc/wireguard`, and they are started and stopped with
//! `wg-quick@<name>.service` (falling back to `wg-quick up/down` when the
//! interface was not started through systemd).

use std::fs::{self, File, Permissions};
use std::io::{self, copy};
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

use crate::config::{self, ConfigParts};

/// Where WireGuard stores its configuration files.
pub const CONFIGS_DIR: &str = "/etc/wireguard";

/// Name of the zip file created when exporting the interfaces.
pub const EXPORT_FILE_NAME: &str = "wireguard_interfaces.zip";

/// Longest interface name accepted, in characters (`wg-quick` limit).
pub const MAX_INTERFACE_NAME: usize = 15;

/// Everything that can go wrong while handling an interface.
#[derive(Debug)]
pub enum Error {
    Validation(config::ValidationError),
    Io(io::Error),
    Zip(zip::result::ZipError),
    Message(String),
}

impl std::fmt::Display for Error {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Error::Validation(_) => write!(formatter, "invalid configuration"),
            Error::Io(error) => write!(formatter, "{error}"),
            Error::Zip(error) => write!(formatter, "{error}"),
            Error::Message(message) => write!(formatter, "{message}"),
        }
    }
}

impl From<io::Error> for Error {
    fn from(error: io::Error) -> Self {
        Error::Io(error)
    }
}

impl From<zip::result::ZipError> for Error {
    fn from(error: zip::result::ZipError) -> Self {
        Error::Zip(error)
    }
}

impl From<config::ValidationError> for Error {
    fn from(error: config::ValidationError) -> Self {
        Error::Validation(error)
    }
}

pub type Result<T> = std::result::Result<T, Error>;

/// Directory holding the configuration files.
///
/// `WGP_CONFIGS_DIR` is only here to try the application without touching
/// `/etc/wireguard`; it is not meant to be used in normal operation.
fn configs_dir() -> PathBuf {
    std::env::var_os("WGP_CONFIGS_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from(CONFIGS_DIR))
}

/// The `*.conf` files of `/etc/wireguard`, sorted by name.
pub fn interfaces() -> Vec<String> {
    let mut files: Vec<String> = match fs::read_dir(configs_dir()) {
        Ok(entries) => entries
            .flatten()
            .filter_map(|entry| entry.file_name().into_string().ok())
            .filter(|name| !name.starts_with('.') && name.ends_with(".conf"))
            .collect(),
        Err(_) => Vec::new(),
    };

    files.sort();
    files
}

/// `wg0.conf` -> `wg0`
pub fn interface_name(file: &str) -> &str {
    file.strip_suffix(".conf").unwrap_or(file)
}

/// Keeps an interface name usable as a file name and as an interface name.
pub fn sanitize_name(name: &str) -> String {
    name.trim()
        .chars()
        .take(MAX_INTERFACE_NAME)
        .filter(|character| {
            character.is_ascii_alphanumeric() || matches!(character, '_' | '-' | '.' | '=')
        })
        .collect()
}

fn status(program: &str, arguments: &[&str]) -> bool {
    Command::new(program)
        .args(arguments)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .map(|status| status.success())
        .unwrap_or(false)
}

fn spawn(program: &str, arguments: &[&str]) {
    let _ = Command::new(program)
        .args(arguments)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn();
}

fn unit(file: &str) -> String {
    format!("wg-quick@{}.service", interface_name(file))
}

/// Whether the interface is currently up, according to `wg show`.
pub fn is_active(file: &str) -> bool {
    status("wg", &["show", interface_name(file)])
}

/// Whether `wg-quick@<name>.service` is active.
pub fn is_systemd_active(file: &str) -> bool {
    status("systemctl", &["is-active", &unit(file)])
}

/// Whether `wg-quick@<name>.service` is enabled at boot.
pub fn is_systemd_enabled(file: &str) -> bool {
    status("systemctl", &["is-enabled", &unit(file)])
}

/// Brings the interface up, returns whether it is up afterwards.
pub fn start_interface(file: &str) -> bool {
    status("systemctl", &["start", &unit(file)]);

    let active = is_active(file);

    if active {
        spawn("systemctl", &["enable", &unit(file)]);
    }

    active
}

/// Brings the interface down, and disables it when it was enabled.
pub fn stop_interface(file: &str) {
    if is_systemd_active(file) {
        status("systemctl", &["stop", &unit(file)]);

        if is_systemd_enabled(file) {
            spawn("systemctl", &["disable", &unit(file)]);
        }
    } else {
        status("wg-quick", &["down", interface_name(file)]);
    }
}

/// Restarts the interface, returns whether it is up afterwards.
pub fn restart_interface(file: &str) -> bool {
    if is_systemd_active(file) {
        status("systemctl", &["restart", &unit(file)]);
        is_active(file)
    } else {
        status("wg-quick", &["down", interface_name(file)]);
        start_interface(file)
    }
}

/// Activates the interface when it is down, deactivates it when it is up.
pub fn turn_interface(file: &str) {
    if is_active(file) {
        stop_interface(file);
    } else {
        start_interface(file);
    }
}

/// The content of a configuration file.
pub fn read_config(file: &str) -> Result<String> {
    Ok(fs::read_to_string(configs_dir().join(file))?)
}

/// The configuration of an interface split in its two parts.
pub fn config_parts(file: &str) -> Result<ConfigParts> {
    Ok(config::split(&read_config(file)?))
}

/// The live configuration of an interface, split in its two parts.
pub fn active_config_parts(file: &str) -> Result<ConfigParts> {
    let output = Command::new("wg")
        .args(["show", interface_name(file)])
        .stdin(Stdio::null())
        .stderr(Stdio::null())
        .output()?;

    Ok(config::split_active(&String::from_utf8_lossy(&output.stdout)))
}

/// Overwrites a configuration file, keeping it readable by root only.
pub fn write_config(file: &str, content: &str) -> Result<()> {
    let path = configs_dir().join(file);

    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }

    fs::write(&path, content)?;
    fs::set_permissions(&path, Permissions::from_mode(0o600))?;

    Ok(())
}

/// Deletes a configuration file, deactivating the interface first.
pub fn delete_interface(file: &str) -> Result<()> {
    if is_active(file) {
        stop_interface(file);
    }

    fs::remove_file(configs_dir().join(file))?;

    Ok(())
}

/// Copies a configuration file into `/etc/wireguard`.
pub fn add_interface(source: &Path) -> Result<String> {
    let name = source
        .file_name()
        .and_then(|name| name.to_str())
        .ok_or_else(|| Error::Message("the file has no name".to_owned()))?
        .to_owned();

    if !name.ends_with(".conf") {
        return Err(Error::Message("the file is not a .conf file".to_owned()));
    }

    let content = fs::read_to_string(source)?;
    config::validate(&content)?;
    write_config(&name, &content)?;

    Ok(name)
}

/// Creates a new interface, removing it when its configuration is invalid.
pub fn new_interface(name: &str, content: &str) -> Result<String> {
    let name = sanitize_name(name);

    if name.is_empty() {
        return Err(Error::Message("the interface name is empty".to_owned()));
    }

    let file = format!("{name}.conf");

    write_config(&file, content)?;

    if let Err(error) = config::validate(content) {
        let _ = fs::remove_file(configs_dir().join(&file));
        return Err(Error::Validation(error));
    }

    Ok(file)
}

/// Saves the changes made in the edit dialog.
///
/// When the new configuration is invalid the previous one is restored, and
/// when the interface is renamed the file is moved and the interface restarted
/// under its new name.
pub fn edit_interface(file: &str, new_name: &str, old_content: &str, new_content: &str) -> Result<()> {
    let was_active = is_active(file);
    let changed = old_content != new_content;

    if changed {
        write_config(file, new_content)?;

        if let Err(error) = config::validate(new_content) {
            // Roll back to the configuration that was working.
            write_config(file, old_content)?;
            return Err(Error::Validation(error));
        }
    }

    let new_name = sanitize_name(new_name);
    let current_name = interface_name(file).to_owned();

    if !new_name.is_empty() && new_name != current_name {
        let target = format!("{new_name}.conf");
        fs::rename(configs_dir().join(file), configs_dir().join(&target))?;

        if was_active {
            stop_interface(file);
            start_interface(&target);
        }
    } else if was_active && changed {
        restart_interface(file);
    }

    Ok(())
}

/// Copies every `*.conf` file of `/etc/wireguard` to a zip file.
pub fn export_interfaces(directory: &Path) -> Result<PathBuf> {
    let archive_path = directory.join(EXPORT_FILE_NAME);
    let file = File::create(&archive_path)?;
    let mut archive = zip::ZipWriter::new(file);
    let options = zip::write::SimpleFileOptions::default()
        .compression_method(zip::CompressionMethod::Deflated)
        .unix_permissions(0o600);

    let mut configs = Vec::new();
    collect_configs(&configs_dir(), &mut configs)?;

    if configs.is_empty() {
        return Err(Error::Message("there is no interface to export".to_owned()));
    }

    for (path, name) in configs {
        archive.start_file(name, options)?;
        let mut source = File::open(path)?;
        copy(&mut source, &mut archive)?;
    }

    archive.finish()?;

    Ok(archive_path)
}

fn collect_configs(directory: &Path, found: &mut Vec<(PathBuf, String)>) -> Result<()> {
    let entries = match fs::read_dir(directory) {
        Ok(entries) => entries,
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(()),
        Err(error) => return Err(Error::Io(error)),
    };

    for entry in entries.flatten() {
        let path = entry.path();

        if path.is_dir() {
            collect_configs(&path, found)?;
        } else if path.extension().and_then(|extension| extension.to_str()) == Some("conf") {
            if let Some(name) = path.file_name().and_then(|name| name.to_str()) {
                found.push((path.clone(), name.to_owned()));
            }
        }
    }

    Ok(())
}

/// Extracts the `*.conf` files of a zip archive into `/etc/wireguard`.
pub fn import_interfaces(archive_path: &Path) -> Result<usize> {
    let file = File::open(archive_path)?;
    let mut archive = zip::ZipArchive::new(file)?;
    let mut imported = 0usize;

    for index in 0..archive.len() {
        let mut entry = archive.by_index(index)?;

        if entry.is_dir() {
            continue;
        }

        // Only the file name is kept: an archive must not be able to write
        // outside of the configuration directory.
        let Some(name) = Path::new(entry.name())
            .file_name()
            .and_then(|name| name.to_str())
            .map(str::to_owned)
        else {
            continue;
        };

        if !name.ends_with(".conf") {
            continue;
        }

        let target = configs_dir().join(&name);
        let mut output = File::create(&target)?;
        copy(&mut entry, &mut output)?;
        fs::set_permissions(&target, Permissions::from_mode(0o600))?;
        imported += 1;
    }

    if imported == 0 {
        return Err(Error::Message(
            "the archive does not contain any .conf file".to_owned(),
        ));
    }

    Ok(imported)
}
