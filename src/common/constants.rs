pub const DEFAULT_REQUEST_TIMEOUT_SECS: u64 = 30;
pub const DEFAULT_MAX_BODY_SIZE: usize = 10 * 1024 * 1024; // 10MB
pub const DEFAULT_BUFFER_SIZE: usize = 8192; // 8KB
pub const DEFAULT_MAX_HEADER_SIZE: usize = 8192; // 8KB
pub const DEFAULT_KEEP_ALIVE_TIMEOUT_SECS: u64 = 5;

pub const HTTP_VERSION_1_1: &str = "HTTP/1.1";
pub const CRLF: &str = "\r\n";
pub const CRLF_BYTES: &[u8] = b"\r\n";

pub const DEFAULT_ERROR_PAGES: &[u16] = &[400, 403, 404, 405, 413, 500];

pub const DEFAULT_SESSION_TIMEOUT_SECS: u64 = 3600; // 1 hour

