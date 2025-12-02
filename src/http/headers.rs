use std::collections::HashMap;
use std::fmt;

/// HTTP headers container
#[derive(Debug, Clone)]
pub struct Headers {
    headers: HashMap<String, Vec<String>>,
}

impl Headers {
    /// Create new empty headers
    pub fn new() -> Self {
        Self {
            headers: HashMap::new(),
        }
    }

    /// Get header value (case-insensitive)
    pub fn get(&self, name: &str) -> Option<&String> {
        let name_lower = name.to_lowercase();
        self.headers
            .iter()
            .find(|(k, _)| k.to_lowercase() == name_lower)
            .and_then(|(_, v)| v.first())
    }

    /// Get all values for a header (case-insensitive)
    pub fn get_all(&self, name: &str) -> Option<&Vec<String>> {
        let name_lower = name.to_lowercase();
        self.headers
            .iter()
            .find(|(k, _)| k.to_lowercase() == name_lower)
            .map(|(_, v)| v)
    }

    /// Set header value (replaces existing)
    pub fn set(&mut self, name: String, value: String) {
        self.headers.insert(name, vec![value]);
    }

    /// Add header value (appends to existing)
    pub fn add(&mut self, name: String, value: String) {
        self.headers
            .entry(name)
            .or_insert_with(Vec::new)
            .push(value);
    }

    /// Remove header (case-insensitive)
    pub fn remove(&mut self, name: &str) {
        let name_lower = name.to_lowercase();
        if let Some(key) = self
            .headers
            .keys()
            .find(|k| k.to_lowercase() == name_lower)
            .cloned()
        {
            self.headers.remove(&key);
        }
    }

    /// Check if header exists (case-insensitive)
    pub fn contains(&self, name: &str) -> bool {
        let name_lower = name.to_lowercase();
        self.headers
            .keys()
            .any(|k| k.to_lowercase() == name_lower)
    }

    /// Get all headers as iterator
    pub fn iter(&self) -> impl Iterator<Item = (&String, &Vec<String>)> {
        self.headers.iter()
    }

    /// Check if headers are empty
    pub fn is_empty(&self) -> bool {
        self.headers.is_empty()
    }

    /// Get number of unique header names
    pub fn len(&self) -> usize {
        self.headers.len()
    }

    /// Parse headers from raw header lines
    pub fn from_lines(lines: &[String]) -> Result<Self, HeaderParseError> {
        let mut headers = Headers::new();

        for line in lines {
            if let Some(colon_pos) = line.find(':') {
                let name = line[..colon_pos].trim().to_string();
                let value = line[colon_pos + 1..].trim().to_string();

                if name.is_empty() {
                    return Err(HeaderParseError::InvalidFormat);
                }

                headers.add(name, value);
            } else {
                return Err(HeaderParseError::InvalidFormat);
            }
        }

        Ok(headers)
    }

    /// Serialize headers to HTTP format
    pub fn to_string(&self) -> String {
        let mut result = String::new();
        for (name, values) in &self.headers {
            for value in values {
                result.push_str(name);
                result.push_str(": ");
                result.push_str(value);
                result.push_str("\r\n");
            }
        }
        result
    }
}

impl Default for Headers {
    fn default() -> Self {
        Self::new()
    }
}

/// Error type for header parsing
#[derive(Debug, Clone)]
pub enum HeaderParseError {
    InvalidFormat,
}

impl fmt::Display for HeaderParseError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            HeaderParseError::InvalidFormat => {
                write!(f, "Invalid header format")
            }
        }
    }
}

impl std::error::Error for HeaderParseError {}

// Common header names as constants
pub mod names {
    pub const CONTENT_TYPE: &str = "Content-Type";
    pub const CONTENT_LENGTH: &str = "Content-Length";
    pub const TRANSFER_ENCODING: &str = "Transfer-Encoding";
    pub const CONNECTION: &str = "Connection";
    pub const HOST: &str = "Host";
    pub const USER_AGENT: &str = "User-Agent";
    pub const ACCEPT: &str = "Accept";
    pub const ACCEPT_ENCODING: &str = "Accept-Encoding";
    pub const COOKIE: &str = "Cookie";
    pub const SET_COOKIE: &str = "Set-Cookie";
    pub const LOCATION: &str = "Location";
    pub const SERVER: &str = "Server";
    pub const DATE: &str = "Date";
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_headers_basic() {
        let mut headers = Headers::new();
        headers.set("Content-Type".to_string(), "text/html".to_string());
        assert_eq!(headers.get("content-type"), Some(&"text/html".to_string()));
        assert_eq!(headers.get("Content-Type"), Some(&"text/html".to_string()));
    }

    #[test]
    fn test_headers_case_insensitive() {
        let mut headers = Headers::new();
        headers.set("Content-Type".to_string(), "text/html".to_string());
        assert!(headers.contains("content-type"));
        assert!(headers.contains("CONTENT-TYPE"));
        assert!(headers.contains("Content-Type"));
    }

    #[test]
    fn test_headers_multiple_values() {
        let mut headers = Headers::new();
        headers.add("Accept".to_string(), "text/html".to_string());
        headers.add("Accept".to_string(), "application/json".to_string());
        let values = headers.get_all("Accept").unwrap();
        assert_eq!(values.len(), 2);
    }

    #[test]
    fn test_headers_parsing() {
        let lines = vec![
            "Content-Type: text/html".to_string(),
            "Content-Length: 123".to_string(),
        ];
        let headers = Headers::from_lines(&lines).unwrap();
        assert_eq!(headers.get("Content-Type"), Some(&"text/html".to_string()));
        assert_eq!(headers.get("Content-Length"), Some(&"123".to_string()));
    }
}
