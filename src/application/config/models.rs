use crate::common::constants::{DEFAULT_MAX_BODY_SIZE, DEFAULT_REQUEST_TIMEOUT_SECS};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::net::IpAddr;

/// Main configuration structure
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct Config {
    /// Global client timeout in seconds
    #[serde(default = "default_timeout")]
    pub client_timeout_secs: u64,

    /// Maximum client body size in bytes (for uploads)
    #[serde(default = "default_max_body_size")]
    pub client_max_body_size: usize,

    /// Server instances
    pub servers: Vec<ServerConfig>,

    /// Admin credentials (optional)
    #[serde(default)]
    pub admin: Option<AdminConfig>,
}

fn default_timeout() -> u64 {
    DEFAULT_REQUEST_TIMEOUT_SECS
}

fn default_max_body_size() -> usize {
    DEFAULT_MAX_BODY_SIZE
}

/// Server instance configuration
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct ServerConfig {
    /// Server bind address
    pub server_address: IpAddr,

    /// Ports to listen on
    pub ports: Vec<u16>,

    /// Server name for virtual host matching
    pub server_name: String,

    /// Root directory for this server
    pub root: String,

    /// Enable admin access
    #[serde(default)]
    pub admin_access: bool,

    /// Route configurations
    #[serde(default)]
    pub routes: HashMap<String, RouteConfig>,

    /// Custom error pages
    #[serde(default)]
    pub errors: HashMap<String, ErrorPageConfig>,

    /// CGI handler mappings (extension -> interpreter)
    #[serde(default)]
    pub cgi_handlers: HashMap<String, String>,
}

/// Route configuration
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct RouteConfig {
    /// Allowed HTTP methods
    #[serde(default)]
    pub methods: Vec<String>,

    /// File to serve for this route
    #[serde(default)]
    pub filename: Option<String>,

    /// Directory to serve for this route
    #[serde(default)]
    pub directory: Option<String>,

    /// Default file when route is a directory
    #[serde(default)]
    pub default_file: Option<String>,

    /// Enable directory listing
    #[serde(default)]
    pub directory_listing: bool,

    /// Upload directory (for POST requests)
    #[serde(default)]
    pub upload_dir: Option<String>,

    /// HTTP redirect target
    #[serde(default)]
    pub redirect: Option<String>,

    /// CGI extension for this route
    #[serde(default)]
    pub cgi_extension: Option<String>,
}

/// Error page configuration
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct ErrorPageConfig {
    /// Filename of the error page
    pub filename: String,
}

/// Admin configuration
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct AdminConfig {
    /// Admin username
    pub username: String,

    /// Admin password
    pub password: String,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            client_timeout_secs: default_timeout(),
            client_max_body_size: default_max_body_size(),
            servers: Vec::new(),
            admin: None,
        }
    }
}

impl Default for RouteConfig {
    fn default() -> Self {
        Self {
            methods: Vec::new(),
            filename: None,
            directory: None,
            default_file: None,
            directory_listing: false,
            upload_dir: None,
            redirect: None,
            cgi_extension: None,
        }
    }
}
