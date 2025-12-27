use crate::application::handler::request_handler::RequestHandler;
use crate::application::handler::router::Router;
use crate::common::error::{Result, ServerError};
use crate::http::method::Method;
use crate::http::request::Request;
use crate::http::response::Response;
use crate::http::status::StatusCode;
use std::fs;
use std::path::Path;

/// Handler for DELETE requests - safely deletes files
pub struct DeleteHandler {
    router: Router,
}

impl DeleteHandler {
    /// Create a new DELETE handler
    pub fn new(router: Router) -> Self {
        Self { router }
    }

    /// Safely delete a file
    fn delete_file(&self, file_path: &Path, version: crate::http::version::Version) -> Result<Response> {
        // Check if file exists
        if !file_path.exists() {
            return Ok(Response::not_found_with_message(version, "File not found"));
        }

        // Check if it's a directory (DELETE should not delete directories)
        if file_path.is_dir() {
            return Ok(Response::forbidden_with_message(version, "Cannot delete directory"));
        }

        // Check if it's a file
        if !file_path.is_file() {
            return Ok(Response::forbidden_with_message(version, "Path is not a file"));
        }

        // Attempt to delete the file
        match fs::remove_file(file_path) {
            Ok(_) => {
                // Successfully deleted - return 200 OK or 204 No Content
                let mut response = Response::ok(version);
                response.set_body_str("File deleted successfully");
                Ok(response)
            }
            Err(e) => {
                // Error deleting file
                match e.kind() {
                    std::io::ErrorKind::PermissionDenied => {
                        Ok(Response::forbidden_with_message(version, "Permission denied"))
                    }
                    std::io::ErrorKind::NotFound => {
                        Ok(Response::not_found_with_message(version, "File not found"))
                    }
                    _ => {
                        Ok(Response::internal_error_with_message(
                            version,
                            &format!("Failed to delete file: {}", e)
                        ))
                    }
                }
            }
        }
    }
}

impl RequestHandler for DeleteHandler {
    fn handle(&self, request: &Request) -> Result<Response> {
        // Only DELETE requests are allowed
        if request.method != Method::DELETE {
            return Ok(Response::method_not_allowed_with_message(
                request.version,
                "Only DELETE method is allowed"
            ));
        }

        // Validate route and method
        let (route, error_response) = self.router.validate_request(request)?;
        if let Some(response) = error_response {
            return Ok(response);
        }

        // Resolve file path
        let file_path = self.router.resolve_file_path(request, route)?;

        // Delete the file
        self.delete_file(&file_path, request.version)
    }
}


