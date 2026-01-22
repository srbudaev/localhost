use std::fmt;
use std::str::FromStr;

/// HTTP version - only HTTP/1.1 is supported
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Version {
    Http11, // HTTP/1.1
}

impl Version {
    /// Get major version number
    pub fn major(&self) -> u8 {
        1
    }

    /// Get minor version number
    pub fn minor(&self) -> u8 {
        1
    }

    /// Check if version supports persistent connections
    pub fn supports_keep_alive(&self) -> bool {
        true
    }

    /// Check if version supports chunked encoding
    pub fn supports_chunked(&self) -> bool {
        true
    }
}

impl Default for Version {
    fn default() -> Self {
        Version::Http11
    }
}

impl fmt::Display for Version {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "HTTP/1.1")
    }
}

impl FromStr for Version {
    type Err = VersionParseError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
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
        assert!(Version::from_str("HTTP/1.0").is_err());
        assert!(Version::from_str("HTTP/2.0").is_err());
        assert!(Version::from_str("HTTP/0.9").is_err());
    }

    #[test]
    fn test_version_display() {
        assert_eq!(Version::Http11.to_string(), "HTTP/1.1");
    }

    #[test]
    fn test_version_properties() {
        assert!(Version::Http11.supports_keep_alive());
        assert!(Version::Http11.supports_chunked());
        assert_eq!(Version::Http11.major(), 1);
        assert_eq!(Version::Http11.minor(), 1);
    }
}
