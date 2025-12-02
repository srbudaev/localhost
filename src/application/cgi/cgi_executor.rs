use crate::application::cgi::cgi_env::CgiEnvironment;
use crate::application::cgi::cgi_io::CgiIo;
use crate::application::cgi::cgi_process::CgiProcess;
use crate::common::error::{Result, ServerError};
use crate::http::request::Request;
use crate::http::response::Response;
use std::path::PathBuf;

/// Executes CGI scripts and returns HTTP responses
pub struct CgiExecutor {
    /// Maximum execution time for CGI scripts (in seconds)
    #[allow(dead_code)] // Will be used for timeout implementation
    timeout_secs: u64,
}

impl CgiExecutor {
    /// Create a new CGI executor
    pub fn new(timeout_secs: u64) -> Self {
        Self { timeout_secs }
    }

    /// Execute a CGI script and return HTTP response
    pub fn execute(
        &self,
        script_path: PathBuf,
        interpreter: Option<&str>,
        request: &Request,
        server_name: &str,
        server_port: u16,
    ) -> Result<Response> {
        // Verify script exists and is readable
        if !script_path.exists() {
            return Err(ServerError::CgiError(format!(
                "CGI script not found: {}",
                script_path.display()
            )));
        }

        if !script_path.is_file() {
            return Err(ServerError::CgiError(format!(
                "CGI script path is not a file: {}",
                script_path.display()
            )));
        }

        // Build environment variables
        let env_vars = CgiEnvironment::build(request, &script_path, server_name, server_port);

        // Get request body if present
        let body_data = if !request.body.is_empty() {
            Some(request.body.as_slice())
        } else {
            None
        };

        // Spawn CGI process
        let mut process = CgiProcess::spawn(script_path.clone(), interpreter, &env_vars, body_data)?;

        // Write request body to stdin if present
        if let Some(body) = body_data {
            CgiIo::write_stdin(process.child_mut(), body)?;
        }

        // Close stdin to signal end of input
        drop(process.child_mut().stdin.take());

        // Wait for process
        // Note: In a production system, this should use async waiting with proper timeout handling
        // For now, we use a simple blocking wait
        let exit_code = process.wait()?;

        // Check exit code
        if exit_code != 0 {
            // Read stderr for error information
            let stderr = CgiIo::read_stderr(process.child_mut())?;
            return Err(ServerError::CgiError(format!(
                "CGI script '{}' exited with code {}: {}",
                script_path.display(),
                exit_code,
                stderr
            )));
        }

        // Read and parse response from stdout
        let response = CgiIo::read_stdout(process.child_mut())?;

        Ok(response)
    }
}
