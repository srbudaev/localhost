use crate::application::handler::request_handler::RequestHandler;
use crate::application::handler::router::Router;
use crate::common::error::{Result, ServerError};
use crate::http::request::Request;
use crate::http::response::Response;

/// Handler for HTTP redirects (301/302)
pub struct RedirectionHandler {
    router: Router,
}

impl RedirectionHandler {
    /// Create a new redirection handler
    pub fn new(router: Router) -> Self {
        Self { router }
    }
}

impl RequestHandler for RedirectionHandler {
    fn handle(&self, request: &Request) -> Result<Response> {
        crate::common::logger::Logger::info(&format!(
            "🔀 RedirectionHandler.handle() called for path='{}'",
            request.path()
        ));
        
        // Validate route and method - this will match the route again
        let (route, error_response) = self.router.validate_request(request)?;
        if let Some(response) = error_response {
            crate::common::logger::Logger::warn(&format!(
                "RedirectionHandler: validate_request returned error response"
            ));
            return Ok(response);
        }

        // Get the matched route path for logging BEFORE accessing redirect
        let matched_path = self.router.match_route_with_path(request)
            .map(|(path, route_config)| {
                (path.as_str(), route_config.redirect.as_ref())
            })
            .unwrap_or(("(unknown)", None));
        
        crate::common::logger::Logger::info(&format!(
            "RedirectionHandler: Matched route='{}', redirect from route={:?}",
            matched_path.0,
            matched_path.1
        ));

        // Check if route has redirect configured
        let redirect_target = route.redirect.as_ref()
            .ok_or_else(|| {
                crate::common::logger::Logger::error(&format!(
                    "RedirectionHandler: Route '{}' does not have redirect configured!",
                    matched_path.0
                ));
                ServerError::HttpError("Route does not have redirect configured".to_string())
            })?;
        
        // Debug logging: show what redirect value we're using
        crate::common::logger::Logger::info(&format!(
            "✓ RedirectionHandler: request path='{}', matched route='{}', redirect_target='{}'",
            request.path(),
            matched_path.0,
            redirect_target
        ));
        
        // Safety check: warn if the matched route doesn't match the request path
        // (This shouldn't happen with proper route matching, but helps debug issues)
        if matched_path.0 != request.path() && !request.path().starts_with(matched_path.0) {
            crate::common::logger::Logger::warn(&format!(
                "⚠ RedirectionHandler: Route mismatch! Request path '{}' matched route '{}'",
                request.path(),
                matched_path.0
            ));
        }

        // Determine redirect type: 301 (permanent) or 302 (temporary, default)
        let redirect_type = route.redirect_type.as_deref().unwrap_or("302");
        let mut response = if redirect_type == "301" {
            Response::moved_permanently(request.version)
        } else {
            Response::found(request.version)
        };
        
        // Set Location header
        // If redirect is relative, make it absolute based on request path
        let location: String = if redirect_target.starts_with("http://") || redirect_target.starts_with("https://") {
            // Absolute URL - use as is
            redirect_target.clone()
        } else if redirect_target.starts_with('/') {
            // Absolute path - use as is
            redirect_target.clone()
        } else {
            // Relative path - resolve relative to current request path
            let request_path = request.path();
            let base_path = request_path.rfind('/')
                .map(|pos| &request_path[..=pos])
                .unwrap_or("/");
            format!("{}{}", base_path, redirect_target)
        };

        // Log redirect information for debugging
        crate::common::logger::Logger::info(&format!(
            "🎯 FINAL REDIRECT: {} {} -> {} (type: {})",
            request.method,
            request.path(),
            location,
            redirect_type
        ));

        response.set_location(&location);
        // Redirect responses have empty body
        response.set_body_str("");

        Ok(response)
    }
}
