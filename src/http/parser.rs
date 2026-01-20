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
    max_body_size: usize,
    current_body_size: usize,
}

impl RequestParser {
    /// Create a new parser with default max body size
    pub fn new() -> Self {
        Self::with_max_body_size(crate::common::constants::DEFAULT_MAX_BODY_SIZE)
    }

    /// Create a new parser with specified max body size
    pub fn with_max_body_size(max_body_size: usize) -> Self {
        Self {
            state: ParseState::RequestLine,
            buffer: Buffer::new(),
            request: None,
            expected_body_size: None,
            header_lines: Vec::new(),
            max_body_size,
            current_body_size: 0,
        }
    }

    /// Check if body size exceeds limit and return error if so (helper to reduce redundancy)
    fn check_body_size_limit(&self, size: usize) -> Result<()> {
        if size > self.max_body_size {
            return Err(ServerError::HttpError(format!(
                "Request body size {} exceeds maximum allowed size {}",
                size, self.max_body_size
            )));
        }
        Ok(())
    }

    /// Check if adding additional data would exceed body size limit (helper to reduce redundancy)
    fn check_would_exceed_limit(&self, current_size: usize, additional_size: usize) -> Result<()> {
        let total_size = current_size + additional_size;
        if total_size > self.max_body_size {
            return Err(ServerError::HttpError(format!(
                "Request body size would exceed maximum allowed size {}",
                self.max_body_size
            )));
        }
        Ok(())
    }

    /// Check current body size against limit (helper to reduce redundancy)
    fn check_current_body_size(&self, current_size: usize) -> Result<()> {
        if current_size > self.max_body_size {
            return Err(ServerError::HttpError(format!(
                "Request body size {} exceeds maximum allowed size {}",
                current_size, self.max_body_size
            )));
        }
        Ok(())
    }

    /// Add data to parser buffer
    pub fn add_data(&mut self, data: &[u8]) -> Result<()> {
        // Check if adding this data would exceed max body size
        // Once we're in Body or ChunkedBody state, headers have been drained,
        // so buffer only contains body data
        let _total_body_size = if matches!(self.state, ParseState::Body | ParseState::ChunkedBody) {
            // In body parsing state, buffer contains only body data (headers were drained)
            // Check current body size + buffer + new data
            self.current_body_size + self.buffer.len() + data.len()
        } else {
            // Before body parsing, we need to be more careful
            // Headers are still in buffer, so we can't accurately measure body size yet
            // But if total buffer is way too large, it's likely a problem
            // Use a more lenient check: allow buffer up to max_body_size + reasonable header size (8KB)
            let max_header_size = 8192;
            if self.buffer.len() + data.len() > self.max_body_size + max_header_size {
                return Err(ServerError::HttpError(format!(
                    "Request body size would exceed maximum allowed size {}",
                    self.max_body_size
                )));
            }
            // Not in body state yet, so we can't determine if it's a body size error
            // Let it through and check later when we parse body
            self.buffer.extend(data);
            return Ok(());
        };
        
        // Check if adding this data would exceed limit (only in body state)
        if matches!(self.state, ParseState::Body | ParseState::ChunkedBody) {
            self.check_would_exceed_limit(self.current_body_size, self.buffer.len() + data.len())?;
        }
        self.buffer.extend(data);
        Ok(())
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
                        self.prepare_body_parsing()?;
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
    fn prepare_body_parsing(&mut self) -> Result<()> {
        if let Some(ref mut request) = self.request {
            // Parse headers
            if let Ok(headers) = Headers::from_lines(&self.header_lines) {
                request.headers = headers;
            }

            // Check for chunked encoding
            if request.is_chunked() {
                self.state = ParseState::ChunkedBody;
                return Ok(());
            }

            // Get Content-Length
            if let Some(length) = request.content_length() {
                // Check if body size exceeds limit
                self.check_body_size_limit(length)?;
                self.expected_body_size = Some(length);
            } else if request.method.allows_body() {
                // No Content-Length and method allows body
                // For POST/PUT/PATCH, we need to handle body data that comes after headers
                // We'll check body size as data arrives (handled in add_data and parse_body)
                // Set to None to indicate we need to read until connection closes or buffer limit
                // But limit the total buffer size to max_body_size
                self.expected_body_size = None;
            } else {
                self.expected_body_size = Some(0);
            }
        }
        Ok(())
    }

    /// Parse body with Content-Length or without (for POST/PUT/PATCH)
    fn parse_body(&mut self) -> Result<bool> {
        // Store values before mutable borrow to avoid conflicts
        let available = self.buffer.len();
        let current_size = self.current_body_size;
        let expected_size_opt = self.expected_body_size;
        
        match expected_size_opt {
            Some(expected_size) => {
                // Content-Length specified - parse exact amount
                // Check if we're exceeding max body size
                self.check_would_exceed_limit(current_size, available)?;
                
                if available >= expected_size {
                    let body = self.buffer.drain(expected_size);
                    self.current_body_size += body.len();
                    let new_size = self.current_body_size; // Store to avoid borrow conflict
                    
                    // Final check
                    self.check_current_body_size(new_size)?;
                    
                    // Now we can safely borrow request mutably
                    if let Some(ref mut request) = self.request {
                        request.body = body;
                    }
                    return Ok(true);
                }
            }
            None => {
                // No Content-Length - for POST/PUT/PATCH, read until buffer is empty or limit reached
                // Check if we're exceeding max body size
                self.check_would_exceed_limit(current_size, available)?;
                
                // For requests without Content-Length, we read all available data
                // This is a simplified approach - in production, you might want to limit
                // by connection timeout or other means
                if available > 0 {
                    let body = self.buffer.drain(available);
                    self.current_body_size += body.len();
                    let new_size = self.current_body_size; // Store to avoid borrow conflict
                    
                    // Final check
                    self.check_current_body_size(new_size)?;
                    
                    // Now we can safely borrow request mutably
                    if let Some(ref mut request) = self.request {
                        request.body = body;
                    }
                    return Ok(true);
                }
                // No data yet - need more data
            }
        }
        Ok(false) // Need more data
    }

    /// Parse chunked body
    fn parse_chunked_body(&mut self) -> Result<bool> {
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

                // Store current_size before mutable operations to avoid borrow conflicts
                let current_size = self.current_body_size;

                if chunk_size == 0 {
                    // Last chunk - read trailing CRLF
                    if self.buffer.len() >= CRLF_BYTES.len() {
                        self.buffer.drain(CRLF_BYTES.len());
                    }
                    
                    // Final check for max body size
                    self.check_current_body_size(current_size)?;
                    
                    // Now we can safely borrow request mutably
                    if let Some(ref mut request) = self.request {
                        request.body = body;
                    }
                    return Ok(true);
                }

                // Check if adding this chunk would exceed max body size
                self.check_would_exceed_limit(current_size, chunk_size)?;

                // Read chunk data
                if self.buffer.len() >= chunk_size + CRLF_BYTES.len() {
                    let chunk_data = self.buffer.drain(chunk_size);
                    self.current_body_size += chunk_data.len();
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

    /// Reset parser for new request
    pub fn reset(&mut self) {
        self.state = ParseState::RequestLine;
        self.buffer.clear();
        self.request = None;
        self.expected_body_size = None;
        self.header_lines.clear();
        self.current_body_size = 0;
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
        parser.add_data(request_str.as_bytes()).unwrap();
        let request = parser.parse().unwrap().unwrap();
        assert_eq!(request.method, Method::GET);
        assert_eq!(request.path(), "/");
    }
}
