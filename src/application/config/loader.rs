use crate::application::config::models::Config;
use crate::application::config::parser::parse_config_file;
use crate::application::config::validator::validate_config;
use crate::common::error::Result;

/// Load and validate configuration from file
pub struct ConfigLoader;

impl ConfigLoader {
    /// Load configuration from file path
    pub fn load(path: &str) -> Result<Config> {
        // Parse configuration
        let config = parse_config_file(path)?;

        // Validate configuration
        validate_config(&config)?;

        Ok(config)
    }

    /// Load configuration from string (useful for testing)
    pub fn load_from_str(content: &str) -> Result<Config> {
        use crate::application::config::parser::parse_config;
        let config = parse_config(content)?;
        validate_config(&config)?;
        Ok(config)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_load_valid_config() {
        // A minimal config whose root resolves to the current working directory
        // (which always exists, both locally and on CI). The loader must parse
        // AND validate it successfully.
        let toml = r#"
            [[servers]]
            server_address = "127.0.0.1"
            ports = [8080]
            server_name = "localhost"
            root = "."
        "#;

        let result = ConfigLoader::load_from_str(toml);
        assert!(
            result.is_ok(),
            "valid config must load cleanly, got error: {:?}",
            result.err()
        );
    }

    #[test]
    fn test_load_rejects_nonexistent_root() {
        // Validator must reject a config whose root directory does not exist
        // on disk; the auditor explicitly checks for "invalid paths" handling.
        let toml = r#"
            [[servers]]
            server_address = "127.0.0.1"
            ports = [8080]
            server_name = "localhost"
            root = "/definitely/does/not/exist/localhost-audit-marker"
        "#;

        let result = ConfigLoader::load_from_str(toml);
        assert!(
            result.is_err(),
            "nonexistent root directory must be rejected by validator"
        );
    }
}
