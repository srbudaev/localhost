use crate::common::constants::CRLF;
use crate::common::error::{Result, ServerError};
use crate::http::response::Response;
use std::io::Write;

/// Serialize HTTP response to bytes
pub struct ResponseSerializer;

impl ResponseSerializer {
    /// Write status line to buffer
    fn write_status_line(buffer: &mut Vec<u8>, response: &Response) -> Result<()> {
        write!(
            buffer,
            "{} {} {}{}",
            response.version,
            response.status,
            response.status.reason_phrase(),
            CRLF
        )
        .map_err(|e| ServerError::HttpError(format!("Failed to write status line: {}", e)))?;
        Ok(())
    }

    /// Serialize response to bytes
    pub fn serialize(response: &Response) -> Result<Vec<u8>> {
        let mut buffer = Vec::new();

        // Status line
        Self::write_status_line(&mut buffer, response)?;

        // Headers
        let headers_str = response.headers.to_string();
        buffer.extend_from_slice(headers_str.as_bytes());

        // Empty line after headers
        buffer.extend_from_slice(CRLF.as_bytes());

        // Body
        if response.has_body() {
            buffer.extend_from_slice(&response.body);
        }

        Ok(buffer)
    }

    /// Serialize response with chunked encoding
    pub fn serialize_chunked(response: &Response) -> Result<Vec<u8>> {
        let mut buffer = Vec::new();

        // Status line
        Self::write_status_line(&mut buffer, response)?;

        // Headers
        let headers_str = response.headers.to_string();
        buffer.extend_from_slice(headers_str.as_bytes());

        // Empty line after headers
        buffer.extend_from_slice(CRLF.as_bytes());

        // Chunked body
        if !response.body.is_empty() {
            // Write chunk size and data
            write!(buffer, "{:x}{}", response.body.len(), CRLF)
                .map_err(|e| ServerError::HttpError(format!("Failed to write chunk size: {}", e)))?;
            buffer.extend_from_slice(&response.body);
            buffer.extend_from_slice(CRLF.as_bytes());
        }

        // Last chunk (empty)
        buffer.extend_from_slice(b"0");
        buffer.extend_from_slice(CRLF.as_bytes());
        buffer.extend_from_slice(CRLF.as_bytes());

        Ok(buffer)
    }

    /// Serialize response (automatically chooses chunked or regular)
    pub fn serialize_auto(response: &Response) -> Result<Vec<u8>> {
        if response.chunked {
            Self::serialize_chunked(response)
        } else {
            Self::serialize(response)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::http::version::Version;

    #[test]
    fn test_serialize_simple_response() {
        let mut response = Response::ok(Version::Http11);
        response.set_body_str("Hello");
        let bytes = ResponseSerializer::serialize(&response).unwrap();
        let text = String::from_utf8(bytes).unwrap();
        assert!(text.contains("200 OK"));
        assert!(text.contains("Hello"));
    }

    #[test]
    fn test_serialize_chunked_response() {
        let mut response = Response::ok(Version::Http11);
        response.set_chunked();
        response.set_body_str("Hello");
        let bytes = ResponseSerializer::serialize_chunked(&response).unwrap();
        let text = String::from_utf8(bytes).unwrap();
        assert!(text.contains("200 OK"));
        assert!(text.contains("Transfer-Encoding: chunked"));
    }
}
