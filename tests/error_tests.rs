// Error handling tests - verify server handles errors gracefully

use std::thread;
use std::time::Duration;

mod common;
use common::{create_test_config, send_request, start_test_server};

#[test]
#[ignore]
fn test_malformed_request_line() {
    let port = 9000;
    let _server_thread = start_test_server(port, 1024);
    thread::sleep(Duration::from_millis(500));
    
    // Invalid request line
    let request = "INVALID REQUEST LINE\r\n\r\n";
    let response = send_request(port, request);
    
    // Server should handle gracefully (return 400 or close connection)
    assert!(!response.is_empty());
}

#[test]
#[ignore]
fn test_invalid_http_method() {
    let port = 9001;
    let _server_thread = start_test_server(port, 1024);
    thread::sleep(Duration::from_millis(500));
    
    // Invalid method
    let request = "INVALIDMETHOD / HTTP/1.1\r\nHost: localhost\r\n\r\n";
    let response = send_request(port, request);
    
    // Should return 400 Bad Request or 405 Method Not Allowed
    assert!(response.contains("400") || response.contains("405"));
}

#[test]
#[ignore]
fn test_missing_host_header() {
    let port = 9002;
    let _server_thread = start_test_server(port, 1024);
    thread::sleep(Duration::from_millis(500));
    
    // Request without Host header
    let request = "GET / HTTP/1.1\r\n\r\n";
    let response = send_request(port, request);
    
    // Server should handle (may use default server)
    assert!(!response.is_empty());
}

#[test]
#[ignore]
fn test_oversized_headers() {
    let port = 9003;
    let _server_thread = start_test_server(port, 1024);
    thread::sleep(Duration::from_millis(500));
    
    // Request with very large header
    let large_header = "X-Large: ".to_string() + &"x".repeat(10000);
    let request = format!("GET / HTTP/1.1\r\nHost: localhost\r\n{}\r\n\r\n", large_header);
    let response = send_request(port, &request);
    
    // Server should handle gracefully (may return 400 or 413)
    assert!(!response.is_empty());
}

#[test]
#[ignore]
fn test_invalid_content_length() {
    let port = 9004;
    let _server_thread = start_test_server(port, 1024);
    thread::sleep(Duration::from_millis(500));
    
    // Request with Content-Length but no body
    let request = "POST / HTTP/1.1\r\nHost: localhost\r\nContent-Length: 100\r\n\r\n";
    let response = send_request(port, request);
    
    // Server should handle (may wait for body or return error)
    assert!(!response.is_empty());
}

#[test]
#[ignore]
fn test_chunked_encoding_error() {
    let port = 9005;
    let _server_thread = start_test_server(port, 1024);
    thread::sleep(Duration::from_millis(500));
    
    // Invalid chunked encoding
    let request = "POST / HTTP/1.1\r\nHost: localhost\r\nTransfer-Encoding: chunked\r\n\r\ninvalid chunk\r\n";
    let response = send_request(port, request);
    
    // Server should handle gracefully
    assert!(!response.is_empty());
}

#[test]
#[ignore]
fn test_server_continues_after_error() {
    let port = 9006;
    let _server_thread = start_test_server(port, 1024);
    thread::sleep(Duration::from_millis(500));
    
    // Send malformed request
    let bad_request = "INVALID REQUEST\r\n\r\n";
    let _response1 = send_request(port, bad_request);
    
    // Server should still respond to valid requests
    thread::sleep(Duration::from_millis(100));
    
    let good_request = "GET / HTTP/1.1\r\nHost: localhost\r\n\r\n";
    let response2 = send_request(port, good_request);
    
    // Should get valid response
    assert!(response2.contains("HTTP/1.1"));
}

#[test]
#[ignore]
fn test_body_size_exceeded() {
    let port = 9007;
    let config = create_test_config(port, 100); // Small limit
    
    let test_root = PathBuf::from(&config.servers[0].root);
    let upload_dir = test_root.join("uploads");
    fs::create_dir_all(&upload_dir).unwrap();
    
    let _server_thread = start_test_server(port, 1024);
    thread::sleep(Duration::from_millis(500));
    
    // Send body larger than limit
    let large_body = "x".repeat(200);
    let request = format!(
        "POST /upload HTTP/1.1\r\nHost: localhost\r\nContent-Length: {}\r\n\r\n{}",
        large_body.len(),
        large_body
    );
    
    let response = send_request(port, &request);
    
    // Should return 413 Payload Too Large
    assert!(response.contains("413"));
}

#[test]
#[ignore]
fn test_invalid_path() {
    let port = 9008;
    let _server_thread = start_test_server(port, 1024);
    thread::sleep(Duration::from_millis(500));
    
    // Path with invalid characters or path traversal
    let request = "GET /../../../etc/passwd HTTP/1.1\r\nHost: localhost\r\n\r\n";
    let response = send_request(port, request);
    
    // Should return 404 or 403, not crash
    assert!(response.contains("404") || response.contains("403"));
}



