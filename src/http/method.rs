use std::fmt;
use std::str::FromStr;

/// HTTP method as defined in RFC 9112
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Method {
    GET,
    POST,
    DELETE,
    PUT,
    PATCH,
    HEAD,
    OPTIONS,
    TRACE,
    CONNECT,
}

impl Method {
    /// Check if method is safe (doesn't modify server state)
    pub fn is_safe(&self) -> bool {
        matches!(self, Method::GET | Method::HEAD | Method::OPTIONS | Method::TRACE)
    }

    /// Check if method is idempotent (can be safely repeated)
    pub fn is_idempotent(&self) -> bool {
        matches!(
            self,
            Method::GET | Method::HEAD | Method::PUT | Method::DELETE | Method::OPTIONS | Method::TRACE
        )
    }

    /// Check if method allows request body
    pub fn allows_body(&self) -> bool {
        matches!(self, Method::POST | Method::PUT | Method::PATCH)
    }
}

impl fmt::Display for Method {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let s = match self {
            Method::GET => "GET",
            Method::POST => "POST",
            Method::DELETE => "DELETE",
            Method::PUT => "PUT",
            Method::PATCH => "PATCH",
            Method::HEAD => "HEAD",
            Method::OPTIONS => "OPTIONS",
            Method::TRACE => "TRACE",
            Method::CONNECT => "CONNECT",
        };
        write!(f, "{}", s)
    }
}

impl FromStr for Method {
    type Err = MethodParseError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "GET" => Ok(Method::GET),
            "POST" => Ok(Method::POST),
            "DELETE" => Ok(Method::DELETE),
            "PUT" => Ok(Method::PUT),
            "PATCH" => Ok(Method::PATCH),
            "HEAD" => Ok(Method::HEAD),
            "OPTIONS" => Ok(Method::OPTIONS),
            "TRACE" => Ok(Method::TRACE),
            "CONNECT" => Ok(Method::CONNECT),
            _ => Err(MethodParseError::InvalidMethod(s.to_string())),
        }
    }
}

/// Error type for method parsing
#[derive(Debug, Clone)]
pub enum MethodParseError {
    InvalidMethod(String),
}

impl fmt::Display for MethodParseError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            MethodParseError::InvalidMethod(method) => {
                write!(f, "Invalid HTTP method: {}", method)
            }
        }
    }
}

impl std::error::Error for MethodParseError {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_method_parsing() {
        assert_eq!(Method::from_str("GET").unwrap(), Method::GET);
        assert_eq!(Method::from_str("POST").unwrap(), Method::POST);
        assert_eq!(Method::from_str("DELETE").unwrap(), Method::DELETE);
        assert!(Method::from_str("INVALID").is_err());
    }

    #[test]
    fn test_method_display() {
        assert_eq!(Method::GET.to_string(), "GET");
        assert_eq!(Method::POST.to_string(), "POST");
    }

    #[test]
    fn test_method_properties() {
        assert!(Method::GET.is_safe());
        assert!(!Method::POST.is_safe());
        assert!(Method::GET.is_idempotent());
        assert!(!Method::POST.is_idempotent());
        assert!(Method::POST.allows_body());
        assert!(!Method::GET.allows_body());
    }
}
