use crate::application::config::models::ServerConfig;
use crate::common::error::Result;
use crate::http::response::Response;
use crate::http::status::StatusCode;
use crate::http::version::Version;
use std::fs;
use std::path::PathBuf;

/// Handler for custom error pages
pub struct ErrorPageHandler {
    root_path: PathBuf,
    error_pages: std::collections::HashMap<String, String>,
}

impl ErrorPageHandler {
    /// Resolve error page path - helper to reduce redundancy with router's resolve_path
    fn resolve_error_path(root_path: &PathBuf, path: &str) -> PathBuf {
        if path.starts_with('/') || path.starts_with("./") {
            PathBuf::from(path)
        } else {
            root_path.join(path)
        }
    }

    /// Create a new error page handler from server configuration
    pub fn new(config: &ServerConfig, root_path: PathBuf) -> Self {
        let error_pages: std::collections::HashMap<String, String> = config
            .errors
            .iter()
            .filter_map(|(code, error_config)| {
                // Only include error pages with filenames
                error_config.filename.as_ref().map(|filename| (code.clone(), filename.clone()))
            })
            .collect();

        Self {
            root_path,
            error_pages,
        }
    }

    /// Generate error response with custom error page
    /// Returns the correct HTTP status code (404, 403, etc.) with custom error page if configured
    pub fn generate_error_response(
        &self,
        status_code: StatusCode,
        version: Version,
    ) -> Result<Response> {
        let status_str = status_code.as_u16().to_string();

        // Try to find custom error page
        if let Some(error_file) = self.error_pages.get(&status_str) {
            let error_path = Self::resolve_error_path(&self.root_path, error_file);

            // Try to read custom error page
            if crate::common::path_utils::is_valid_file(&error_path) {
                match fs::read(&error_path) {
                    Ok(content) => {
                        return Ok(Self::create_html_response(version, status_code, content));
                    }
                    Err(_) => {
                        // If file read fails, fall through to default error message
                    }
                }
            }
        }

        // Fall back to default error message
        self.generate_default_error_response(status_code, version)
    }

    /// Create HTML response with content (helper to reduce redundancy)
    fn create_html_response(version: Version, status_code: StatusCode, content: Vec<u8>) -> Response {
        let mut response = Response::new(version, status_code);
        response.set_content_type("text/html");
        response.set_body(content);
        response
    }

    /// Generate default error response with standard message
    fn generate_default_error_response(
        &self,
        status_code: StatusCode,
        version: Version,
    ) -> Result<Response> {
        let mut response = Response::new(version, status_code);
        response.set_content_type("text/html");

        // Generate simple HTML error page
        let html = format!(
            r#"<!DOCTYPE html>
<html lang="en">
<head>
    <meta charset="UTF-8">
    <meta name="viewport" content="width=device-width, initial-scale=1.0">
    <title>{} {}</title>
    <style>
        body {{
            font-family: Arial, sans-serif;
            max-width: 600px;
            margin: 50px auto;
            padding: 20px;
            background-color: #f5f5f5;
        }}
        h1 {{
            color: #333;
            border-bottom: 2px solid #333;
            padding-bottom: 10px;
        }}
        .error-code {{
            font-size: 72px;
            font-weight: bold;
            color: #666;
            margin: 0;
        }}
        .error-message {{
            color: #666;
            margin-top: 10px;
        }}
    </style>
</head>
<body>
    <h1 class="error-code">{}</h1>
    <p class="error-message">{}</p>
</body>
</html>"#,
            status_code.as_u16(),
            status_code.reason_phrase(),
            status_code.as_u16(),
            status_code.reason_phrase()
        );

        response.set_body_str(&html);
        Ok(response)
    }
}
