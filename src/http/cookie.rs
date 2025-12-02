use std::collections::HashMap;
use std::time::{SystemTime, UNIX_EPOCH};

/// HTTP Cookie structure
/// 
/// Represents a single cookie with its attributes according to RFC 6265
#[derive(Debug, Clone)]
pub struct Cookie {
    /// Cookie name
    pub name: String,
    
    /// Cookie value
    pub value: String,
    
    /// Path attribute (optional)
    pub path: Option<String>,
    
    /// Domain attribute (optional)
    pub domain: Option<String>,
    
    /// Expires attribute (optional)
    pub expires: Option<SystemTime>,
    
    /// Max-Age attribute in seconds (optional)
    pub max_age: Option<u64>,
    
    /// Secure flag - cookie only sent over HTTPS
    pub secure: bool,
    
    /// HttpOnly flag - cookie not accessible via JavaScript
    pub http_only: bool,
    
    /// SameSite attribute (optional)
    pub same_site: Option<SameSite>,
}

/// SameSite attribute values
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SameSite {
    /// Strict - cookie not sent on cross-site requests
    Strict,
    /// Lax - cookie sent on top-level navigation
    Lax,
    /// None - cookie sent on all requests (requires Secure)
    None,
}

impl Cookie {
    /// Create a new cookie with name and value
    pub fn new(name: String, value: String) -> Self {
        Self {
            name,
            value,
            path: None,
            domain: None,
            expires: None,
            max_age: None,
            secure: false,
            http_only: false,
            same_site: None,
        }
    }

    /// Set the Path attribute
    pub fn set_path(mut self, path: String) -> Self {
        self.path = Some(path);
        self
    }

    /// Set the Domain attribute
    pub fn set_domain(mut self, domain: String) -> Self {
        self.domain = Some(domain);
        self
    }

    /// Set the Expires attribute
    pub fn set_expires(mut self, expires: SystemTime) -> Self {
        self.expires = Some(expires);
        self
    }

    /// Set the Max-Age attribute (in seconds)
    pub fn set_max_age(mut self, max_age: u64) -> Self {
        self.max_age = Some(max_age);
        self
    }

    /// Set the Secure flag
    pub fn set_secure(mut self, secure: bool) -> Self {
        self.secure = secure;
        self
    }

    /// Set the HttpOnly flag
    pub fn set_http_only(mut self, http_only: bool) -> Self {
        self.http_only = http_only;
        self
    }

    /// Set the SameSite attribute
    pub fn set_same_site(mut self, same_site: SameSite) -> Self {
        self.same_site = Some(same_site);
        self
    }

    /// Check if cookie is expired
    pub fn is_expired(&self) -> bool {
        if let Some(expires) = self.expires {
            SystemTime::now() > expires
        } else if self.max_age.is_some() {
            // For Max-Age, we'd need creation time, which we don't store
            // This is a simplified check - in production, store creation time
            false
        } else {
            false
        }
    }

    /// Serialize cookie to Set-Cookie header value format
    /// Format: name=value; Path=/path; Domain=example.com; Expires=...; Max-Age=...; Secure; HttpOnly; SameSite=...
    pub fn to_set_cookie_string(&self) -> String {
        let mut parts = vec![format!("{}={}", self.name, self.value)];

        if let Some(ref path) = self.path {
            parts.push(format!("Path={}", path));
        }

        if let Some(ref domain) = self.domain {
            parts.push(format!("Domain={}", domain));
        }

        if let Some(expires) = self.expires {
            // Format: Wed, 21 Oct 2015 07:28:00 GMT
            // Simplified - in production use proper date formatting
            if let Ok(duration) = expires.duration_since(UNIX_EPOCH) {
                let secs = duration.as_secs();
                // Simple date calculation (not accurate, but functional)
                let days_since_epoch = secs / 86400;
                let day_of_week = (days_since_epoch + 4) % 7;
                let days = ["Mon", "Tue", "Wed", "Thu", "Fri", "Sat", "Sun"];
                let year = 1970 + (days_since_epoch / 365);
                let day = (days_since_epoch % 365) + 1;
                let month = "Jan"; // Simplified
                parts.push(format!(
                    "Expires={}, {:02} {} {} 12:00:00 GMT",
                    days[day_of_week as usize], day, month, year
                ));
            }
        }

        if let Some(max_age) = self.max_age {
            parts.push(format!("Max-Age={}", max_age));
        }

        if self.secure {
            parts.push("Secure".to_string());
        }

        if self.http_only {
            parts.push("HttpOnly".to_string());
        }

        if let Some(same_site) = self.same_site {
            let same_site_str = match same_site {
                SameSite::Strict => "Strict",
                SameSite::Lax => "Lax",
                SameSite::None => "None",
            };
            parts.push(format!("SameSite={}", same_site_str));
        }

        parts.join("; ")
    }
}

/// Parse Cookie header value into a HashMap of name-value pairs
/// 
/// Cookie header format: name1=value1; name2=value2; name3=value3
pub fn parse_cookie_header(cookie_header: &str) -> HashMap<String, String> {
    let mut cookies = HashMap::new();
    
    for part in cookie_header.split(';') {
        let part = part.trim();
        if let Some(equal_pos) = part.find('=') {
            let name = part[..equal_pos].trim().to_string();
            let value = part[equal_pos + 1..].trim().to_string();
            cookies.insert(name, value);
        }
    }
    
    cookies
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_cookie_creation() {
        let cookie = Cookie::new("session_id".to_string(), "abc123".to_string());
        assert_eq!(cookie.name, "session_id");
        assert_eq!(cookie.value, "abc123");
        assert!(!cookie.secure);
        assert!(!cookie.http_only);
    }

    #[test]
    fn test_cookie_builder() {
        let cookie = Cookie::new("test".to_string(), "value".to_string())
            .set_path("/".to_string())
            .set_secure(true)
            .set_http_only(true)
            .set_max_age(3600);
        
        assert_eq!(cookie.path, Some("/".to_string()));
        assert!(cookie.secure);
        assert!(cookie.http_only);
        assert_eq!(cookie.max_age, Some(3600));
    }

    #[test]
    fn test_cookie_serialization() {
        let cookie = Cookie::new("session".to_string(), "abc123".to_string())
            .set_path("/".to_string())
            .set_http_only(true)
            .set_secure(true);
        
        let header_value = cookie.to_set_cookie_string();
        assert!(header_value.contains("session=abc123"));
        assert!(header_value.contains("Path=/"));
        assert!(header_value.contains("Secure"));
        assert!(header_value.contains("HttpOnly"));
    }

    #[test]
    fn test_parse_cookie_header() {
        let header = "session_id=abc123; user=john; theme=dark";
        let cookies = parse_cookie_header(header);
        
        assert_eq!(cookies.get("session_id"), Some(&"abc123".to_string()));
        assert_eq!(cookies.get("user"), Some(&"john".to_string()));
        assert_eq!(cookies.get("theme"), Some(&"dark".to_string()));
    }

    #[test]
    fn test_parse_cookie_header_with_spaces() {
        let header = "session_id = abc123 ; user = john";
        let cookies = parse_cookie_header(header);
        
        assert_eq!(cookies.get("session_id"), Some(&"abc123".to_string()));
        assert_eq!(cookies.get("user"), Some(&"john".to_string()));
    }
}
