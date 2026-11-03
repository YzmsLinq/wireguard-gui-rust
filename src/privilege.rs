//! Root privileges.
//!
//! `wgr` manages `/etc/wireguard` and the `wg-quick@*.service` units, so it has
//! to run as root. Instead of relying on an external launcher script, the
//! application checks its own effective user id and, when it is not root,
//! re-executes itself through `pkexec` (falling back to `sudo`).
//!
//! Re-executing keeps the old process alive and waiting: it is the only way to
//! know whether the elevation was granted. When it is refused the process stays
//! around with [`Elevation::Failed`] and the interface can report the reason.

use std::os::unix::process::ExitStatusExt;
use std::path::{Path, PathBuf};
use std::process::{Command, ExitStatus, Stdio};

/// The variable that marks the process started through `pkexec`.
const PKEXEC_UID: &str = "PKEXEC_UID";

/// A command that runs a process as root.
struct Elevator {
    program: &'static str,
    arguments: Vec<String>,
}

/// The outcome of the privilege check.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Elevation {
    /// Nothing to do: the process is already root.
    Root,
    /// The application was started again as root, by `program`.
    Relaunched(&'static str),
    /// The application could not become root.
    Failed(Failure),
}

/// Why the application could not get root privileges.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Failure {
    /// Neither `pkexec` nor `sudo` is installed.
    NoTool,
    /// The elevation prompt was dismissed or the password was wrong.
    Cancelled,
    /// The authentification or the new process failed.
    Error(String),
}

/// The path of the running executable, usable to start it again.
pub fn self_path() -> Option<PathBuf> {
    let path = std::env::current_exe().ok()?;

    // `/proc/self/exe` is the right answer, but it points to the resolved path
    // of the file: it is only usable while that file still exists.
    path.exists().then_some(path)
}

/// Effective user id of the process.
fn is_root() -> bool {
    // SAFETY: `geteuid` has no preconditions and cannot fail.
    unsafe { libc::geteuid() == 0 }
}

/// Makes the process run as root, starting it again when needed.
pub fn ensure_root() -> Elevation {
    if is_root() {
        return Elevation::Root;
    }

    let Some(path) = self_path() else {
        return Elevation::Failed(Failure::Error(
            "the path of the executable cannot be determined".to_owned(),
        ));
    };

    let (program, status) = match launch(&path) {
        Some(result) => result,
        None => return Elevation::Failed(Failure::NoTool),
    };

    match status.code() {
        Some(0) => Elevation::Relaunched(program),
        _ => Elevation::Failed(describe(program, &status)),
    }
}

/// Runs the executable again as root, through `pkexec` then `sudo`.
///
/// Returns the tool used and the status of the attempt; `None` when no
/// elevation tool is available.
fn launch(path: &Path) -> Option<(&'static str, ExitStatus)> {
    let current_dir = std::env::current_dir().ok();

    for elevator in elevators(path) {
        // Skip the tools that are not actually usable.
        if result_of(elevator.program).is_none() {
            continue;
        }

        let mut command = Command::new(elevator.program);
        command
            .args(&elevator.arguments)
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            // `pkexec` exports `PKEXEC_UID` to the process it starts; here it
            // would describe a process that is about to be replaced.
            .env_remove(PKEXEC_UID);

        if let Some(directory) = &current_dir {
            command.current_dir(directory);
        }

        if let Ok(status) = command.status() {
            return Some((elevator.program, status));
        }
    }

    None
}

/// The tools that can elevate, in order of preference.
fn elevators(path: &Path) -> Vec<Elevator> {
    let mut candidates = Vec::new();
    let executable = path.display().to_string();

    if let Some(pkexec) = find_in_path("pkexec") {
        // `pkexec` resets the environment; the graphical session variables
        // have to be given to `env` explicitly, otherwise the new process
        // cannot open a window.
        let mut arguments = vec![pkexec];
        let session = session_variables();

        if !session.is_empty() {
            arguments.push(env_program());
            arguments.extend(session);
        }

        arguments.push(executable.clone());

        candidates.push(Elevator {
            program: "pkexec",
            arguments,
        });
    }

    // `sudo` keeps the environment of the calling user, so it needs no help;
    // `--` keeps the path from being read as an option.
    if find_in_path("sudo").is_some() {
        candidates.push(Elevator {
            program: "sudo",
            arguments: vec!["--".to_owned(), executable],
        });
    }

    candidates
}

/// Looks for an executable in `PATH`, then in the usual system directories.
///
/// The `PATH` of a desktop session is not the one of a shell: it is often
/// reduced to `/usr/local/bin`, so the system directories are always tried.
fn find_in_path(program: &str) -> Option<String> {
    let path = std::env::var_os("PATH").unwrap_or_default();
    let mut directories: Vec<PathBuf> = std::env::split_paths(&path).collect();

    for directory in ["/usr/local/bin", "/usr/bin", "/bin", "/usr/sbin", "/sbin"] {
        let directory = PathBuf::from(directory);
        if !directories.contains(&directory) {
            directories.push(directory);
        }
    }

    directories
        .into_iter()
        .map(|directory| directory.join(program))
        .find(|candidate| is_executable(candidate))
        .map(|candidate| candidate.display().to_string())
}

fn is_executable(path: &Path) -> bool {
    use std::os::unix::fs::PermissionsExt;

    std::fs::metadata(path)
        .map(|metadata| metadata.is_file() && metadata.permissions().mode() & 0o111 != 0)
        .unwrap_or(false)
}

/// Turns the exit code of an elevation tool into a reason.
fn describe(program: &'static str, status: &ExitStatus) -> Failure {
    let code = status.code().unwrap_or(-1);

    // The process died from a signal: killed before it could say anything.
    if status.signal().is_some() {
        return Failure::Cancelled;
    }

    match (program, code) {
        // pkexec: 126 = the dialog was dismissed, or the user is not allowed to
        // elevate, 127 = pkexec refused to run the program.
        ("pkexec", 126 | 127) => Failure::Cancelled,
        // sudo: 1 = wrong password, or the user is not a sudoer.
        ("sudo", 1) => Failure::Cancelled,
        _ => Failure::Error(format!("{program} exited with status {code}")),
    }
}

/// `NAME=value` pairs of the graphical session, if there is one.
fn session_variables() -> Vec<String> {
    session_variables_from(|name| std::env::var(name).ok().filter(|value| !value.is_empty()))
}

/// Same as [`session_variables`], reading the variables through `get`.
///
/// `pkexec` starts the application with an empty environment, and without
/// these variables the new process cannot reach the display: they have to be
/// restored explicitly.
fn session_variables_from(get: impl Fn(&str) -> Option<String>) -> Vec<String> {
    let display = get("DISPLAY");
    let wayland = get("WAYLAND_DISPLAY");

    if display.is_none() && wayland.is_none() {
        return Vec::new();
    }

    let xauthority = get("XAUTHORITY").or_else(|| {
        std::env::var_os("HOME")
            .map(|home| PathBuf::from(home).join(".Xauthority"))
            .filter(|path| path.exists())
            .map(|path| path.display().to_string())
    });

    [
        ("DISPLAY", display),
        ("XAUTHORITY", xauthority),
        ("WAYLAND_DISPLAY", wayland),
        ("XDG_RUNTIME_DIR", get("XDG_RUNTIME_DIR")),
        ("XDG_SESSION_TYPE", get("XDG_SESSION_TYPE")),
    ]
    .into_iter()
    .filter_map(|(name, value)| value.map(|value| format!("{name}={value}")))
    .collect()
}

/// `env` is only useful when it belongs to the system, not to a shell.
fn env_program() -> String {
    find_in_path("env").unwrap_or_else(|| "env".to_owned())
}

/// Exit code of a command, `None` when it cannot be run.
fn result_of(program: &str) -> Option<i32> {
    Command::new(program)
        .arg("--version")
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .ok()
        .and_then(|status| status.code())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn status(code: i32) -> ExitStatus {
        ExitStatus::from_raw(code << 8)
    }

    #[test]
    fn finds_tools_in_path() {
        assert!(find_in_path("sh").is_some());
        assert!(find_in_path("wgr-does-not-exist").is_none());
    }

    #[test]
    fn reads_the_session_variables() {
        let session = session_variables_from(|name| match name {
            "DISPLAY" => Some(":0".to_owned()),
            "WAYLAND_DISPLAY" => Some("wayland-1".to_owned()),
            "XDG_RUNTIME_DIR" => Some("/run/user/1000".to_owned()),
            // An empty value must not be passed on.
            _ => Some(String::new()).filter(|value| !value.is_empty()),
        });

        assert!(session.contains(&"DISPLAY=:0".to_owned()));
        assert!(session.contains(&"WAYLAND_DISPLAY=wayland-1".to_owned()));
        assert!(session.contains(&"XDG_RUNTIME_DIR=/run/user/1000".to_owned()));

        // Without a display server, `env` is useless.
        assert!(session_variables_from(|_| None).is_empty());
    }

    #[test]
    fn builds_the_elevation_commands() {
        let elevators = elevators(Path::new("/usr/local/bin/wgr"));

        for elevator in &elevators {
            // The program to start as root is the last argument, whatever
            // wraps it (`env` for pkexec, nothing for sudo).
            assert_eq!(elevator.arguments.last().unwrap(), "/usr/local/bin/wgr");

            // A path with a space must stay a single argument, and must not be
            // read as an option by the tool.
            if elevator.program == "sudo" {
                assert_eq!(elevator.arguments[0], "--");
            }
        }
    }

    #[test]
    fn explains_the_failure() {
        assert_eq!(describe("pkexec", &status(126)), Failure::Cancelled);
        assert_eq!(describe("sudo", &status(1)), Failure::Cancelled);
        assert_eq!(describe("pkexec", &status(127)), Failure::Cancelled);
        assert!(matches!(describe("pkexec", &status(2)), Failure::Error(_)));
    }

    #[test]
    fn only_adds_tools_that_exist() {
        for elevator in elevators(Path::new("/usr/bin/wgr")) {
            assert!(matches!(elevator.program, "pkexec" | "sudo"));
            assert!(find_in_path(elevator.program).is_some());
        }
    }
}

