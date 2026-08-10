use std::io::{BufRead, BufReader, Write};
use std::path::{Path, PathBuf};
use std::process::{Child, ChildStdin, ChildStdout, Command, Stdio};

use serde_json::Value;

use crate::AgentError;

/// Replaceable app-server wire boundary. Tests use deterministic scripted transports.
pub trait RpcTransport: Send {
    fn send(&mut self, message: &Value) -> Result<(), AgentError>;
    fn receive(&mut self) -> Result<Value, AgentError>;
    fn shutdown(&mut self);
}

pub struct StdioTransport {
    child: Child,
    stdin: ChildStdin,
    stdout: BufReader<ChildStdout>,
}

impl StdioTransport {
    pub fn spawn(binary: &Path) -> Result<Self, AgentError> {
        let mut command = codex_command(binary);
        let mut child = command
            .args(["app-server", "--listen", "stdio://"])
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::inherit())
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
            child,
            stdin,
            stdout: BufReader::new(stdout),
        })
    }
}

fn codex_command(binary: &Path) -> Command {
    #[cfg(target_os = "windows")]
    {
        if let Some(shim) = windows_command_shim(binary) {
            // npm/global Windows installs commonly expose `codex.cmd` (and an extensionless
            // POSIX shim) rather than a native PE executable. CreateProcess cannot execute a
            // .cmd file directly and reports ERROR_BAD_EXE_FORMAT / os error 193, so run the
            // command shim through the user's command processor while preserving stdio.
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

    // `where codex` can return the extensionless npm POSIX shim before `codex.cmd`.
    // Prefer the sibling .cmd file when it exists instead of handing the shell script to
    // CreateProcess and getting os error 193.
    let cmd = binary.with_extension("cmd");
    cmd.is_file().then_some(cmd)
}

#[cfg(target_os = "windows")]
fn is_windows_command_shim(path: &Path) -> bool {
    path.extension().is_some_and(|extension| {
        extension.eq_ignore_ascii_case("cmd") || extension.eq_ignore_ascii_case("bat")
    })
}

impl RpcTransport for StdioTransport {
    fn send(&mut self, message: &Value) -> Result<(), AgentError> {
        serde_json::to_writer(&mut self.stdin, message)?;
        self.stdin.write_all(b"\n")?;
        self.stdin.flush()?;
        Ok(())
    }

    fn receive(&mut self) -> Result<Value, AgentError> {
        let mut line = String::new();
        let read = self.stdout.read_line(&mut line)?;
        if read == 0 {
            return Err(AgentError::Protocol("Codex closed stdout".into()));
        }
        Ok(serde_json::from_str(&line)?)
    }

    fn shutdown(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
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
}
