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

    /// Resolve path - if absolute use as-is, if relative (./) resolve relative to root_path, otherwise join with root
    pub fn resolve_path(&self, path: &str) -> PathBuf {
        if path.starts_with('/') {
            PathBuf::from(path)
        } else if path == "." {
            // "." means root directory
            self.root_path.clone()
        } else if let Some(relative) = path.strip_prefix("./") {
            self.root_path.join(relative)
        } else {
            self.root_path.join(path)
        }
    }

    /// Match a request to a route and return the route configuration
    pub fn match_route(&self, request: &Request) -> Option<&RouteConfig> {
        self.match_route_with_path(request)
            .map(|(_, config)| config)
    }

    /// Match a request to a route and return both the matched path and route configuration
    pub fn match_route_with_path(&self, request: &Request) -> Option<(&String, &RouteConfig)> {
        let path = request.path();

        // Try exact match first
        for (route_path, route_config) in &self.routes {
            if route_path == path {
                return Some((route_path, route_config));
            }
        }

        // Try longest prefix match
        // A route matches if:
        // 1. Path exactly equals route path, OR
        // 2. Path starts with route path followed by '/' (for subdirectories/files)
        // Special case: "/" route matches everything
        let mut best_match: Option<(&String, &RouteConfig)> = None;
        for (route_path, route_config) in &self.routes {
            let matches = if path == *route_path {
                true
            } else if route_path == "/" {
                // Root route matches everything
                path.starts_with("/")
            } else if path.starts_with(route_path) {
                // For other routes, check if route path is followed by '/' or is at the end
                // This prevents "/upload" from matching "/uploads/filename"
                let remaining = &path[route_path.len()..];
                remaining.is_empty() || remaining.starts_with('/')
            } else {
                false
            };

            if matches {
                if let Some((best_path, _)) = &best_match {
                    if route_path.len() > best_path.len() {
                        best_match = Some((route_path, route_config));
                    }
                } else {
                    best_match = Some((route_path, route_config));
                }
            }
        }

        best_match
    }

    /// Check if method is allowed for the route
    pub fn is_method_allowed(&self, request: &Request, route: &RouteConfig) -> bool {
        if route.methods.is_empty() {
            return true; // No restrictions
        }

        let method_str = request.method.to_string();
        route
            .methods
            .iter()
            .any(|m| m.eq_ignore_ascii_case(&method_str))
    }

    /// Validate route and method, return error response if invalid
    pub fn validate_request(&self, request: &Request) -> Result<(&RouteConfig, Option<Response>)> {
        let route = self
            .match_route(request)
            .ok_or_else(|| ServerError::HttpError("No matching route".to_string()))?;

        if !self.is_method_allowed(request, route) {
            return Ok((
                route,
                Some(Response::method_not_allowed_with_message(
                    request.version,
                    "Method Not Allowed",
                )),
            ));
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
            let route_path = self
                .routes
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
                // Always return the directory path, not default_file
                // The server manager will decide whether to show directory listing or serve default_file
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
            // Always return the root directory path, not default_file
            // The server manager will decide whether to show directory listing or serve default_file
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

    fn empty_server() -> ServerConfig {
        ServerConfig {
            server_address: "127.0.0.1".parse().unwrap(),
            ports: vec![8080],
            server_name: "test".to_string(),
            root: ".".to_string(),
            admin_access: false,
            routes: HashMap::new(),
            errors: HashMap::new(),
            cgi_handlers: HashMap::new(),
        }
    }

    fn route_with(methods: &[&str], directory: Option<&str>) -> RouteConfig {
        RouteConfig {
            methods: methods.iter().map(|s| s.to_string()).collect(),
            directory: directory.map(|s| s.to_string()),
            ..Default::default()
        }
    }

    fn create_test_config() -> (ServerConfig, PathBuf) {
        let mut config = empty_server();
        config
            .routes
            .insert("/static".to_string(), route_with(&["GET"], Some("static")));
        let root = std::env::current_dir().unwrap();
        (config, root)
    }

    fn req(method: Method, target: &str) -> Request {
        Request::new(method, target.to_string(), Version::Http11)
    }

    // -----------------------------------------------------------------------
    // Pre-existing smoke test
    // -----------------------------------------------------------------------

    #[test]
    fn test_route_matching() {
        let (config, root) = create_test_config();
        let router = Router::new(&config, root);

        let request = req(Method::GET, "/static/file.html");
        assert!(router.match_route(&request).is_some());
    }

    // -----------------------------------------------------------------------
    // Exact, prefix, longest-prefix-wins, root fallback
    // (audit-required: "Test Route Matching logic")
    // -----------------------------------------------------------------------

    #[test]
    fn test_exact_match_beats_prefix_match() {
        let mut config = empty_server();
        config
            .routes
            .insert("/".to_string(), route_with(&["GET"], Some(".")));
        config.routes.insert(
            "/upload".to_string(),
            route_with(&["POST"], Some("uploads")),
        );

        let router = Router::new(&config, std::env::current_dir().unwrap());

        let (matched_path, _) = router
            .match_route_with_path(&req(Method::POST, "/upload"))
            .expect("must match");
        assert_eq!(matched_path, "/upload");
    }

    #[test]
    fn test_longest_prefix_wins() {
        let mut config = empty_server();
        config
            .routes
            .insert("/".to_string(), route_with(&["GET"], Some(".")));
        config
            .routes
            .insert("/api".to_string(), route_with(&["GET"], Some("api")));
        config
            .routes
            .insert("/api/v1".to_string(), route_with(&["GET"], Some("api_v1")));

        let router = Router::new(&config, std::env::current_dir().unwrap());

        let (matched_path, _) = router
            .match_route_with_path(&req(Method::GET, "/api/v1/users"))
            .expect("must match");
        assert_eq!(
            matched_path, "/api/v1",
            "longest matching prefix should win, got {}",
            matched_path
        );
    }

    #[test]
    fn test_prefix_must_be_followed_by_slash_or_end() {
        // "/upload" must NOT match "/uploads/file.txt" — that path has its
        // own boundary semantics.
        let mut config = empty_server();
        config.routes.insert(
            "/upload".to_string(),
            route_with(&["POST"], Some("uploads")),
        );
        config
            .routes
            .insert("/".to_string(), route_with(&["GET"], Some(".")));

        let router = Router::new(&config, std::env::current_dir().unwrap());

        let (matched_path, _) = router
            .match_route_with_path(&req(Method::GET, "/uploads/file.txt"))
            .expect("must fall through to /");
        assert_eq!(matched_path, "/", "expected fall-through to root route");
    }

    #[test]
    fn test_root_route_catches_unknown_paths() {
        let mut config = empty_server();
        config
            .routes
            .insert("/".to_string(), route_with(&["GET"], Some(".")));

        let router = Router::new(&config, std::env::current_dir().unwrap());

        assert!(router
            .match_route(&req(Method::GET, "/whatever/random/path"))
            .is_some());
    }

    #[test]
    fn test_no_match_when_no_root_route() {
        let mut config = empty_server();
        config
            .routes
            .insert("/api".to_string(), route_with(&["GET"], Some("api")));

        let router = Router::new(&config, std::env::current_dir().unwrap());

        assert!(router
            .match_route(&req(Method::GET, "/totally/unrelated"))
            .is_none());
    }

    // -----------------------------------------------------------------------
    // Method validation (audit-required: "list of accepted methods for a route")
    // -----------------------------------------------------------------------

    #[test]
    fn test_method_allowed_when_in_list() {
        let route = route_with(&["GET", "POST"], Some("."));
        let router = Router::new(&empty_server(), std::env::current_dir().unwrap());
        assert!(router.is_method_allowed(&req(Method::GET, "/x"), &route));
        assert!(router.is_method_allowed(&req(Method::POST, "/x"), &route));
    }

    #[test]
    fn test_method_rejected_when_not_in_list() {
        let route = route_with(&["GET"], Some("."));
        let router = Router::new(&empty_server(), std::env::current_dir().unwrap());
        assert!(!router.is_method_allowed(&req(Method::POST, "/x"), &route));
        assert!(!router.is_method_allowed(&req(Method::DELETE, "/x"), &route));
    }

    #[test]
    fn test_method_check_is_case_insensitive() {
        // Methods are persisted as strings in config, audit may compare e.g. "get".
        let route = RouteConfig {
            methods: vec!["get".to_string()],
            ..Default::default()
        };
        let router = Router::new(&empty_server(), std::env::current_dir().unwrap());
        assert!(router.is_method_allowed(&req(Method::GET, "/x"), &route));
    }

    #[test]
    fn test_validate_request_returns_405_for_wrong_method() {
        let mut config = empty_server();
        config
            .routes
            .insert("/only-get".to_string(), route_with(&["GET"], Some(".")));
        let router = Router::new(&config, std::env::current_dir().unwrap());

        let (_, response) = router
            .validate_request(&req(Method::DELETE, "/only-get"))
            .expect("route exists");
        let response = response.expect("expected 405 response");
        assert_eq!(response.status.as_u16(), 405);
    }

    #[test]
    fn test_validate_request_errors_when_no_route() {
        let config = empty_server(); // no routes
        let router = Router::new(&config, std::env::current_dir().unwrap());
        let result = router.validate_request(&req(Method::GET, "/missing"));
        assert!(result.is_err(), "missing route must surface as Err");
    }

    // -----------------------------------------------------------------------
    // Path sanitization (directory traversal protection)
    // -----------------------------------------------------------------------

    #[test]
    fn test_resolve_file_path_rejects_parent_dir() {
        let mut config = empty_server();
        config
            .routes
            .insert("/".to_string(), route_with(&["GET"], None));
        let router = Router::new(&config, PathBuf::from("/tmp"));

        let request = req(Method::GET, "/../../etc/passwd");
        let route = router.match_route(&request).unwrap().clone();
        let result = router.resolve_file_path(&request, &route);
        assert!(
            result.is_err(),
            "directory traversal via '..' must be rejected"
        );
    }
}
