use crate::application::config::models::{RouteConfig, ServerConfig};
use crate::common::error::{Result, ServerError};
use crate::http::request::Request;
use crate::http::response::Response;
use std::collections::HashMap;
use std::path::{Path, PathBuf};

/// Router matches requests to routes and determines the appropriate handler
pub struct Router {
    routes: HashMap<String, RouteConfig>,
    root_path: PathBuf,
}

impl Router {
    /// Create a new router from server configuration
    pub fn new(config: &ServerConfig, root_path: PathBuf) -> Self {
        Self {
            routes: config.routes.clone(),
            root_path,
        }
    }

    /// Resolve path - if absolute/relative use as-is, otherwise join with root
    pub fn resolve_path(&self, path: &str) -> PathBuf {
        if path.starts_with('/') || path.starts_with("./") {
            PathBuf::from(path)
        } else {
            self.root_path.join(path)
        }
    }

    /// Match a request to a route and return the route configuration
    pub fn match_route(&self, request: &Request) -> Option<&RouteConfig> {
        let path = request.path();

        // Try exact match first
        if let Some(route) = self.routes.get(path) {
            return Some(route);
        }

        // Try longest prefix match
        let mut best_match: Option<(&String, &RouteConfig)> = None;
        for (route_path, route_config) in &self.routes {
            if path.starts_with(route_path) {
                if let Some((best_path, _)) = &best_match {
                    if route_path.len() > best_path.len() {
                        best_match = Some((route_path, route_config));
                    }
                } else {
                    best_match = Some((route_path, route_config));
                }
            }
        }

        best_match.map(|(_, config)| config)
    }

    /// Check if method is allowed for the route
    pub fn is_method_allowed(&self, request: &Request, route: &RouteConfig) -> bool {
        if route.methods.is_empty() {
            return true; // No restrictions
        }

        let method_str = request.method.to_string();
        route.methods.iter().any(|m| m.eq_ignore_ascii_case(&method_str))
    }

    /// Validate route and method, return error response if invalid
    pub fn validate_request(&self, request: &Request) -> Result<(&RouteConfig, Option<Response>)> {
        let route = self.match_route(request)
            .ok_or_else(|| ServerError::HttpError("No matching route".to_string()))?;

        if !self.is_method_allowed(request, route) {
            return Ok((route, Some(Response::method_not_allowed_with_message(
                request.version,
                "Method Not Allowed"
            ))));
        }

        Ok((route, None))
    }

    /// Resolve file path for a route
    pub fn resolve_file_path(&self, request: &Request, route: &RouteConfig) -> Result<PathBuf> {
        let path = request.path();

        // If route has filename, use it
        if let Some(ref filename) = route.filename {
            return Ok(self.resolve_path(filename));
        }

        // If route has directory, map path to directory
        if let Some(ref directory) = route.directory {
            let route_path = self.routes
                .iter()
                .find(|(p, _)| path.starts_with(*p))
                .map(|(p, _)| p.as_str())
                .unwrap_or("/");

            // Remove route prefix from request path
            let relative_path = if path.len() > route_path.len() {
                &path[route_path.len()..]
            } else {
                ""
            };

            let dir_path = self.resolve_path(directory);

            let file_path = if relative_path.is_empty() {
                dir_path
            } else {
                // Sanitize path to prevent directory traversal
                let sanitized = self.sanitize_path(relative_path)?;
                dir_path.join(sanitized)
            };

            return Ok(file_path);
        }

        // Default: map to root directory
        let relative_path = if path == "/" {
            ""
        } else {
            &path[1..] // Remove leading /
        };

        if relative_path.is_empty() {
            Ok(self.root_path.clone())
        } else {
            let sanitized = self.sanitize_path(relative_path)?;
            Ok(self.root_path.join(sanitized))
        }
    }

    /// Sanitize path to prevent directory traversal attacks
    fn sanitize_path(&self, path: &str) -> Result<String> {
        let path = Path::new(path);
        
        // Check for directory traversal attempts
        for component in path.components() {
            if let std::path::Component::ParentDir = component {
                return Err(ServerError::HttpError(
                    "Path contains '..' - directory traversal attempt".to_string(),
                ));
            }
        }

        // Normalize path
        let normalized = path
            .components()
            .filter_map(|c| match c {
                std::path::Component::Normal(s) => s.to_str(),
                _ => None,
            })
            .collect::<Vec<_>>()
            .join("/");

        Ok(normalized)
    }

    /// Get default file for a directory route
    pub fn get_default_file<'a>(&self, route: &'a RouteConfig) -> Option<&'a String> {
        route.default_file.as_ref()
    }

    /// Check if directory listing is enabled for route
    pub fn is_directory_listing_enabled(&self, route: &RouteConfig) -> bool {
        route.directory_listing
    }

    /// Get redirect target for route
    pub fn get_redirect<'a>(&self, route: &'a RouteConfig) -> Option<&'a String> {
        route.redirect.as_ref()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::application::config::models::ServerConfig;
    use crate::http::method::Method;
    use crate::http::version::Version;

    fn create_test_config() -> (ServerConfig, PathBuf) {
        let mut config = ServerConfig {
            server_address: "127.0.0.1".parse().unwrap(),
            ports: vec![8080],
            server_name: "test".to_string(),
            root: ".".to_string(),
            admin_access: false,
            routes: HashMap::new(),
            errors: HashMap::new(),
            cgi_handlers: HashMap::new(),
        };

        let mut route = RouteConfig::default();
        route.methods = vec!["GET".to_string()];
        route.directory = Some("static".to_string());
        config.routes.insert("/static".to_string(), route);

        let root = std::env::current_dir().unwrap();
        (config, root)
    }

    #[test]
    fn test_route_matching() {
        let (config, root) = create_test_config();
        let router = Router::new(&config, root);
        
        let request = Request::new(
            Method::GET,
            "/static/file.html".to_string(),
            Version::Http11,
        );
        
        assert!(router.match_route(&request).is_some());
    }
}
