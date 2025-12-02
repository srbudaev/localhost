use crate::common::error::{Result, ServerError};
use std::collections::HashMap;
use std::path::PathBuf;
use std::process::{Child, Command, Stdio};

/// Represents a running CGI process
pub struct CgiProcess {
    child: Child,
    script_path: PathBuf,
}

impl CgiProcess {
    /// Spawn a new CGI process
    pub fn spawn(
        script_path: PathBuf,
        interpreter: Option<&str>,
        env_vars: &HashMap<String, String>,
        stdin_data: Option<&[u8]>,
    ) -> Result<Self> {
        // Determine command and arguments
        let (cmd, args) = if let Some(interpreter) = interpreter {
            // Use specified interpreter
            (interpreter.to_string(), vec![script_path.to_string_lossy().to_string()])
        } else {
            // Execute script directly (must be executable)
            (script_path.to_string_lossy().to_string(), Vec::new())
        };

        // Build command
        let mut command = Command::new(&cmd);
        command.args(&args);
        
        // Set environment variables
        command.env_clear(); // Clear existing environment for security
        for (key, value) in env_vars {
            command.env(key, value);
        }

        // Set up stdin/stdout/stderr
        if stdin_data.is_some() {
            command.stdin(Stdio::piped());
        } else {
            command.stdin(Stdio::null());
        }
        command.stdout(Stdio::piped());
        command.stderr(Stdio::piped());

        // Set working directory to script's directory
        if let Some(parent) = script_path.parent() {
            command.current_dir(parent);
        }

        // Spawn process
        let child = command
            .spawn()
            .map_err(|e| {
                ServerError::CgiError(format!(
                    "Failed to spawn CGI process for '{}': {}",
                    script_path.display(),
                    e
                ))
            })?;

        Ok(Self {
            child,
            script_path,
        })
    }

    /// Get mutable reference to child process
    pub fn child_mut(&mut self) -> &mut Child {
        &mut self.child
    }

    /// Get script path
    pub fn script_path(&self) -> &PathBuf {
        &self.script_path
    }

    /// Wait for process to complete and get exit status
    pub fn wait(&mut self) -> Result<i32> {
        self.child
            .wait()
            .map_err(|e| {
                ServerError::CgiError(format!(
                    "Failed to wait for CGI process '{}': {}",
                    self.script_path.display(),
                    e
                ))
            })
            .map(|status| status.code().unwrap_or(-1))
    }

    /// Kill the process if it's still running
    pub fn kill(&mut self) -> Result<()> {
        if let Err(e) = self.child.kill() {
            // Process might have already finished
            if e.kind() != std::io::ErrorKind::InvalidInput {
                return Err(ServerError::CgiError(format!(
                    "Failed to kill CGI process '{}': {}",
                    self.script_path.display(),
                    e
                )));
            }
        }
        Ok(())
    }
}

impl Drop for CgiProcess {
    fn drop(&mut self) {
        // Try to kill process if still running
        let _ = self.kill();
        // Wait for process to finish
        let _ = self.child.wait();
    }
}
