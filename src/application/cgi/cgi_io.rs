use crate::common::error::{Result, ServerError};
use crate::common::constants::CRLF_BYTES;
use crate::http::headers::Headers;
use crate::http::response::Response;
use crate::http::status::StatusCode;
use crate::http::version::Version;
use std::io::{Read, Write};
use std::process::Child;

/// Handle CGI script I/O
pub struct CgiIo;

impl CgiIo {
    /// Write request body to CGI process stdin
    pub fn write_stdin(child: &mut Child, data: &[u8]) -> Result<()> {
        if let Some(ref mut stdin) = child.stdin {
            stdin.write_all(data)
                .map_err(|e| ServerError::CgiError(format!("Failed to write to CGI stdin: {}", e)))?;
            stdin.flush()
                .map_err(|e| ServerError::CgiError(format!("Failed to flush CGI stdin: {}", e)))?;
        }
        Ok(())
    }

    /// Read CGI process stdout and parse response
    pub fn read_stdout(child: &mut Child) -> Result<Response> {
        let mut output = Vec::new();
        
        if let Some(ref mut stdout) = child.stdout {
            stdout.read_to_end(&mut output)
                .map_err(|e| ServerError::CgiError(format!("Failed to read CGI stdout: {}", e)))?;
        }

        Self::parse_cgi_output(&output)
    }

    /// Read CGI process stderr
    pub fn read_stderr(child: &mut Child) -> Result<String> {
        let mut error = String::new();
        
        if let Some(ref mut stderr) = child.stderr {
            std::io::Read::read_to_string(stderr, &mut error)
                .map_err(|e| ServerError::CgiError(format!("Failed to read CGI stderr: {}", e)))?;
        }

        Ok(error)
    }

    /// Parse CGI script output according to CGI/1.1 specification
    /// CGI scripts output headers followed by blank line, then body
    fn parse_cgi_output(output: &[u8]) -> Result<Response> {
        // Find double CRLF (end of headers)
        // Look for pattern: CRLF CRLF
        let crlf_len = CRLF_BYTES.len();
        let mut header_end = None;
        for i in 0..output.len().saturating_sub(crlf_len * 2) {
            if &output[i..i + crlf_len] == CRLF_BYTES 
                && &output[i + crlf_len..i + crlf_len * 2] == CRLF_BYTES {
                header_end = Some(i);
                break;
            }
        }
        
        let header_end = header_end
            .ok_or_else(|| ServerError::CgiError("CGI output missing header separator".to_string()))?;

        // Parse headers
        let header_bytes = &output[..header_end];
        let header_lines: Vec<String> = String::from_utf8_lossy(header_bytes)
            .lines()
            .map(|s| s.to_string())
            .collect();

        let headers = Headers::from_lines(&header_lines)
            .map_err(|e| ServerError::CgiError(format!("Failed to parse CGI headers: {:?}", e)))?;

        // Extract body (skip header separator: CRLF CRLF)
        let body_start = header_end + CRLF_BYTES.len() * 2;
        let body = if body_start < output.len() {
            output[body_start..].to_vec()
        } else {
            Vec::new()
        };

        // Determine status code from Status header or default to 200
        let status = if let Some(status_header) = headers.get("Status") {
            Self::parse_status_header(status_header)?
        } else {
            StatusCode::OK
        };

        // Determine version (default to HTTP/1.1)
        let version = Version::Http11;

        // Build response
        let mut response = Response::new(version, status);
        response.headers = headers;
        response.body = body;

        Ok(response)
    }

    /// Parse Status header (format: "200 OK" or just "200")
    fn parse_status_header(status_str: &str) -> Result<StatusCode> {
        let parts: Vec<&str> = status_str.trim().split_whitespace().collect();
        if parts.is_empty() {
            return Err(ServerError::CgiError("Empty Status header".to_string()));
        }

        let code: u16 = parts[0]
            .parse()
            .map_err(|_| ServerError::CgiError(format!("Invalid status code in Status header: {}", parts[0])))?;

        StatusCode::new(code)
            .ok_or_else(|| ServerError::CgiError(format!("Invalid HTTP status code: {}", code)))
    }
}
