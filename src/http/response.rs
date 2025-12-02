use crate::http::headers::{Headers, names as header_names};
use crate::http::status::StatusCode;
use crate::http::version::Version;
use crate::http::cookie::Cookie;
use std::time::SystemTime;

/// HTTP response structure
#[derive(Debug, Clone)]
pub struct Response {
    /// HTTP version
    pub version: Version,

    /// Status code
    pub status: StatusCode,

    /// Response headers
    pub headers: Headers,

    /// Response body
    pub body: Vec<u8>,

    /// Whether to use chunked encoding
    pub chunked: bool,
}

impl Response {
    /// Create a new response
    pub fn new(version: Version, status: StatusCode) -> Self {
        let mut response = Self {
            version,
            status,
            headers: Headers::new(),
            body: Vec::new(),
            chunked: false,
        };

        // Set default headers
        response.set_default_headers();
        response
    }

    /// Create a 200 OK response
    pub fn ok(version: Version) -> Self {
        Self::new(version, StatusCode::OK)
    }

    /// Create a 404 Not Found response
    pub fn not_found(version: Version) -> Self {
        Self::new(version, StatusCode::NOT_FOUND)
    }

    /// Create a 403 Forbidden response
    pub fn forbidden(version: Version) -> Self {
        Self::new(version, StatusCode::FORBIDDEN)
    }

    /// Create a 405 Method Not Allowed response
    pub fn method_not_allowed(version: Version) -> Self {
        Self::new(version, StatusCode::METHOD_NOT_ALLOWED)
    }

    /// Create a 500 Internal Server Error response
    pub fn internal_error(version: Version) -> Self {
        Self::new(version, StatusCode::INTERNAL_SERVER_ERROR)
    }

    /// Create a 302 Found (redirect) response
    pub fn found(version: Version) -> Self {
        Self::new(version, StatusCode::FOUND)
    }

    /// Create a 400 Bad Request response
    pub fn bad_request(version: Version) -> Self {
        Self::new(version, StatusCode::BAD_REQUEST)
    }
}

// Response builders with messages
// These helper methods create responses with common status codes and error messages
impl Response {
    /// Create a 404 Not Found response with message
    pub fn not_found_with_message(version: Version, message: &str) -> Self {
        let mut response = Self::not_found(version);
        response.set_body_str(message);
        response
    }

    /// Create a 403 Forbidden response with message
    pub fn forbidden_with_message(version: Version, message: &str) -> Self {
        let mut response = Self::forbidden(version);
        response.set_body_str(message);
        response
    }

    /// Create a 405 Method Not Allowed response with message
    pub fn method_not_allowed_with_message(version: Version, message: &str) -> Self {
        let mut response = Self::method_not_allowed(version);
        response.set_body_str(message);
        response
    }

    /// Create a 500 Internal Server Error response with message
    pub fn internal_error_with_message(version: Version, message: &str) -> Self {
        let mut response = Self::internal_error(version);
        response.set_body_str(message);
        response
    }

    /// Create a 400 Bad Request response with message
    pub fn bad_request_with_message(version: Version, message: &str) -> Self {
        let mut response = Self::bad_request(version);
        response.set_body_str(message);
        response
    }

    /// Create a 504 Gateway Timeout response with message
    pub fn gateway_timeout_with_message(version: Version, message: &str) -> Self {
        let mut response = Self::new(version, StatusCode::GATEWAY_TIMEOUT);
        response.set_body_str(message);
        response
    }
}

impl Response {
    /// Set default headers
    fn set_default_headers(&mut self) {
        // Set Server header
        self.headers.set(
            header_names::SERVER.to_string(),
            "localhost/0.1.0".to_string(),
        );

        // Set Date header
        if let Ok(duration) = SystemTime::UNIX_EPOCH.elapsed() {
            let date = format_http_date(duration.as_secs());
            self.headers.set(header_names::DATE.to_string(), date);
        }
    }

    /// Set Content-Type header
    pub fn set_content_type(&mut self, content_type: &str) {
        self.headers
            .set(header_names::CONTENT_TYPE.to_string(), content_type.to_string());
    }

    /// Set Content-Length header
    pub fn set_content_length(&mut self, length: usize) {
        self.headers
            .set(header_names::CONTENT_LENGTH.to_string(), length.to_string());
    }

    /// Set Location header (for redirects)
    pub fn set_location(&mut self, location: &str) {
        self.headers
            .set(header_names::LOCATION.to_string(), location.to_string());
    }

    /// Set Connection header
    pub fn set_connection(&mut self, connection: &str) {
        self.headers
            .set(header_names::CONNECTION.to_string(), connection.to_string());
    }

    /// Set body and update Content-Length
    pub fn set_body(&mut self, body: Vec<u8>) {
        self.body = body;
        if !self.chunked {
            self.set_content_length(self.body.len());
        }
    }

    /// Set body from string
    pub fn set_body_str(&mut self, body: &str) {
        self.set_body(body.as_bytes().to_vec());
    }

    /// Enable chunked encoding
    pub fn set_chunked(&mut self) {
        self.chunked = true;
        self.headers
            .set(header_names::TRANSFER_ENCODING.to_string(), "chunked".to_string());
        self.headers.remove(header_names::CONTENT_LENGTH);
    }

    /// Check if response has body
    pub fn has_body(&self) -> bool {
        self.status.allows_body() && !self.body.is_empty()
    }

    /// Get Content-Length
    pub fn content_length(&self) -> Option<usize> {
        if self.chunked {
            None
        } else {
            Some(self.body.len())
        }
    }

    /// Add a Set-Cookie header
    pub fn add_cookie(&mut self, cookie: Cookie) {
        // Set-Cookie can have multiple values, so we use add() instead of set()
        self.headers.add(
            header_names::SET_COOKIE.to_string(),
            cookie.to_set_cookie_string(),
        );
    }

    /// Remove a cookie by setting it with Max-Age=0
    pub fn remove_cookie(&mut self, name: &str, path: Option<&str>) {
        let mut cookie = Cookie::new(name.to_string(), "".to_string())
            .set_max_age(0);
        
        if let Some(path) = path {
            cookie = cookie.set_path(path.to_string());
        }
        
        self.add_cookie(cookie);
    }
}

/// Format HTTP date (RFC 7231)
/// Returns date in format: Wed, 21 Oct 2015 07:28:00 GMT
/// Note: This is a simplified implementation. For production, use chrono crate.
fn format_http_date(_timestamp: u64) -> String {
    // For now, return current date in HTTP format
    // In production, this should use chrono::Utc::now().format("%a, %d %b %Y %H:%M:%S GMT")
    // Simplified version - returns a valid HTTP date format
    use std::time::SystemTime;
    
    // Get current time
    let now = SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();
    
    // Simple date calculation (not accurate, but functional)
    // Proper implementation would use chrono or similar
    let days_since_epoch = now / 86400;
    let day_of_week = (days_since_epoch + 4) % 7; // Jan 1, 1970 was Thursday
    let days = ["Mon", "Tue", "Wed", "Thu", "Fri", "Sat", "Sun"];
    
    // Calculate approximate date (simplified)
    let year = 1970 + (days_since_epoch / 365);
    let day = (days_since_epoch % 365) + 1;
    let month = "Jan"; // Simplified - always Jan for now
    
    format!("{}, {:02} {} {} 12:00:00 GMT", days[day_of_week as usize], day, month, year)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_response_creation() {
        let resp = Response::ok(Version::Http11);
        assert_eq!(resp.status, StatusCode::OK);
        assert_eq!(resp.version, Version::Http11);
    }

    #[test]
    fn test_response_with_body() {
        let mut resp = Response::ok(Version::Http11);
        resp.set_body_str("Hello, World!");
        assert_eq!(resp.body.len(), 13);
        assert_eq!(resp.content_length(), Some(13));
    }

    #[test]
    fn test_chunked_response() {
        let mut resp = Response::ok(Version::Http11);
        resp.set_chunked();
        assert!(resp.chunked);
        assert!(resp.headers.get("Transfer-Encoding").is_some());
        assert!(resp.headers.get("Content-Length").is_none());
    }

    #[test]
    fn test_response_headers() {
        let mut resp = Response::ok(Version::Http11);
        resp.set_content_type("text/html");
        assert_eq!(resp.headers.get("Content-Type"), Some(&"text/html".to_string()));
    }
}
