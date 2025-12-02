use crate::application::cgi::CgiExecutor;
use crate::application::config::models::ServerConfig;
use crate::application::handler::request_handler::RequestHandler;
use crate::application::handler::router::Router;
use crate::common::constants::DEFAULT_REQUEST_TIMEOUT_SECS;
use crate::common::error::{Result, ServerError};
use crate::http::request::Request;
use crate::http::response::Response;
use crate::http::status::StatusCode;
use std::path::PathBuf;

/// Handler for executing CGI scripts
pub struct CgiHandler {
    router: Router,
    executor: CgiExecutor,
    server_config: ServerConfig,
    server_port: u16,
}

impl CgiHandler {
    /// Create a new CGI handler
    pub fn new(router: Router, server_config: ServerConfig, server_port: u16) -> Self {
        let executor = CgiExecutor::new(DEFAULT_REQUEST_TIMEOUT_SECS);
        Self {
            router,
            executor,
            server_config,
            server_port,
        }
    }

    /// Determine interpreter for script based on extension
    fn get_interpreter(&self, script_path: &PathBuf) -> Option<&String> {
        if let Some(ext) = script_path.extension().and_then(|e| e.to_str()) {
            self.server_config.cgi_handlers.get(ext)
        } else {
            None
        }
    }

    /// Check if file is a CGI script based on route configuration
    fn is_cgi_script(&self, route: &crate::application::config::models::RouteConfig, file_path: &PathBuf) -> bool {
        // Check if route has CGI extension configured
        if let Some(ref cgi_ext) = route.cgi_extension {
            if let Some(file_ext) = file_path.extension().and_then(|e| e.to_str()) {
                return file_ext == cgi_ext;
            }
        }

        // Check if file extension matches any configured CGI handler
        if let Some(ext) = file_path.extension().and_then(|e| e.to_str()) {
            return self.server_config.cgi_handlers.contains_key(ext);
        }

        false
    }
}

impl RequestHandler for CgiHandler {
    fn handle(&self, request: &Request) -> Result<Response> {
        // Validate route and method
        let (route, error_response) = self.router.validate_request(request)?;
        if let Some(response) = error_response {
            return Ok(response);
        }

        // Resolve script path
        let script_path = self.router.resolve_file_path(request, route)?;

        // Verify script exists
        if !script_path.exists() {
            let mut response = Response::not_found(request.version);
            response.set_body_str("CGI script not found");
            return Ok(response);
        }

        // Check if this is a CGI script
        if !self.is_cgi_script(route, &script_path) {
            let mut response = Response::forbidden(request.version);
            response.set_body_str("Not a CGI script");
            return Ok(response);
        }

        // Get interpreter for script
        let interpreter = self.get_interpreter(&script_path);

        // Execute CGI script
        match self.executor.execute(
            script_path,
            interpreter.map(|s| s.as_str()),
            request,
            &self.server_config.server_name,
            self.server_port,
        ) {
            Ok(response) => Ok(response),
            Err(ServerError::CgiError(msg)) => {
                let mut response = Response::internal_error(request.version);
                response.set_body_str(&format!("CGI Error: {}", msg));
                Ok(response)
            }
            Err(ServerError::TimeoutError(msg)) => {
                let mut response = Response::new(request.version, StatusCode::GATEWAY_TIMEOUT);
                response.set_body_str(&format!("CGI Timeout: {}", msg));
                Ok(response)
            }
            Err(e) => Err(e),
        }
    }
}
