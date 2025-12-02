use crate::application::handler::request_handler::RequestHandler;
use crate::application::handler::router::Router;
use crate::common::error::{Result, ServerError};
use crate::http::method::Method;
use crate::http::request::Request;
use crate::http::response::Response;
use crate::http::status::StatusCode;
use std::fs;
use std::hash::{Hash, Hasher};
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

/// Handler for file uploads
pub struct UploadHandler {
    router: Router,
    upload_dir: PathBuf,
}

impl UploadHandler {
    /// Create a new upload handler
    pub fn new(router: Router, upload_dir: PathBuf) -> Self {
        Self { router, upload_dir }
    }

    /// Generate unique filename for uploaded file
    fn generate_filename(&self, content: &[u8]) -> String {
        let timestamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();
        
        // Use timestamp + hash of content for uniqueness
        let mut hasher = std::collections::hash_map::DefaultHasher::new();
        content.hash(&mut hasher);
        let hash = hasher.finish();
        
        format!("upload_{}_{:x}", timestamp, hash & 0xFFFF)
    }

    /// Save uploaded file
    fn save_file(&self, content: &[u8], original_filename: Option<&str>) -> Result<PathBuf> {
        // Ensure upload directory exists
        if !self.upload_dir.exists() {
            fs::create_dir_all(&self.upload_dir)
                .map_err(|e| ServerError::HttpError(format!(
                    "Failed to create upload directory: {}", e
                )))?;
        }

        // Generate filename
        let filename = if let Some(orig_name) = original_filename {
            // Try to preserve original extension if possible
            if let Some(ext) = Path::new(orig_name).extension().and_then(|e| e.to_str()) {
                format!("{}.{}", self.generate_filename(content), ext)
            } else {
                self.generate_filename(content)
            }
        } else {
            self.generate_filename(content)
        };

        let file_path = self.upload_dir.join(&filename);

        // Write file
        fs::write(&file_path, content)
            .map_err(|e| ServerError::HttpError(format!(
                "Failed to write uploaded file: {}", e
            )))?;

        Ok(file_path)
    }
}

impl RequestHandler for UploadHandler {
    fn handle(&self, request: &Request) -> Result<Response> {
        // Only POST requests are allowed for uploads
        if request.method != Method::POST {
            return Ok(Response::method_not_allowed_with_message(
                request.version,
                "Only POST method is allowed for file uploads"
            ));
        }

        // Validate route and method
        let (route, error_response) = self.router.validate_request(request)?;
        if let Some(response) = error_response {
            return Ok(response);
        }

        // Check if upload directory is configured
        if route.upload_dir.is_none() {
            return Ok(Response::bad_request_with_message(
                request.version,
                "Upload directory not configured for this route"
            ));
        }

        // Check if body is empty
        if request.body.is_empty() {
            return Ok(Response::bad_request_with_message(
                request.version,
                "No file data provided"
            ));
        }

        // Extract filename from Content-Disposition header if present
        let filename = request.headers
            .get("Content-Disposition")
            .and_then(|header| {
                // Parse Content-Disposition: form-data; name="file"; filename="example.txt"
                header
                    .find("filename=")
                    .map(|pos| {
                        let start = pos + 9; // "filename=".len()
                        let value = &header[start..];
                        // Remove quotes if present
                        value.trim_matches('"').trim_matches('\'').to_string()
                    })
            });

        // Save uploaded file
        let saved_path = self.save_file(&request.body, filename.as_deref())?;

        // Return success response
        let mut response = Response::new(request.version, StatusCode::CREATED);
        response.set_content_type("application/json");
        
        let json_response = format!(
            r#"{{"status": "success", "message": "File uploaded successfully", "filename": "{}"}}"#,
            saved_path.file_name()
                .and_then(|n| n.to_str())
                .unwrap_or("unknown")
        );
        response.set_body_str(&json_response);

        Ok(response)
    }
}
