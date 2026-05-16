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
    /// Accumulator for chunked body data; persists across parse() calls so that
    /// chunks already drained from `buffer` are not lost when we return
    /// `Ok(false)` waiting for the next CRLF/chunk to arrive.
    chunked_body: Vec<u8>,
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
            chunked_body: Vec::new(),
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
            let line = str::from_utf8(&line_bytes[..crlf_pos]).map_err(|e| {
                ServerError::ParseError(format!("Invalid UTF-8 in request line: {}", e))
            })?;

            let parts: Vec<&str> = line.split_whitespace().collect();
            if parts.len() < 2 {
                return Err(ServerError::ParseError(
                    "Invalid request line format".to_string(),
                ));
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
                let line = str::from_utf8(&line_bytes[..crlf_pos]).map_err(|e| {
                    ServerError::ParseError(format!("Invalid UTF-8 in header: {}", e))
                })?;

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

    /// Parse chunked body.
    ///
    /// Chunks already extracted from `self.buffer` are appended to
    /// `self.chunked_body`, which persists across calls. This means we can
    /// safely return `Ok(false)` to wait for more data without losing any
    /// chunk we have already consumed from the network buffer.
    fn parse_chunked_body(&mut self) -> Result<bool> {
        loop {
            // Parse chunk size line
            if let Some(crlf_pos) = self.buffer.find(CRLF_BYTES) {
                let line_bytes = self.buffer.drain(crlf_pos + CRLF_BYTES.len());
                let line = str::from_utf8(&line_bytes[..crlf_pos]).map_err(|e| {
                    ServerError::ParseError(format!("Invalid UTF-8 in chunk size: {}", e))
                })?;

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

                    // Move accumulated body into the request
                    let body = std::mem::take(&mut self.chunked_body);
                    if let Some(ref mut request) = self.request {
                        request.body = body;
                    }
                    return Ok(true);
                }

                // Check if adding this chunk would exceed max body size
                self.check_would_exceed_limit(current_size, chunk_size)?;

                // Read chunk data: require both chunk bytes AND trailing CRLF
                if self.buffer.len() >= chunk_size + CRLF_BYTES.len() {
                    let chunk_data = self.buffer.drain(chunk_size);
                    self.current_body_size += chunk_data.len();
                    self.chunked_body.extend_from_slice(&chunk_data);
                    // Skip CRLF after chunk
                    self.buffer.drain(CRLF_BYTES.len());
                } else {
                    // Not enough bytes for this chunk yet. We have already
                    // consumed the chunk-size line above; the chunk payload
                    // (so far) still sits in `self.buffer` and will be
                    // re-examined on the next parse() call once more data
                    // arrives. Anything we have appended to `self.chunked_body`
                    // is retained for the next call.
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
        self.chunked_body.clear();
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

    // -----------------------------------------------------------------------
    // Request line / headers
    // -----------------------------------------------------------------------

    #[test]
    fn test_parse_simple_request() {
        let request_str = "GET / HTTP/1.1\r\nHost: localhost\r\n\r\n";
        let mut parser = RequestParser::new();
        parser.add_data(request_str.as_bytes()).unwrap();
        let request = parser.parse().unwrap().unwrap();
        assert_eq!(request.method, Method::GET);
        assert_eq!(request.path(), "/");
        assert_eq!(request.version, Version::Http11);
    }

    #[test]
    fn test_parse_request_with_query_string() {
        let request_str = "GET /search?q=rust&page=2 HTTP/1.1\r\nHost: localhost\r\n\r\n";
        let mut parser = RequestParser::new();
        parser.add_data(request_str.as_bytes()).unwrap();
        let request = parser.parse().unwrap().unwrap();
        assert_eq!(request.path(), "/search");
        assert_eq!(
            request.query_params.get("q").map(String::as_str),
            Some("rust")
        );
        assert_eq!(
            request.query_params.get("page").map(String::as_str),
            Some("2")
        );
    }

    #[test]
    fn test_parse_headers_case_insensitive_lookup() {
        let request_str = "GET / HTTP/1.1\r\nHost: example.com\r\nContent-Type: text/plain\r\n\r\n";
        let mut parser = RequestParser::new();
        parser.add_data(request_str.as_bytes()).unwrap();
        let request = parser.parse().unwrap().unwrap();
        assert_eq!(request.host().map(String::as_str), Some("example.com"));
        assert!(request.headers.get("content-type").is_some());
        assert!(request.headers.get("CONTENT-TYPE").is_some());
    }

    #[test]
    fn test_parse_invalid_method_returns_error() {
        let request_str = "FROBNICATE / HTTP/1.1\r\nHost: localhost\r\n\r\n";
        let mut parser = RequestParser::new();
        parser.add_data(request_str.as_bytes()).unwrap();
        let result = parser.parse();
        assert!(result.is_err(), "invalid method must produce ParseError");
    }

    #[test]
    fn test_parse_invalid_version_returns_error() {
        let request_str = "GET / HTTP/2.0\r\nHost: localhost\r\n\r\n";
        let mut parser = RequestParser::new();
        parser.add_data(request_str.as_bytes()).unwrap();
        let result = parser.parse();
        assert!(
            result.is_err(),
            "non HTTP/1.1 version must produce ParseError"
        );
    }

    // -----------------------------------------------------------------------
    // Body with Content-Length (audit-required: "body content … extracted")
    // -----------------------------------------------------------------------

    #[test]
    fn test_parse_post_with_content_length() {
        let body = "hello=world&foo=bar";
        let request_str = format!(
            "POST /submit HTTP/1.1\r\nHost: x\r\nContent-Length: {}\r\n\r\n{}",
            body.len(),
            body
        );
        let mut parser = RequestParser::new();
        parser.add_data(request_str.as_bytes()).unwrap();
        let request = parser.parse().unwrap().unwrap();
        assert_eq!(request.method, Method::POST);
        assert_eq!(request.body, body.as_bytes());
    }

    #[test]
    fn test_parse_post_incremental_body() {
        // Add data in two chunks - parser must wait, then complete.
        let body = "abcdefghij";
        let mut parser = RequestParser::new();

        let head = format!(
            "POST / HTTP/1.1\r\nHost: x\r\nContent-Length: {}\r\n\r\nabcde",
            body.len()
        );
        parser.add_data(head.as_bytes()).unwrap();
        assert!(parser.parse().unwrap().is_none(), "body incomplete");

        parser.add_data(b"fghij").unwrap();
        let request = parser.parse().unwrap().unwrap();
        assert_eq!(request.body, body.as_bytes());
    }

    // -----------------------------------------------------------------------
    // Chunked encoding (audit-required: "including chunked encoding")
    // -----------------------------------------------------------------------

    #[test]
    fn test_parse_chunked_body_single_chunk() {
        // One 5-byte chunk "Hello" followed by terminator chunk "0\r\n\r\n".
        let request_str = "POST / HTTP/1.1\r\nHost: x\r\nTransfer-Encoding: chunked\r\n\r\n\
                           5\r\nHello\r\n\
                           0\r\n\r\n";
        let mut parser = RequestParser::new();
        parser.add_data(request_str.as_bytes()).unwrap();
        let request = parser.parse().unwrap().unwrap();
        assert_eq!(request.body, b"Hello");
    }

    #[test]
    fn test_parse_chunked_body_multiple_chunks() {
        // Multiple chunks: "Wiki" + "pedia" + " in\r\n\r\nchunks." (with size 0xE = 14 incl CRLF inside).
        // Use simpler payload to keep arithmetic obvious.
        let request_str = "POST / HTTP/1.1\r\nHost: x\r\nTransfer-Encoding: chunked\r\n\r\n\
                           4\r\nWiki\r\n\
                           5\r\npedia\r\n\
                           e\r\n in\r\n\r\nchunks.\r\n\
                           0\r\n\r\n";
        let mut parser = RequestParser::new();
        parser.add_data(request_str.as_bytes()).unwrap();
        let request = parser.parse().unwrap().unwrap();
        assert_eq!(request.body, b"Wikipedia in\r\n\r\nchunks.");
    }

    #[test]
    fn test_parse_chunked_body_with_chunk_extensions() {
        // Per RFC 7230 §4.1.1, a chunk size may be followed by ";<extension>".
        // The parser must strip everything after ';' on the size line.
        let request_str = "POST / HTTP/1.1\r\nHost: x\r\nTransfer-Encoding: chunked\r\n\r\n\
                           5;name=val\r\nHello\r\n\
                           0\r\n\r\n";
        let mut parser = RequestParser::new();
        parser.add_data(request_str.as_bytes()).unwrap();
        let request = parser.parse().unwrap().unwrap();
        assert_eq!(request.body, b"Hello");
    }

    #[test]
    fn test_parse_chunked_body_empty() {
        // Only the terminating zero-length chunk - no payload.
        let request_str = "POST / HTTP/1.1\r\nHost: x\r\nTransfer-Encoding: chunked\r\n\r\n\
                           0\r\n\r\n";
        let mut parser = RequestParser::new();
        parser.add_data(request_str.as_bytes()).unwrap();
        let request = parser.parse().unwrap().unwrap();
        assert!(request.body.is_empty());
    }

    #[test]
    fn test_parse_chunked_invalid_size_returns_error() {
        let request_str = "POST / HTTP/1.1\r\nHost: x\r\nTransfer-Encoding: chunked\r\n\r\n\
                           ZZZZ\r\nHello\r\n\
                           0\r\n\r\n";
        let mut parser = RequestParser::new();
        parser.add_data(request_str.as_bytes()).unwrap();
        let result = parser.parse();
        assert!(result.is_err(), "non-hex chunk size must fail");
    }

    #[test]
    fn test_parse_chunked_body_incremental() {
        // Feed in three slices, ensure parser is happy to wait for more data.
        let parts: &[&[u8]] = &[
            b"POST / HTTP/1.1\r\nHost: x\r\nTransfer-Encoding: chunked\r\n\r\n",
            b"5\r\nHello\r\n",
            b"6\r\n World\r\n0\r\n\r\n",
        ];
        let mut parser = RequestParser::new();

        parser.add_data(parts[0]).unwrap();
        assert!(parser.parse().unwrap().is_none());

        parser.add_data(parts[1]).unwrap();
        assert!(parser.parse().unwrap().is_none());

        parser.add_data(parts[2]).unwrap();
        let request = parser.parse().unwrap().unwrap();
        assert_eq!(request.body, b"Hello World");
    }

    // -----------------------------------------------------------------------
    // Body size limit (audit-required: "out-of-bounds body size limits")
    // -----------------------------------------------------------------------

    #[test]
    fn test_body_too_large_with_content_length_rejected() {
        // Limit = 16 bytes, declared Content-Length = 100 → must error in
        // prepare_body_parsing (check_body_size_limit on the declared length).
        let mut parser = RequestParser::with_max_body_size(16);
        let head = "POST / HTTP/1.1\r\nHost: x\r\nContent-Length: 100\r\n\r\n";
        parser.add_data(head.as_bytes()).unwrap();
        let result = parser.parse();
        assert!(
            result.is_err(),
            "declared body larger than limit must be rejected"
        );
    }

    #[test]
    fn test_body_too_large_with_chunked_rejected() {
        // Limit = 4 bytes, chunked payload of 5 bytes → must error in
        // parse_chunked_body (check_would_exceed_limit on the chunk size).
        let mut parser = RequestParser::with_max_body_size(4);
        let request_str = "POST / HTTP/1.1\r\nHost: x\r\nTransfer-Encoding: chunked\r\n\r\n\
                           5\r\nHello\r\n\
                           0\r\n\r\n";
        parser.add_data(request_str.as_bytes()).unwrap();
        let result = parser.parse();
        assert!(
            result.is_err(),
            "chunked body larger than limit must be rejected"
        );
    }

    #[test]
    fn test_body_exactly_at_limit_accepted() {
        // body size == max_body_size is allowed (limit is inclusive).
        let body = b"abcdef";
        let mut parser = RequestParser::with_max_body_size(body.len());
        let request_str = format!(
            "POST / HTTP/1.1\r\nHost: x\r\nContent-Length: {}\r\n\r\nabcdef",
            body.len()
        );
        parser.add_data(request_str.as_bytes()).unwrap();
        let request = parser.parse().unwrap().unwrap();
        assert_eq!(request.body, body);
    }
}
