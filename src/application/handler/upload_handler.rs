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

    /// Parse multipart/form-data body to extract file content, filename, and MIME type
    fn parse_multipart_body(&self, body: &[u8], content_type: &str) -> Result<(Vec<u8>, Option<String>, Option<String>)> {
        // Extract boundary from Content-Type header
        // Content-Type: multipart/form-data; boundary=----WebKitFormBoundary7MA4YWxkTrZu0gW
        let boundary_str = content_type
            .find("boundary=")
            .map(|pos| {
                let start = pos + 9; // "boundary=".len()
                let value = &content_type[start..];
                // Remove quotes if present and trim
                value.trim_matches('"').trim_matches('\'').trim().to_string()
            })
            .ok_or_else(|| ServerError::HttpError("No boundary found in multipart Content-Type".to_string()))?;
        
        let boundary = format!("--{}", boundary_str);
        let boundary_bytes = boundary.as_bytes();
        
        // Split body by boundary to find parts
        let mut parts = Vec::new();
        let mut start = 0;
        
        while let Some(boundary_pos) = body[start..].windows(boundary_bytes.len())
            .position(|window| window == boundary_bytes)
            .map(|pos| start + pos)
        {
            if start < boundary_pos {
                parts.push(&body[start..boundary_pos]);
            }
            start = boundary_pos + boundary_bytes.len();
            
            // Skip CRLF after boundary
            if start < body.len() && body[start] == b'\r' {
                start += 1;
            }
            if start < body.len() && body[start] == b'\n' {
                start += 1;
            }
        }
        
        // Process each part to find file content
        for part in parts {
            // Find Content-Disposition header
            if let Some(disposition_pos) = part.windows(b"Content-Disposition:".len())
                .position(|window| window == b"Content-Disposition:")
            {
                let disposition_section = &part[disposition_pos..];
                
                // Extract filename from Content-Disposition
                let filename = if let Some(filename_pos) = disposition_section.windows(b"filename=".len())
                    .position(|window| window == b"filename=")
                {
                    let filename_start = filename_pos + 9; // "filename=".len()
                    let filename_section = &disposition_section[filename_start..];
                    
                    // Find the filename value (between quotes or until semicolon/newline)
                    let mut filename_end = filename_section.len();
                    for (i, &byte) in filename_section.iter().enumerate() {
                        if byte == b'"' || byte == b'\'' || byte == b';' || byte == b'\r' || byte == b'\n' {
                            filename_end = i;
                            break;
                        }
                    }
                    
                    let filename_bytes = &filename_section[..filename_end];
                    let filename_str = String::from_utf8_lossy(filename_bytes)
                        .trim_matches('"')
                        .trim_matches('\'')
                        .trim()
                        .to_string();
                    
                    if !filename_str.is_empty() {
                        Some(filename_str)
                    } else {
                        None
                    }
                } else {
                    None
                };
                
                // Extract Content-Type (MIME type) from this part
                let mime_type = if let Some(content_type_pos) = part.windows(b"Content-Type:".len())
                    .position(|window| window == b"Content-Type:")
                {
                    let content_type_section = &part[content_type_pos + 13..]; // Skip "Content-Type:"
                    // Find the end of Content-Type value (until \r or \n)
                    let mut mime_end = content_type_section.len();
                    for (i, &byte) in content_type_section.iter().enumerate() {
                        if byte == b'\r' || byte == b'\n' {
                            mime_end = i;
                            break;
                        }
                    }
                    let mime_bytes = &content_type_section[..mime_end];
                    let mime_str = String::from_utf8_lossy(mime_bytes).trim().to_string();
                    if !mime_str.is_empty() {
                        Some(mime_str)
                    } else {
                        None
                    }
                } else {
                    None
                };
                
                // Find content start (after \r\n\r\n)
                if let Some(content_sep_pos) = part.windows(b"\r\n\r\n".len())
                    .position(|window| window == b"\r\n\r\n")
                {
                    let content_start = content_sep_pos + 4; // Skip \r\n\r\n
                    let mut content_end = part.len();
                    
                    // Remove trailing CRLF before next boundary
                    if content_end >= 2 && part[content_end - 2..] == b"\r\n"[..] {
                        content_end -= 2;
                    } else if content_end >= 1 && part[content_end - 1] == b'\n' {
                        content_end -= 1;
                    }
                    
                    let content = part[content_start..content_end].to_vec();
                    return Ok((content, filename, mime_type));
                }
            }
        }
        
        // If no file part found, return the whole body
        Ok((body.to_vec(), None, None))
    }

    /// Detect MIME type from file extension
    fn detect_mime_type_from_filename(&self, filename: &str) -> Option<String> {
        Path::new(filename)
            .extension()
            .and_then(|ext| ext.to_str())
            .map(|ext| {
                match ext.to_lowercase().as_str() {
                    "html" | "htm" => "text/html",
                    "css" => "text/css",
                    "js" => "application/javascript",
                    "json" => "application/json",
                    "png" => "image/png",
                    "jpg" | "jpeg" => "image/jpeg",
                    "gif" => "image/gif",
                    "webp" => "image/webp",
                    "svg" => "image/svg+xml",
                    "ico" => "image/x-icon",
                    "pdf" => "application/pdf",
                    "txt" => "text/plain",
                    "xml" => "application/xml",
                    "zip" => "application/zip",
                    "mp4" => "video/mp4",
                    "mp3" => "audio/mpeg",
                    _ => "application/octet-stream",
                }.to_string()
            })
    }

    /// Get file extension from MIME type
    fn get_extension_from_mime_type(&self, mime_type: &str) -> Option<String> {
        // Normalize MIME type (remove parameters like charset)
        let mime = mime_type.split(';').next().unwrap_or(mime_type).trim();
        
        match mime {
            "text/html" => Some("html".to_string()),
            "text/css" => Some("css".to_string()),
            "application/javascript" | "text/javascript" => Some("js".to_string()),
            "application/json" => Some("json".to_string()),
            "image/png" => Some("png".to_string()),
            "image/jpeg" => Some("jpg".to_string()),
            "image/gif" => Some("gif".to_string()),
            "image/webp" => Some("webp".to_string()),
            "image/svg+xml" => Some("svg".to_string()),
            "image/x-icon" | "image/vnd.microsoft.icon" => Some("ico".to_string()),
            "application/pdf" => Some("pdf".to_string()),
            "text/plain" => Some("txt".to_string()),
            "application/xml" | "text/xml" => Some("xml".to_string()),
            "application/zip" => Some("zip".to_string()),
            "video/mp4" => Some("mp4".to_string()),
            "audio/mpeg" | "audio/mp3" => Some("mp3".to_string()),
            _ => None,
        }
    }

    /// Validate if MIME type is allowed
    fn is_valid_mime_type(&self, mime_type: &str) -> bool {
        // Normalize MIME type (remove parameters like charset)
        let mime = mime_type.split(';').next().unwrap_or(mime_type).trim();
        
        // Whitelist of allowed MIME types
        let allowed_types = [
            // Text types
            "text/html",
            "text/css",
            "text/plain",
            "text/xml",
            // Application types
            "application/javascript",
            "text/javascript",
            "application/json",
            "application/pdf",
            "application/xml",
            "application/zip",
            // Image types
            "image/png",
            "image/jpeg",
            "image/gif",
            "image/webp",
            "image/svg+xml",
            "image/x-icon",
            "image/vnd.microsoft.icon",
            // Video types
            "video/mp4",
            // Audio types
            "audio/mpeg",
            "audio/mp3",
        ];
        
        allowed_types.contains(&mime)
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

    /// Save uploaded file and optionally store MIME type metadata
    /// Ensures the file extension matches the MIME type
    fn save_file(&self, content: &[u8], original_filename: Option<&str>, mime_type: Option<&str>) -> Result<PathBuf> {
        // Ensure upload directory exists
        if !self.upload_dir.exists() {
            fs::create_dir_all(&self.upload_dir)
                .map_err(|e| ServerError::HttpError(format!(
                    "Failed to create upload directory: {}", e
                )))?;
        }

        // Determine the correct file extension based on MIME type
        let correct_extension = mime_type.and_then(|mime| self.get_extension_from_mime_type(mime));

        // Use original filename if provided, otherwise generate one
        let filename = if let Some(orig_name) = original_filename {
            // Sanitize filename to prevent directory traversal
            let sanitized = Path::new(orig_name)
                .file_name()
                .and_then(|n| n.to_str())
                .unwrap_or(orig_name);
            
            // Get the base name and current extension
            let base_name = Path::new(sanitized)
                .file_stem()
                .and_then(|s| s.to_str())
                .unwrap_or(sanitized);
            
            let current_ext = Path::new(sanitized)
                .extension()
                .and_then(|e| e.to_str());
            
            // Use correct extension from MIME type if available, otherwise use original extension
            let final_ext = correct_extension.as_deref().or(current_ext);
            
            // Build final filename with correct extension
            let mut final_name = if let Some(ext) = final_ext {
                format!("{}.{}", base_name, ext)
            } else {
                base_name.to_string()
            };
            
            // Check if file already exists, append number if needed
            let mut counter = 1;
            while self.upload_dir.join(&final_name).exists() {
                if let Some(ext) = final_ext {
                    final_name = format!("{}_{}.{}", base_name, counter, ext);
                } else {
                    final_name = format!("{}_{}", base_name, counter);
                }
                counter += 1;
            }
            final_name
        } else {
            // Generate unique filename with correct extension
            let base_name = self.generate_filename(content);
            if let Some(ext) = &correct_extension {
                format!("{}.{}", base_name, ext)
            } else {
                base_name
            }
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

        // Parse multipart/form-data if Content-Type indicates it
        let (file_content, filename, mime_type) = if let Some(content_type) = request.content_type() {
            if content_type.starts_with("multipart/form-data") {
                // Parse multipart body to extract file content, filename, and MIME type
                self.parse_multipart_body(&request.body, content_type)?
            } else {
                // Not multipart - use body as-is and try to get filename from Content-Disposition header
                let filename = request.headers
                    .get("Content-Disposition")
                    .and_then(|header| {
                        header
                            .find("filename=")
                            .map(|pos| {
                                let start = pos + 9; // "filename=".len()
                                let value = &header[start..];
                                // Remove quotes and any trailing content
                                let value = value.split(';').next().unwrap_or(value);
                                value.trim_matches('"').trim_matches('\'').trim().to_string()
                            })
                    });
                // Get MIME type from Content-Type header if not multipart
                let mime = if content_type != "multipart/form-data" {
                    Some(content_type.clone())
                } else {
                    None
                };
                (request.body.clone(), filename, mime)
            }
        } else {
            // No Content-Type - use body as-is
            (request.body.clone(), None, None)
        };

        // Detect MIME type from file extension if not provided
        let final_mime_type = mime_type.or_else(|| {
            filename.as_ref().and_then(|name| {
                self.detect_mime_type_from_filename(name)
            })
        });

        // Validate MIME type if provided
        if let Some(ref mime) = final_mime_type {
            if !self.is_valid_mime_type(mime) {
                return Ok(Response::bad_request_with_message(
                    request.version,
                    &format!("Invalid or unsupported MIME type: {}", mime)
                ));
            }
        } else {
            // If no MIME type could be determined, reject the upload
            return Ok(Response::bad_request_with_message(
                request.version,
                "Unable to determine file type. Please ensure Content-Type header is set or file has a recognized extension."
            ));
        }

        // Save uploaded file
        let saved_path = self.save_file(&file_content, filename.as_deref(), final_mime_type.as_deref())?;

        // Return success response
        let mut response = Response::new(request.version, StatusCode::CREATED);
        response.set_content_type("application/json");
        
        // Build JSON response with filename and MIME type
        let filename_str = saved_path.file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("unknown");
        
        let json_response = if let Some(mime) = final_mime_type {
            format!(
                r#"{{"status": "success", "message": "File uploaded successfully", "filename": "{}", "mime_type": "{}"}}"#,
                filename_str, mime
            )
        } else {
            format!(
                r#"{{"status": "success", "message": "File uploaded successfully", "filename": "{}"}}"#,
                filename_str
            )
        };
        response.set_body_str(&json_response);

        Ok(response)
    }
}
