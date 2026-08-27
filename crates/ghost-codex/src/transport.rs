use std::ffi::OsStr;
use std::io::{BufRead, BufReader, Write};
use std::path::Path;
#[cfg(target_os = "windows")]
use std::path::PathBuf;
use std::process::{Child, ChildStdin, ChildStdout, Command, Stdio};
use std::sync::{Arc, Mutex};

use serde_json::Value;

use crate::AgentError;

/// Split stdio ownership for the concurrent App Server runtime. Exactly one reader thread owns
/// stdout while any number of caller threads may serialize writes through `stdin`.
pub(crate) struct SplitStdioTransport {
    pub stdin: Arc<Mutex<ChildStdin>>,
    pub stdout: BufReader<ChildStdout>,
    pub child: Arc<Mutex<Child>>,
}

impl SplitStdioTransport {
    pub fn spawn(binary: &Path) -> Result<Self, AgentError> {
        let mut command = codex_command(binary);
        let mut child = command
            .args(["app-server", "--listen", "stdio://"])
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            // App Server stderr contains its internal tracing stream, including ERROR-level
            // diagnostics for recoverable events such as a search command returning no matches.
            // Ghost consumes operational failures through JSON-RPC instead of presenting those
            // implementation logs as application errors. Developers can opt back into the raw
            // stream with GHOST_CODEX_STDERR=inherit.
            .stderr(codex_stderr())
            .spawn()?;
        let stdin = child
            .stdin
            .take()
            .ok_or_else(|| AgentError::Protocol("Codex stdin unavailable".into()))?;
        let stdout = child
            .stdout
            .take()
            .ok_or_else(|| AgentError::Protocol("Codex stdout unavailable".into()))?;
        Ok(Self {
            stdin: Arc::new(Mutex::new(stdin)),
            stdout: BufReader::new(stdout),
            child: Arc::new(Mutex::new(child)),
        })
    }
}

fn codex_stderr() -> Stdio {
    if inherit_codex_stderr(std::env::var_os("GHOST_CODEX_STDERR").as_deref()) {
        Stdio::inherit()
    } else {
        Stdio::null()
    }
}

fn inherit_codex_stderr(value: Option<&OsStr>) -> bool {
    value.is_some_and(|value| {
        matches!(
            value.to_string_lossy().trim().to_ascii_lowercase().as_str(),
            "1" | "true" | "inherit"
        )
    })
}

pub(crate) fn write_stdio_message(
    stdin: &Arc<Mutex<ChildStdin>>,
    message: &Value,
) -> Result<(), AgentError> {
    let mut stdin = stdin
        .lock()
        .map_err(|_| AgentError::Protocol("Codex stdin lock poisoned".into()))?;
    serde_json::to_writer(&mut *stdin, message)?;
    stdin.write_all(b"\n")?;
    stdin.flush()?;
    Ok(())
}

pub(crate) fn read_stdio_message(stdout: &mut BufReader<ChildStdout>) -> Result<Value, AgentError> {
    let mut line = String::new();
    let read = stdout.read_line(&mut line)?;
    if read == 0 {
        return Err(AgentError::Protocol("Codex closed stdout".into()));
    }
    Ok(serde_json::from_str(&line)?)
}

fn codex_command(binary: &Path) -> Command {
    #[cfg(target_os = "windows")]
    {
        if let Some(shim) = windows_command_shim(binary) {
            let shell = std::env::var_os("COMSPEC").unwrap_or_else(|| "cmd.exe".into());
            let mut command = Command::new(shell);
            command.arg("/D").arg("/C").arg(shim);
            return command;
        }
    }

    Command::new(binary)
}

#[cfg(target_os = "windows")]
fn windows_command_shim(binary: &Path) -> Option<PathBuf> {
    if is_windows_command_shim(binary) {
        return Some(binary.to_path_buf());
    }
    let cmd = binary.with_extension("cmd");
    cmd.is_file().then_some(cmd)
}

#[cfg(target_os = "windows")]
fn is_windows_command_shim(path: &Path) -> bool {
    path.extension().is_some_and(|extension| {
        extension.eq_ignore_ascii_case("cmd") || extension.eq_ignore_ascii_case("bat")
    })
}

#[cfg(all(test, target_os = "windows"))]
mod tests {
    use super::*;

    #[test]
    fn recognizes_windows_command_shims() {
        assert!(is_windows_command_shim(Path::new(r"C:\tools\codex.cmd")));
        assert!(is_windows_command_shim(Path::new(r"C:\tools\codex.BAT")));
        assert!(!is_windows_command_shim(Path::new(r"C:\tools\codex.exe")));
        assert!(!is_windows_command_shim(Path::new(r"C:\tools\codex")));
    }

    #[test]
    fn raw_codex_stderr_is_opt_in() {
        assert!(!inherit_codex_stderr(None));
        assert!(!inherit_codex_stderr(Some(OsStr::new("off"))));
        assert!(inherit_codex_stderr(Some(OsStr::new("inherit"))));
        assert!(inherit_codex_stderr(Some(OsStr::new("TRUE"))));
    }
}
