use std::fmt;
use std::str::FromStr;

/// HTTP version as defined in RFC 9112
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Version {
    Http09, // HTTP/0.9 (not supported, but defined)
    Http10, // HTTP/1.0
    Http11, // HTTP/1.1
}

impl Version {
    /// Get major version number
    pub fn major(&self) -> u8 {
        match self {
            Version::Http09 => 0,
            Version::Http10 => 1,
            Version::Http11 => 1,
        }
    }

    /// Get minor version number
    pub fn minor(&self) -> u8 {
        match self {
            Version::Http09 => 9,
            Version::Http10 => 0,
            Version::Http11 => 1,
        }
    }

    /// Check if version supports persistent connections
    pub fn supports_keep_alive(&self) -> bool {
        matches!(self, Version::Http10 | Version::Http11)
    }

    /// Check if version supports chunked encoding
    pub fn supports_chunked(&self) -> bool {
        matches!(self, Version::Http11)
    }
}

impl Default for Version {
    fn default() -> Self {
        Version::Http11
    }
}

impl fmt::Display for Version {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let s = match self {
            Version::Http09 => "HTTP/0.9",
            Version::Http10 => "HTTP/1.0",
            Version::Http11 => "HTTP/1.1",
        };
        write!(f, "{}", s)
    }
}

impl FromStr for Version {
    type Err = VersionParseError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "HTTP/0.9" => Ok(Version::Http09),
            "HTTP/1.0" => Ok(Version::Http10),
            "HTTP/1.1" => Ok(Version::Http11),
            _ => Err(VersionParseError::InvalidVersion(s.to_string())),
        }
    }
}

/// Error type for version parsing
#[derive(Debug, Clone)]
pub enum VersionParseError {
    InvalidVersion(String),
}

impl fmt::Display for VersionParseError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            VersionParseError::InvalidVersion(version) => {
                write!(f, "Invalid HTTP version: {}", version)
            }
        }
    }
}

impl std::error::Error for VersionParseError {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_version_parsing() {
        assert_eq!(Version::from_str("HTTP/1.1").unwrap(), Version::Http11);
        assert_eq!(Version::from_str("HTTP/1.0").unwrap(), Version::Http10);
        assert!(Version::from_str("HTTP/2.0").is_err());
    }

    #[test]
    fn test_version_display() {
        assert_eq!(Version::Http11.to_string(), "HTTP/1.1");
        assert_eq!(Version::Http10.to_string(), "HTTP/1.0");
    }

    #[test]
    fn test_version_properties() {
        assert!(Version::Http11.supports_keep_alive());
        assert!(Version::Http11.supports_chunked());
        assert!(Version::Http10.supports_keep_alive());
        assert!(!Version::Http10.supports_chunked());
    }
}
