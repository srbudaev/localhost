use crate::application::config::models::Config;
use crate::common::error::{Result, ServerError};
use std::fs;

/// Parse configuration from TOML file
pub fn parse_config_file(path: &str) -> Result<Config> {
    let content = fs::read_to_string(path).map_err(|e| {
        ServerError::ConfigError(format!("Failed to read config file '{}': {}", path, e))
    })?;

    parse_config(&content)
}

/// Parse configuration from TOML string
pub fn parse_config(content: &str) -> Result<Config> {
    toml::from_str(content).map_err(|e| {
        ServerError::ConfigError(format!("Failed to parse TOML config: {}", e))
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_minimal_config() {
        let toml = r#"
            [[servers]]
            server_address = "127.0.0.1"
            ports = [8080]
            server_name = "localhost"
            root = "root"
        "#;

        let config = parse_config(toml).unwrap();
        assert_eq!(config.servers.len(), 1);
        assert_eq!(config.servers[0].server_name, "localhost");
    }

    #[test]
    fn test_parse_with_routes() {
        let toml = r#"
            [[servers]]
            server_address = "127.0.0.1"
            ports = [8080]
            server_name = "localhost"
            root = "root"

            [servers.routes."/"]
            methods = ["GET"]

            [servers.routes."/static"]
            directory = "static"
            methods = ["GET"]
        "#;

        let config = parse_config(toml).unwrap();
        assert_eq!(config.servers[0].routes.len(), 2);
        assert!(config.servers[0].routes.contains_key("/"));
    }
}
