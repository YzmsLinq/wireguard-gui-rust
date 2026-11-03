//! Parsing and validation of WireGuard configuration files.
//!
//! This module replaces the `wgconfig` dependency used by the Python version:
//! it only needs to split a configuration in its `[Interface]` / `[Peer]`
//! parts and tell whether the content is a usable WireGuard configuration.

use std::io::Write;
use std::process::{Command, Stdio};

/// The three views of a configuration that the main window displays.
#[derive(Debug, Clone, Default)]
pub struct ConfigParts {
    /// The whole configuration, as stored on disk (or printed by `wg show`).
    #[allow(dead_code)]
    pub full: String,
    /// Everything before the first `[Peer]` section.
    pub interface: String,
    /// The first `[Peer]` section and everything after it.
    pub peers: String,
}

/// Splits a configuration file in its `[Interface]` and `[Peer]` parts.
pub fn split(content: &str) -> ConfigParts {
    match section_offset(content, "peer") {
        Some(index) => ConfigParts {
            full: content.to_owned(),
            interface: content[..index].trim().to_owned(),
            peers: content[index..].trim().to_owned(),
        },
        None => ConfigParts {
            full: content.to_owned(),
            interface: content.trim().to_owned(),
            peers: String::new(),
        },
    }
}

/// Splits the output of `wg show <iface>` in its interface and peer parts.
pub fn split_active(content: &str) -> ConfigParts {
    match content
        .split_inclusive('\n')
        .scan(0usize, |offset, line| {
            let start = *offset;
            *offset += line.len();
            Some((start, line))
        })
        .find(|(_, line)| line.trim_start().to_ascii_lowercase().starts_with("peer:"))
        .map(|(start, _)| start)
    {
        Some(index) => ConfigParts {
            full: content.to_owned(),
            interface: content[..index].trim().to_owned(),
            peers: content[index..].trim().to_owned(),
        },
        None => ConfigParts {
            full: content.to_owned(),
            interface: content.trim().to_owned(),
            peers: String::new(),
        },
    }
}

/// Returns the byte offset at which a `[section]` header starts.
fn section_offset(content: &str, section: &str) -> Option<usize> {
    let mut offset = 0;

    for line in content.split_inclusive('\n') {
        let trimmed = line.trim();

        if let Some(rest) = trimmed.strip_prefix('[') {
            if let Some((name, _)) = rest.split_once(']') {
                if name.trim().eq_ignore_ascii_case(section) {
                    return Some(offset);
                }
            }
        }

        offset += line.len();
    }

    None
}

/// The label used by `wg show` for a configuration key
/// (`privatekey` -> `private key`).
fn active_label(key: &str) -> String {
    match key.to_ascii_lowercase().as_str() {
        "privatekey" => "private key".to_owned(),
        "publickey" => "public key".to_owned(),
        "presharedkey" => "preshared key".to_owned(),
        "listenport" => "listening port".to_owned(),
        "allowedips" => "allowed ips".to_owned(),
        "persistentkeepalive" => "persistent keepalive".to_owned(),
        "lastendpoint" => "endpoint".to_owned(),
        other => other.to_owned(),
    }
}

/// Reads the value of `key` inside a configuration file (`active == false`)
/// or inside the output of `wg show` (`active == true`).
pub fn field(content: &str, key: &str, active: bool) -> String {
    let label = if active {
        active_label(key)
    } else {
        key.to_ascii_lowercase()
    };

    let separator = if active { ':' } else { '=' };

    for line in content.lines() {
        let trimmed = line.trim();

        if trimmed.is_empty() || trimmed.starts_with('#') || trimmed.starts_with(';') {
            continue;
        }

        let lowered = trimmed.to_ascii_lowercase();

        // `wg show` prints `peer: <key>` lines: a peer must not be mistaken
        // for an interface level setting.
        if lowered.starts_with("peer:") && key.eq_ignore_ascii_case("publickey") && active {
            continue;
        }

        if lowered.starts_with(&label) {
            if let Some(position) = trimmed.find(separator) {
                return trimmed[position + separator.len_utf8()..].trim().to_owned();
            }
        }
    }

    String::new()
}

/// The public key matching a private key, computed with `wg pubkey`.
pub fn derive_public_key(private_key: &str) -> Option<String> {
    let private_key = private_key.trim();

    if private_key.is_empty() {
        return None;
    }

    let mut child = Command::new("wg")
        .arg("pubkey")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .ok()?;

    if let Some(mut stdin) = child.stdin.take() {
        stdin.write_all(private_key.as_bytes()).ok()?;
    }

    let output = child.wait_with_output().ok()?;

    if !output.status.success() {
        return None;
    }

    let public_key = String::from_utf8_lossy(&output.stdout).trim().to_owned();

    if public_key.is_empty() {
        None
    } else {
        Some(public_key)
    }
}

/// The public key of the interface described by a configuration, derived from
/// its `PrivateKey` when there is one.
pub fn public_key_of(content: &str) -> String {
    let private_key = field(content, "privatekey", false);

    if private_key.is_empty() {
        String::new()
    } else {
        derive_public_key(&private_key).unwrap_or_default()
    }
}

/// Why a configuration cannot be used.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ValidationError {
    NoInterfaceSection,
    NoPrivateKey,
    PeerWithoutPublicKey,
}

/// Checks that a configuration is a usable WireGuard configuration.
///
/// A configuration is accepted when it has an `[Interface]` section with a
/// `PrivateKey`, and when every `[Peer]` section has a `PublicKey`. This is
/// the same rule the Python version applied through `wgconfig`.
pub fn validate(content: &str) -> Result<(), ValidationError> {
    let mut in_interface = false;
    let mut seen_interface = false;
    let mut in_peer = false;
    let mut interface_private_key = false;
    let mut peer_public_key = false;

    for line in content.lines() {
        let trimmed = line.trim();

        if trimmed.is_empty() || trimmed.starts_with('#') || trimmed.starts_with(';') {
            continue;
        }

        if let Some(rest) = trimmed.strip_prefix('[') {
            let name = rest
                .split_once(']')
                .map(|(name, _)| name.trim())
                .unwrap_or("");

            if in_peer && !peer_public_key {
                return Err(ValidationError::PeerWithoutPublicKey);
            }

            if name.eq_ignore_ascii_case("interface") {
                seen_interface = true;
                in_interface = true;
                in_peer = false;
            } else if name.eq_ignore_ascii_case("peer") {
                in_interface = false;
                in_peer = true;
                peer_public_key = false;
            } else {
                in_interface = false;
                in_peer = false;
            }

            continue;
        }

        let Some((key, value)) = trimmed.split_once('=') else {
            continue;
        };

        let key = key.trim();
        let value = value.trim();

        if value.is_empty() {
            continue;
        }

        if key.eq_ignore_ascii_case("privatekey") {
            if in_interface {
                interface_private_key = true;
            }
        } else if key.eq_ignore_ascii_case("publickey") && in_peer {
            peer_public_key = true;
        }
    }

    if in_peer && !peer_public_key {
        return Err(ValidationError::PeerWithoutPublicKey);
    }

    if !seen_interface {
        return Err(ValidationError::NoInterfaceSection);
    }

    if !interface_private_key {
        return Err(ValidationError::NoPrivateKey);
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    const CONFIG: &str = "[Interface]\nAddress = 10.0.0.2/32\nPrivateKey = aGVsbG8=\n\n[Peer]\nPublicKey = d29ybGQ=\nEndpoint = 1.2.3.4:51820\n";

    #[test]
    fn splits_interface_and_peers() {
        let parts = split(CONFIG);
        assert!(parts.interface.starts_with("[Interface]"));
        assert!(!parts.interface.contains("[Peer]"));
        assert!(parts.peers.starts_with("[Peer]"));
        assert_eq!(parts.full.len(), CONFIG.len());
    }

    #[test]
    fn reads_field_values() {
        assert_eq!(field(CONFIG, "PrivateKey", false), "aGVsbG8=");
        assert_eq!(field(CONFIG, "endpoint", false), "1.2.3.4:51820");
        assert_eq!(field(CONFIG, "missing", false), "");
    }

    #[test]
    fn reads_active_field_values() {
        let show = "interface: wg0\n  public key: abc=\n  private key: def=\n\npeer: ghi=\n  endpoint: 1.2.3.4:51820\n";
        assert_eq!(field(show, "publickey", true), "abc=");
        assert_eq!(field(show, "privatekey", true), "def=");
        assert_eq!(field(show, "allowedips", true), "");
    }

    #[test]
    fn validates_configurations() {
        assert_eq!(validate(CONFIG), Ok(()));
        assert_eq!(validate("Address = 10.0.0.2/32\n"), Err(ValidationError::NoInterfaceSection));
        assert_eq!(validate("[Interface]\nAddress = 1.2.3.4/32\n"), Err(ValidationError::NoPrivateKey));
        assert_eq!(
            validate("[Interface]\nPrivateKey = aGVsbG8=\n[Peer]\nEndpoint = 1.2.3.4:5\n"),
            Err(ValidationError::PeerWithoutPublicKey)
        );
    }
}
