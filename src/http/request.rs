use crate::http::headers::Headers;
use crate::http::method::Method;
use crate::http::version::Version;
use crate::http::cookie::parse_cookie_header;
use std::collections::HashMap;

/// HTTP request structure
#[derive(Debug, Clone)]
pub struct Request {
    /// HTTP method
    pub method: Method,

    /// Request target (path + query string)
    pub target: String,

    /// HTTP version
    pub version: Version,

    /// Request headers
    pub headers: Headers,

    /// Request body
    pub body: Vec<u8>,

    /// Parsed query parameters
    pub query_params: HashMap<String, String>,
}

impl Request {
    /// Create a new request
    pub fn new(method: Method, target: String, version: Version) -> Self {
        Self {
            method,
            target,
            version,
            headers: Headers::new(),
            body: Vec::new(),
            query_params: HashMap::new(),
        }
    }

    /// Get the path part of the target (without query string)
    pub fn path(&self) -> &str {
        if let Some(pos) = self.target.find('?') {
            &self.target[..pos]
        } else {
            &self.target
        }
    }

    /// Get the query string part
    pub fn query_string(&self) -> Option<&str> {
        self.target.find('?').map(|pos| &self.target[pos + 1..])
    }

    /// Parse query parameters from target
    pub fn parse_query_params(&mut self) {
        if let Some(query_pos) = self.target.find('?') {
            let query = self.target[query_pos + 1..].to_string();
            let mut params = Vec::new();
            
            for pair in query.split('&') {
                if let Some(equal_pos) = pair.find('=') {
                    let key = url_decode(&pair[..equal_pos]);
                    let value = url_decode(&pair[equal_pos + 1..]);
                    params.push((key, value));
                } else if !pair.is_empty() {
                    let key = url_decode(pair);
                    params.push((key, String::new()));
                }
            }
            
            for (key, value) in params {
                self.query_params.insert(key, value);
            }
        }
    }

    /// Get Content-Length header value
    pub fn content_length(&self) -> Option<usize> {
        self.headers
            .get("Content-Length")
            .and_then(|v| v.parse().ok())
    }

    /// Check if request has body
    pub fn has_body(&self) -> bool {
        self.method.allows_body() && self.content_length().unwrap_or(0) > 0
    }

    /// Get Host header value
    pub fn host(&self) -> Option<&String> {
        self.headers.get("Host")
    }

    /// Get Connection header value
    pub fn connection(&self) -> Option<&String> {
        self.headers.get("Connection")
    }

    /// Check if connection should be kept alive
    pub fn should_keep_alive(&self) -> bool {
        match self.connection() {
            Some(conn) => conn.eq_ignore_ascii_case("keep-alive"),
            None => self.version.supports_keep_alive(),
        }
    }

    /// Get Transfer-Encoding header value
    pub fn transfer_encoding(&self) -> Option<&String> {
        self.headers.get("Transfer-Encoding")
    }

    /// Check if request uses chunked encoding
    pub fn is_chunked(&self) -> bool {
        self.transfer_encoding()
            .map(|v| v.eq_ignore_ascii_case("chunked"))
            .unwrap_or(false)
    }

    /// Get Content-Type header value
    pub fn content_type(&self) -> Option<&String> {
        self.headers.get("Content-Type")
    }

    /// Get all cookies from Cookie header
    pub fn cookies(&self) -> HashMap<String, String> {
        self.headers
            .get("Cookie")
            .map(|header| parse_cookie_header(header))
            .unwrap_or_default()
    }

    /// Get a specific cookie value by name
    pub fn cookie(&self, name: &str) -> Option<String> {
        self.cookies().get(name).cloned()
    }
}

/// URL decode function
fn url_decode(s: &str) -> String {
    let mut result = String::new();
    let mut chars = s.chars().peekable();

    while let Some(ch) = chars.next() {
        if ch == '%' {
            let mut hex = String::new();
            if let Some(c1) = chars.next() {
                hex.push(c1);
                if let Some(c2) = chars.next() {
                    hex.push(c2);
                    if let Ok(byte) = u8::from_str_radix(&hex, 16) {
                        result.push(byte as char);
                        continue;
                    }
                }
            }
            result.push('%');
            result.push_str(&hex);
        } else if ch == '+' {
            result.push(' ');
        } else {
            result.push(ch);
        }
    }

    result
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_request_path() {
        let req = Request::new(Method::GET, "/path?key=value".to_string(), Version::Http11);
        assert_eq!(req.path(), "/path");
    }

    #[test]
    fn test_query_string() {
        let req = Request::new(Method::GET, "/path?key=value".to_string(), Version::Http11);
        assert_eq!(req.query_string(), Some("key=value"));
    }

    #[test]
    fn test_parse_query_params() {
        let mut req = Request::new(
            Method::GET,
            "/path?key1=value1&key2=value2".to_string(),
            Version::Http11,
        );
        req.parse_query_params();
        assert_eq!(req.query_params.get("key1"), Some(&"value1".to_string()));
        assert_eq!(req.query_params.get("key2"), Some(&"value2".to_string()));
    }

    #[test]
    fn test_keep_alive() {
        let mut req = Request::new(Method::GET, "/".to_string(), Version::Http11);
        assert!(req.should_keep_alive());

        req.headers.set("Connection".to_string(), "close".to_string());
        assert!(!req.should_keep_alive());
    }
}
