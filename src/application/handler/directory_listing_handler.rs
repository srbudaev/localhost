use crate::application::handler::request_handler::RequestHandler;
use crate::application::handler::router::Router;
use crate::common::error::{Result, ServerError};
use crate::http::request::Request;
use crate::http::response::Response;
use std::fs;
use std::path::Path;

/// Handler for generating directory listings
pub struct DirectoryListingHandler {
    router: Router,
}

impl DirectoryListingHandler {
    /// Create a new directory listing handler
    pub fn new(router: Router) -> Self {
        Self { router }
    }

    /// Generate HTML directory listing
    fn generate_listing(&self, dir_path: &Path, request_path: &str) -> Result<String> {
        let mut html = String::from("<!DOCTYPE html>\n<html><head><title>Index of ");
        html.push_str(request_path);
        html.push_str("</title></head><body><h1>Index of ");
        html.push_str(request_path);
        html.push_str("</h1><hr><pre>");

        // Add parent directory link if not root
        if request_path != "/" {
            let parent_path = if let Some(pos) = request_path.rfind('/') {
                if pos == 0 {
                    "/"
                } else {
                    &request_path[..pos]
                }
            } else {
                "/"
            };
            html.push_str("<a href=\"");
            html.push_str(parent_path);
            html.push_str("\">../</a>\n");
        }

        // Read directory entries
        let entries = fs::read_dir(dir_path)
            .map_err(|e| ServerError::HttpError(format!("Failed to read directory: {}", e)))?;

        let mut entries: Vec<_> = entries
            .filter_map(|e| e.ok())
            .collect();

        // Sort entries: directories first, then files
        entries.sort_by(|a, b| {
            let a_is_dir = a.path().is_dir();
            let b_is_dir = b.path().is_dir();
            match (a_is_dir, b_is_dir) {
                (true, false) => std::cmp::Ordering::Less,
                (false, true) => std::cmp::Ordering::Greater,
                _ => a.file_name().cmp(&b.file_name()),
            }
        });

        // Generate listing
        for entry in entries {
            let path = entry.path();
            let name = entry.file_name();
            let name_str = name.to_string_lossy();
            let is_dir = path.is_dir();

            // Build URL
            let url = if request_path.ends_with('/') {
                format!("{}{}", request_path, name_str)
            } else {
                format!("{}/{}", request_path, name_str)
            };

            html.push_str("<a href=\"");
            html.push_str(&url);
            html.push_str("\">");
            html.push_str(&name_str);
            if is_dir {
                html.push_str("/");
            }
            html.push_str("</a>");

            // Add spacing for alignment
            let name_len = name_str.len();
            let padding = 50usize.saturating_sub(name_len);
            html.push_str(&" ".repeat(padding));

            // Add file size or directory indicator
            if is_dir {
                html.push_str("-");
            } else {
                if let Ok(metadata) = path.metadata() {
                    let size = metadata.len();
                    html.push_str(&format!("{}", size));
                } else {
                    html.push_str("-");
                }
            }

            html.push_str("\n");
        }

        html.push_str("</pre><hr></body></html>");
        Ok(html)
    }
}

impl RequestHandler for DirectoryListingHandler {
    fn handle(&self, request: &Request) -> Result<Response> {
        // Validate route and method
        let (route, error_response) = self.router.validate_request(request)?;
        if let Some(response) = error_response {
            return Ok(response);
        }

        // Resolve directory path
        let dir_path = self.router.resolve_file_path(request, route)?;

        // Verify it's a directory
        if !dir_path.is_dir() {
            let mut response = Response::not_found(request.version);
            response.set_body_str("Not Found");
            return Ok(response);
        }

        // Check if directory listing is enabled
        if !self.router.is_directory_listing_enabled(route) {
            let mut response = Response::forbidden(request.version);
            response.set_body_str("Directory listing is disabled");
            return Ok(response);
        }

        // Generate listing
        let html = self.generate_listing(&dir_path, request.path())?;

        let mut response = Response::ok(request.version);
        response.set_content_type("text/html");
        response.set_body_str(&html);

        Ok(response)
    }
}
