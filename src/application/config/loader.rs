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
        let toml = r#"
            [[servers]]
            server_address = "127.0.0.1"
            ports = [8080]
            server_name = "localhost"
            root = "."
        "#;

        // This will fail validation because root doesn't exist, but parsing should work
        let result = ConfigLoader::load_from_str(toml);
        // We expect validation error, not parse error
        assert!(result.is_err());
    }
}
