use crate::common::error::Result;
use crate::http::request::Request;
use crate::http::response::Response;

/// Trait for handling HTTP requests
pub trait RequestHandler {
    /// Handle an HTTP request and return a response
    fn handle(&self, request: &Request) -> Result<Response>;
}
