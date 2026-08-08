use std::io::{BufRead, BufReader, Write};
use std::path::Path;
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
        let mut child = Command::new(binary)
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
