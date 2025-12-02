use crate::common::buffer::Buffer;
use crate::common::constants::CRLF_BYTES;
use crate::common::error::{Result, ServerError};
use crate::http::headers::Headers;
use crate::http::method::Method;
use crate::http::request::Request;
use crate::http::version::Version;
use std::str;
use std::str::FromStr;

/// Parser state machine
#[derive(Debug, Clone, PartialEq)]
pub enum ParseState {
    RequestLine,
    Headers,
    Body,
    ChunkedBody,
    Complete,
    Error(String),
}

/// HTTP request parser with incremental parsing support
pub struct RequestParser {
    state: ParseState,
    buffer: Buffer,
    request: Option<Request>,
    expected_body_size: Option<usize>,
    header_lines: Vec<String>,
}

impl RequestParser {
    /// Create a new parser
    pub fn new() -> Self {
        Self {
            state: ParseState::RequestLine,
            buffer: Buffer::new(),
            request: None,
            expected_body_size: None,
            header_lines: Vec::new(),
        }
    }

    /// Add data to parser buffer
    pub fn add_data(&mut self, data: &[u8]) {
        self.buffer.extend(data);
    }

    /// Parse available data
    pub fn parse(&mut self) -> Result<Option<Request>> {
        loop {
            match &self.state {
                ParseState::RequestLine => {
                    if let Some(request) = self.parse_request_line()? {
                        self.request = Some(request);
                        self.state = ParseState::Headers;
                    } else {
                        return Ok(None); // Need more data
                    }
                }
                ParseState::Headers => {
                    if self.parse_headers()? {
                        self.state = ParseState::Body;
                        self.prepare_body_parsing();
                    } else {
                        return Ok(None); // Need more data
                    }
                }
                ParseState::Body => {
                    if self.parse_body()? {
                        self.state = ParseState::Complete;
                        if let Some(mut request) = self.request.take() {
                            request.parse_query_params();
                            return Ok(Some(request));
                        }
                    } else {
                        return Ok(None); // Need more data
                    }
                }
                ParseState::ChunkedBody => {
                    if self.parse_chunked_body()? {
                        self.state = ParseState::Complete;
                        if let Some(mut request) = self.request.take() {
                            request.parse_query_params();
                            return Ok(Some(request));
                        }
                    } else {
                        return Ok(None); // Need more data
                    }
                }
                ParseState::Complete => {
                    return Ok(None); // Already parsed
                }
                ParseState::Error(msg) => {
                    return Err(ServerError::ParseError(msg.clone()));
                }
            }
        }
    }

    /// Parse request line: "METHOD /path HTTP/1.1\r\n"
    fn parse_request_line(&mut self) -> Result<Option<Request>> {
        if let Some(crlf_pos) = self.buffer.find(CRLF_BYTES) {
            let line_bytes = self.buffer.drain(crlf_pos + CRLF_BYTES.len());
            let line = str::from_utf8(&line_bytes[..crlf_pos])
                .map_err(|e| ServerError::ParseError(format!("Invalid UTF-8 in request line: {}", e)))?;

            let parts: Vec<&str> = line.split_whitespace().collect();
            if parts.len() < 2 {
                return Err(ServerError::ParseError("Invalid request line format".to_string()));
            }

            let method = Method::from_str(parts[0])
                .map_err(|e| ServerError::ParseError(format!("Invalid method: {}", e)))?;

            let target = parts[1].to_string();

            let version = if parts.len() >= 3 {
                Version::from_str(parts[2])
                    .map_err(|e| ServerError::ParseError(format!("Invalid version: {}", e)))?
            } else {
                Version::Http11 // Default to HTTP/1.1
            };

            Ok(Some(Request::new(method, target, version)))
        } else {
            Ok(None) // Need more data
        }
    }

    /// Parse headers until empty line
    fn parse_headers(&mut self) -> Result<bool> {
        loop {
            if let Some(crlf_pos) = self.buffer.find(CRLF_BYTES) {
                let line_bytes = self.buffer.drain(crlf_pos + CRLF_BYTES.len());
                let line = str::from_utf8(&line_bytes[..crlf_pos])
                    .map_err(|e| ServerError::ParseError(format!("Invalid UTF-8 in header: {}", e)))?;

                // Empty line indicates end of headers
                if line.is_empty() {
                    return Ok(true);
                }

                self.header_lines.push(line.to_string());
            } else {
                return Ok(false); // Need more data
            }
        }
    }

    /// Prepare for body parsing
    fn prepare_body_parsing(&mut self) {
        if let Some(ref mut request) = self.request {
            // Parse headers
            if let Ok(headers) = Headers::from_lines(&self.header_lines) {
                request.headers = headers;
            }

            // Check for chunked encoding
            if request.is_chunked() {
                self.state = ParseState::ChunkedBody;
                return;
            }

            // Get Content-Length
            if let Some(length) = request.content_length() {
                self.expected_body_size = Some(length);
            } else if request.method.allows_body() {
                // No Content-Length and method allows body - assume no body
                self.expected_body_size = Some(0);
            } else {
                self.expected_body_size = Some(0);
            }
        }
    }

    /// Parse body with Content-Length
    fn parse_body(&mut self) -> Result<bool> {
        if let Some(expected_size) = self.expected_body_size {
            if let Some(ref mut request) = self.request {
                let available = self.buffer.len();
                if available >= expected_size {
                    let body = self.buffer.drain(expected_size);
                    request.body = body;
                    return Ok(true);
                }
            }
        }
        Ok(false) // Need more data
    }

    /// Parse chunked body
    fn parse_chunked_body(&mut self) -> Result<bool> {
        if let Some(ref mut request) = self.request {
            let mut body = Vec::new();

            loop {
                // Parse chunk size line
                if let Some(crlf_pos) = self.buffer.find(CRLF_BYTES) {
                    let line_bytes = self.buffer.drain(crlf_pos + CRLF_BYTES.len());
                    let line = str::from_utf8(&line_bytes[..crlf_pos])
                        .map_err(|e| ServerError::ParseError(format!("Invalid UTF-8 in chunk size: {}", e)))?;

                    // Parse chunk size (hex)
                    let chunk_size_str = line.split(';').next().unwrap_or(line).trim();
                    let chunk_size = usize::from_str_radix(chunk_size_str, 16)
                        .map_err(|_| ServerError::ParseError("Invalid chunk size".to_string()))?;

                    if chunk_size == 0 {
                        // Last chunk - read trailing CRLF
                        if self.buffer.len() >= CRLF_BYTES.len() {
                            self.buffer.drain(CRLF_BYTES.len());
                        }
                        request.body = body;
                        return Ok(true);
                    }

                    // Read chunk data
                    if self.buffer.len() >= chunk_size + CRLF_BYTES.len() {
                        let chunk_data = self.buffer.drain(chunk_size);
                        body.extend_from_slice(&chunk_data);
                        // Skip CRLF after chunk
                        self.buffer.drain(CRLF_BYTES.len());
                    } else {
                        // Need more data - restore what we drained
                        return Ok(false);
                    }
                } else {
                    return Ok(false); // Need more data
                }
            }
        }
        Ok(false)
    }

    /// Reset parser for new request
    pub fn reset(&mut self) {
        self.state = ParseState::RequestLine;
        self.buffer.clear();
        self.request = None;
        self.expected_body_size = None;
        self.header_lines.clear();
    }

    /// Check if parser is in error state
    pub fn is_error(&self) -> bool {
        matches!(self.state, ParseState::Error(_))
    }
}

impl Default for RequestParser {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_simple_request() {
        let request_str = "GET / HTTP/1.1\r\nHost: localhost\r\n\r\n";
        let mut parser = RequestParser::new();
        parser.add_data(request_str.as_bytes());
        let request = parser.parse().unwrap().unwrap();
        assert_eq!(request.method, Method::GET);
        assert_eq!(request.path(), "/");
    }
}
