use std::fmt;

/// HTTP status code as defined in RFC 9112
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct StatusCode(u16);

impl StatusCode {
    /// Create a new status code
    pub fn new(code: u16) -> Option<Self> {
        if code >= 100 && code <= 599 {
            Some(StatusCode(code))
        } else {
            None
        }
    }

    /// Get the numeric value
    pub fn as_u16(&self) -> u16 {
        self.0
    }

    /// Check if status is informational (1xx)
    pub fn is_informational(&self) -> bool {
        self.0 >= 100 && self.0 < 200
    }

    /// Check if status is successful (2xx)
    pub fn is_success(&self) -> bool {
        self.0 >= 200 && self.0 < 300
    }

    /// Check if status is redirection (3xx)
    pub fn is_redirection(&self) -> bool {
        self.0 >= 300 && self.0 < 400
    }

    /// Check if status is client error (4xx)
    pub fn is_client_error(&self) -> bool {
        self.0 >= 400 && self.0 < 500
    }

    /// Check if status is server error (5xx)
    pub fn is_server_error(&self) -> bool {
        self.0 >= 500 && self.0 < 600
    }

    /// Check if status allows response body
    pub fn allows_body(&self) -> bool {
        // HEAD requests and 1xx, 204, 304 responses don't have body
        !self.is_informational() && self.0 != 204 && self.0 != 304
    }

    /// Get reason phrase for common status codes
    pub fn reason_phrase(&self) -> &'static str {
        match self.0 {
            200 => "OK",
            201 => "Created",
            204 => "No Content",
            301 => "Moved Permanently",
            302 => "Found",
            304 => "Not Modified",
            400 => "Bad Request",
            403 => "Forbidden",
            404 => "Not Found",
            405 => "Method Not Allowed",
            413 => "Payload Too Large",
            500 => "Internal Server Error",
            501 => "Not Implemented",
            502 => "Bad Gateway",
            503 => "Service Unavailable",
            504 => "Gateway Timeout",
            _ => "Unknown",
        }
    }
}

impl fmt::Display for StatusCode {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

// Common status codes
impl StatusCode {
    pub const OK: StatusCode = StatusCode(200);
    pub const CREATED: StatusCode = StatusCode(201);
    pub const NO_CONTENT: StatusCode = StatusCode(204);
    pub const MOVED_PERMANENTLY: StatusCode = StatusCode(301);
    pub const FOUND: StatusCode = StatusCode(302);
    pub const NOT_MODIFIED: StatusCode = StatusCode(304);
    pub const BAD_REQUEST: StatusCode = StatusCode(400);
    pub const FORBIDDEN: StatusCode = StatusCode(403);
    pub const NOT_FOUND: StatusCode = StatusCode(404);
    pub const METHOD_NOT_ALLOWED: StatusCode = StatusCode(405);
    pub const PAYLOAD_TOO_LARGE: StatusCode = StatusCode(413);
    pub const INTERNAL_SERVER_ERROR: StatusCode = StatusCode(500);
    pub const NOT_IMPLEMENTED: StatusCode = StatusCode(501);
    pub const BAD_GATEWAY: StatusCode = StatusCode(502);
    pub const SERVICE_UNAVAILABLE: StatusCode = StatusCode(503);
    pub const GATEWAY_TIMEOUT: StatusCode = StatusCode(504);
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::http::response::Response;
    use crate::http::version::Version;

    // -----------------------------------------------------------------------
    // Raw StatusCode behaviour
    // -----------------------------------------------------------------------

    #[test]
    fn test_status_code_creation() {
        assert!(StatusCode::new(200).is_some());
        assert!(StatusCode::new(404).is_some());
        assert!(StatusCode::new(500).is_some());
        assert!(StatusCode::new(99).is_none());
        assert!(StatusCode::new(600).is_none());
    }

    #[test]
    fn test_status_code_categories() {
        assert!(StatusCode::OK.is_success());
        assert!(!StatusCode::OK.is_client_error());

        assert!(StatusCode::NOT_FOUND.is_client_error());
        assert!(!StatusCode::NOT_FOUND.is_success());

        assert!(StatusCode::INTERNAL_SERVER_ERROR.is_server_error());

        assert!(StatusCode::FOUND.is_redirection());
        assert!(StatusCode::MOVED_PERMANENTLY.is_redirection());
    }

    #[test]
    fn test_status_code_display() {
        assert_eq!(StatusCode::OK.to_string(), "200");
        assert_eq!(StatusCode::NOT_FOUND.to_string(), "404");
        assert_eq!(StatusCode::PAYLOAD_TOO_LARGE.to_string(), "413");
    }

    #[test]
    fn test_reason_phrase() {
        assert_eq!(StatusCode::OK.reason_phrase(), "OK");
        assert_eq!(StatusCode::NOT_FOUND.reason_phrase(), "Not Found");
        assert_eq!(
            StatusCode::INTERNAL_SERVER_ERROR.reason_phrase(),
            "Internal Server Error"
        );
        assert_eq!(
            StatusCode::PAYLOAD_TOO_LARGE.reason_phrase(),
            "Payload Too Large"
        );
        assert_eq!(
            StatusCode::METHOD_NOT_ALLOWED.reason_phrase(),
            "Method Not Allowed"
        );
        assert_eq!(StatusCode::FORBIDDEN.reason_phrase(), "Forbidden");
    }

    #[test]
    fn test_allows_body_for_no_content_and_not_modified() {
        assert!(!StatusCode::NO_CONTENT.allows_body());
        assert!(!StatusCode::NOT_MODIFIED.allows_body());
        assert!(StatusCode::OK.allows_body());
        assert!(StatusCode::NOT_FOUND.allows_body());
    }

    // -----------------------------------------------------------------------
    // Audit-required: status code GENERATION via Response constructors
    // ("Verify Status Code Generation by ensuring the internal logic
    //  returns the correct 4xx or 5xx codes for malformed requests or
    //  missing files before the response is sent")
    // -----------------------------------------------------------------------

    #[test]
    fn test_response_bad_request_is_400() {
        let r = Response::bad_request_with_message(Version::Http11, "malformed");
        assert_eq!(r.status, StatusCode::BAD_REQUEST);
        assert_eq!(r.status.as_u16(), 400);
    }

    #[test]
    fn test_response_forbidden_is_403() {
        let r = Response::forbidden_with_message(Version::Http11, "no");
        assert_eq!(r.status, StatusCode::FORBIDDEN);
        assert_eq!(r.status.as_u16(), 403);
    }

    #[test]
    fn test_response_not_found_is_404() {
        let r = Response::not_found_with_message(Version::Http11, "missing");
        assert_eq!(r.status, StatusCode::NOT_FOUND);
        assert_eq!(r.status.as_u16(), 404);
    }

    #[test]
    fn test_response_method_not_allowed_is_405() {
        let r = Response::method_not_allowed_with_message(Version::Http11, "nope");
        assert_eq!(r.status, StatusCode::METHOD_NOT_ALLOWED);
        assert_eq!(r.status.as_u16(), 405);
    }

    #[test]
    fn test_response_payload_too_large_can_be_constructed() {
        // 413 is built explicitly when body exceeds client_max_body_size.
        let r = Response::new(Version::Http11, StatusCode::PAYLOAD_TOO_LARGE);
        assert_eq!(r.status.as_u16(), 413);
        assert!(r.status.is_client_error());
    }

    #[test]
    fn test_response_internal_error_is_500() {
        let r = Response::internal_error_with_message(Version::Http11, "boom");
        assert_eq!(r.status, StatusCode::INTERNAL_SERVER_ERROR);
        assert_eq!(r.status.as_u16(), 500);
    }

    #[test]
    fn test_all_required_error_codes_have_reason_phrases() {
        // Spec mandates default pages for [400, 403, 404, 405, 413, 500].
        // Every one must have a non-"Unknown" reason phrase so that the
        // serializer can emit a valid status line.
        for code in [400u16, 403, 404, 405, 413, 500] {
            let sc = StatusCode::new(code).expect("constructible");
            assert_ne!(
                sc.reason_phrase(),
                "Unknown",
                "missing reason phrase for {}",
                code
            );
        }
    }
}
