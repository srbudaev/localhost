use crate::application::config::models::ServerConfig;
use crate::application::server::listener::Listener;
use crate::common::error::{Result, ServerError};
use std::collections::HashMap;
use std::net::SocketAddr;
use std::path::PathBuf;

/// Represents a single server instance (virtual host)
pub struct ServerInstance {
    /// Server configuration
    config: ServerConfig,

    /// Root directory path (resolved absolute path)
    root_path: PathBuf,

    /// Listeners for each port
    listeners: HashMap<u16, Listener>,

    /// Whether this is the default server for its ports
    is_default: bool,
}

impl ServerInstance {
    /// Create a new server instance from configuration
    /// If create_listeners is false, listeners won't be created (for shared port scenarios)
    pub fn new(config: ServerConfig, is_default: bool) -> Result<Self> {
        Self::new_without_listeners(config, is_default)
    }

    /// Create a new server instance without creating listeners
    /// Used when listeners are managed at ServerManager level for shared ports
    pub fn new_without_listeners(config: ServerConfig, is_default: bool) -> Result<Self> {
        // Resolve root path to absolute
        let root_path = std::fs::canonicalize(&config.root)
            .map_err(|e| {
                ServerError::ConfigError(format!(
                    "Failed to resolve root path '{}': {}",
                    config.root, e
                ))
            })?;

        // Verify root is a directory
        if !root_path.is_dir() {
            return Err(ServerError::ConfigError(format!(
                "Root path '{}' is not a directory",
                root_path.display()
            )));
        }

        Ok(Self {
            config,
            root_path,
            listeners: HashMap::new(),
            is_default,
        })
    }

    /// Create listeners for all configured ports
    /// This is now optional - listeners can be created at ServerManager level
    pub fn create_listeners(&mut self) -> Result<()> {
        for port in &self.config.ports {
            let addr = SocketAddr::new(self.config.server_address, *port);
            let listener = Listener::new(addr)?;
            self.listeners.insert(*port, listener);
        }
        Ok(())
    }

    /// Get server name
    pub fn server_name(&self) -> &str {
        &self.config.server_name
    }

    /// Get root directory path
    pub fn root_path(&self) -> &PathBuf {
        &self.root_path
    }

    /// Get server configuration
    pub fn config(&self) -> &ServerConfig {
        &self.config
    }

    /// Check if this is the default server
    pub fn is_default(&self) -> bool {
        self.is_default
    }

    /// Get listener for a specific port
    pub fn listener(&self, port: u16) -> Option<&Listener> {
        self.listeners.get(&port)
    }

    /// Get all listeners
    pub fn listeners(&self) -> &HashMap<u16, Listener> {
        &self.listeners
    }

    /// Get all ports this server listens on
    pub fn ports(&self) -> Vec<u16> {
        self.config.ports.clone()
    }

    /// Check if server has admin access enabled
    pub fn has_admin_access(&self) -> bool {
        self.config.admin_access
    }
}
