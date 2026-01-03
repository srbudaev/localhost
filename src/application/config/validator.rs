use crate::application::config::models::{Config, RouteConfig, ServerConfig};
use crate::common::error::{Result, ServerError};
use std::collections::{HashMap, HashSet};
use std::net::IpAddr;
use std::path::Path;

/// Validate configuration for correctness and consistency
pub fn validate_config(config: &Config) -> Result<()> {
    // Validate global settings
    validate_global_settings(config)?;

    // Validate servers
    if config.servers.is_empty() {
        return Err(ServerError::ConfigError(
            "At least one server must be configured".to_string(),
        ));
    }

    // Check for port conflicts
    validate_port_conflicts(config)?;

    // Validate each server
    for (idx, server) in config.servers.iter().enumerate() {
        validate_server(server, idx)?;
    }

    // Validate admin config if present
    if let Some(ref admin) = config.admin {
        validate_admin(admin)?;
    }

    Ok(())
}

fn validate_global_settings(config: &Config) -> Result<()> {
    if config.client_timeout_secs == 0 {
        return Err(ServerError::ConfigError(
            "client_timeout_secs must be greater than 0".to_string(),
        ));
    }

    if config.client_max_body_size == 0 {
        return Err(ServerError::ConfigError(
            "client_max_body_size must be greater than 0".to_string(),
        ));
    }

    Ok(())
}

fn validate_port_conflicts(config: &Config) -> Result<()> {
    let mut port_map: HashMap<(IpAddr, u16), Vec<usize>> = HashMap::new();

    for (idx, server) in config.servers.iter().enumerate() {
        for port in &server.ports {
            let key = (server.server_address, *port);
            port_map.entry(key).or_insert_with(Vec::new).push(idx);
        }
    }

    for ((addr, port), indices) in port_map {
        if indices.len() > 1 {
            let server_names: Vec<String> = indices
                .iter()
                .map(|&idx| config.servers[idx].server_name.clone())
                .collect();
            return Err(ServerError::ConfigError(format!(
                "Port conflict: {}:{} is used by multiple servers: {:?}",
                addr, port, server_names
            )));
        }
    }

    Ok(())
}

fn validate_server(server: &ServerConfig, index: usize) -> Result<()> {
    // Validate server address
    // Note: 0.0.0.0 is allowed for Docker/container deployments
    // The original check was removed to support binding to all interfaces

    // Validate ports
    if server.ports.is_empty() {
        return Err(ServerError::ConfigError(format!(
            "Server {}: at least one port must be specified",
            index
        )));
    }

    for port in &server.ports {
        if *port == 0 {
            return Err(ServerError::ConfigError(format!(
                "Server {}: port cannot be 0",
                index
            )));
        }
    }

    // Validate server name
    if server.server_name.is_empty() {
        return Err(ServerError::ConfigError(format!(
            "Server {}: server_name cannot be empty",
            index
        )));
    }

    // Validate root directory exists
    if !Path::new(&server.root).exists() {
        return Err(ServerError::ConfigError(format!(
            "Server {}: root directory '{}' does not exist",
            index, server.root
        )));
    }

    if !Path::new(&server.root).is_dir() {
        return Err(ServerError::ConfigError(format!(
            "Server {}: root '{}' is not a directory",
            index, server.root
        )));
    }

    // Validate routes
    for (path, route) in &server.routes {
        validate_route(route, path, index)?;
    }

    // Validate error pages
    for (code, error_page) in &server.errors {
        validate_error_code(code)?;
        if error_page.filename.is_empty() {
            return Err(ServerError::ConfigError(format!(
                "Server {}: error page filename for {} cannot be empty",
                index, code
            )));
        }
    }

    // Validate CGI handlers
    for (ext, interpreter) in &server.cgi_handlers {
        if !ext.starts_with('.') {
            return Err(ServerError::ConfigError(format!(
                "Server {}: CGI extension '{}' must start with '.'",
                index, ext
            )));
        }
        if interpreter.is_empty() {
            return Err(ServerError::ConfigError(format!(
                "Server {}: CGI interpreter for '{}' cannot be empty",
                index, ext
            )));
        }
    }

    Ok(())
}

fn validate_route(route: &RouteConfig, path: &str, server_idx: usize) -> Result<()> {
    // Validate path
    if path.is_empty() {
        return Err(ServerError::ConfigError(format!(
            "Server {}: route path cannot be empty",
            server_idx
        )));
    }

    if !path.starts_with('/') {
        return Err(ServerError::ConfigError(format!(
            "Server {}: route path '{}' must start with '/'",
            server_idx, path
        )));
    }

    // Validate methods
    if route.methods.is_empty() {
        return Err(ServerError::ConfigError(format!(
            "Server {}: route '{}' must specify at least one method",
            server_idx, path
        )));
    }

    let valid_methods: HashSet<&str> = ["GET", "POST", "DELETE", "PUT", "PATCH", "HEAD", "OPTIONS"]
        .iter()
        .copied()
        .collect();

    for method in &route.methods {
        if !valid_methods.contains(method.as_str()) {
            return Err(ServerError::ConfigError(format!(
                "Server {}: route '{}' has invalid method '{}'",
                server_idx, path, method
            )));
        }
    }

    // Validate route configuration consistency
    // Route can have: filename OR directory OR redirect (mutually exclusive)
    // CGI extension can be combined with filename/directory
    let has_file = route.filename.is_some();
    let has_dir = route.directory.is_some();
    let has_redirect = route.redirect.is_some();

    let target_count = [has_file, has_dir, has_redirect]
        .iter()
        .filter(|&&x| x)
        .count();

    if target_count > 1 {
        return Err(ServerError::ConfigError(format!(
            "Server {}: route '{}' cannot specify multiple targets (filename, directory, redirect)",
            server_idx, path
        )));
    }

    // Route without explicit target is valid - will use default behavior
    // (serve from root directory or return 404)

    // Validate filename/directory paths if specified
    if let Some(ref filename) = route.filename {
        if filename.is_empty() {
            return Err(ServerError::ConfigError(format!(
                "Server {}: route '{}' filename cannot be empty",
                server_idx, path
            )));
        }
    }

    if let Some(ref directory) = route.directory {
        if directory.is_empty() {
            return Err(ServerError::ConfigError(format!(
                "Server {}: route '{}' directory cannot be empty",
                server_idx, path
            )));
        }
    }

    // Validate redirect
    if let Some(ref redirect) = route.redirect {
        if redirect.is_empty() {
            return Err(ServerError::ConfigError(format!(
                "Server {}: route '{}' redirect cannot be empty",
                server_idx, path
            )));
        }
        if !redirect.starts_with('/') && !redirect.starts_with("http://")
            && !redirect.starts_with("https://")
        {
            return Err(ServerError::ConfigError(format!(
                "Server {}: route '{}' redirect must start with '/', 'http://', or 'https://'",
                server_idx, path
            )));
        }
    }

    Ok(())
}

fn validate_error_code(code: &str) -> Result<()> {
    let valid_codes: HashSet<&str> = ["400", "403", "404", "405", "413", "500"]
        .iter()
        .copied()
        .collect();

    if !valid_codes.contains(code) {
        return Err(ServerError::ConfigError(format!(
            "Invalid error code '{}'. Valid codes are: 400, 403, 404, 405, 413, 500",
            code
        )));
    }

    Ok(())
}

fn validate_admin(admin: &crate::application::config::models::AdminConfig) -> Result<()> {
    if admin.username.is_empty() {
        return Err(ServerError::ConfigError(
            "Admin username cannot be empty".to_string(),
        ));
    }

    if admin.password.is_empty() {
        return Err(ServerError::ConfigError(
            "Admin password cannot be empty".to_string(),
        ));
    }

    Ok(())
}
