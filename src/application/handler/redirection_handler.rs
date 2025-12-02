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
        // Validate route and method
        let (route, error_response) = self.router.validate_request(request)?;
        if let Some(response) = error_response {
            return Ok(response);
        }

        // Check if route has redirect configured
        let redirect_target = route.redirect.as_ref()
            .ok_or_else(|| ServerError::HttpError("Route does not have redirect configured".to_string()))?;

        // Create 302 Found (temporary redirect) response
        // For permanent redirects (301), we could add a redirect_type field to RouteConfig
        let mut response = Response::found(request.version);
        
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

        response.set_location(&location);
        // Redirect responses have empty body
        response.set_body_str("");

        Ok(response)
    }
}
