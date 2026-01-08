use crate::application::handler::request_handler::RequestHandler;
use crate::application::handler::router::Router;
use crate::common::error::{Result, ServerError};
use crate::http::request::Request;
use crate::http::response::Response;
use crate::http::version::Version;
use std::fs;
use std::path::Path;

/// Handler for serving static files
pub struct StaticFileHandler {
    router: Router,
}

impl StaticFileHandler {
    /// Create a new static file handler
    pub fn new(router: Router) -> Self {
        Self { router }
    }

    /// Determine MIME type from file extension
    fn get_mime_type(&self, path: &Path) -> &'static str {
        if let Some(ext) = path.extension().and_then(|e| e.to_str()) {
            match ext.to_lowercase().as_str() {
                "html" | "htm" => "text/html",
                "css" => "text/css",
                "js" => "application/javascript",
                "json" => "application/json",
                "png" => "image/png",
                "jpg" | "jpeg" => "image/jpeg",
                "gif" => "image/gif",
                "svg" => "image/svg+xml",
                "ico" => "image/x-icon",
                "pdf" => "application/pdf",
                "txt" => "text/plain",
                "xml" => "application/xml",
                _ => "application/octet-stream",
            }
        } else {
            "application/octet-stream"
        }
    }
}

impl RequestHandler for StaticFileHandler {
    fn handle(&self, request: &Request) -> Result<Response> {
        // Validate route and method
        let (route, error_response) = self.router.validate_request(request)?;
        if let Some(response) = error_response {
            return Ok(response);
        }

        // Resolve file path
        let file_path = self.router.resolve_file_path(request, route)?;

        // Check if file exists
        if !file_path.exists() {
            return Err(ServerError::HttpError("File not found".to_string()));
        }

        // Check if it's a directory
        if file_path.is_dir() {
            // Check for default file
            if let Some(default_file) = self.router.get_default_file(route) {
                let default_path = file_path.join(default_file);
                if default_path.exists() && default_path.is_file() {
                    return self.serve_file(&default_path, request.version);
                }
            }

            // Directory listing should be handled by DirectoryListingHandler in server_manager
            // Return 403 if listing is disabled

            // Directory without listing - return 403
            return Ok(Response::forbidden_with_message(request.version, "Forbidden"));
        }

        // Serve the file
        self.serve_file(&file_path, request.version)
    }
}

impl StaticFileHandler {
    /// Serve a file
    fn serve_file(&self, path: &Path, version: Version) -> Result<Response> {
        let content = fs::read(path)
            .map_err(|e| ServerError::HttpError(format!("Failed to read file: {}", e)))?;

        let mut response = Response::ok(version);
        response.set_content_type(self.get_mime_type(path));
        response.set_body(content);

        Ok(response)
    }
}
