// CGI tests - Testing chunked and unchunked data handling
// Tests both Transfer-Encoding: chunked and Content-Length scenarios

use localhost::application::cgi::CgiExecutor;
use localhost::http::method::Method;
use localhost::http::request::Request;
use localhost::http::version::Version;
use std::fs;
use std::io::Write;
use std::path::PathBuf;

/// Helper to create a temporary test CGI script
fn create_test_script(name: &str, content: &str) -> PathBuf {
    let script_path = PathBuf::from(format!("/tmp/{}", name));
    let mut file = fs::File::create(&script_path).unwrap();
    file.write_all(content.as_bytes()).unwrap();
    
    // Make script executable
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mut perms = fs::metadata(&script_path).unwrap().permissions();
        perms.set_mode(0o755);
        fs::set_permissions(&script_path, perms).unwrap();
    }
    
    script_path
}

/// Helper to clean up test script
fn cleanup_script(path: &PathBuf) {
    let _ = fs::remove_file(path);
}

#[test]
fn test_cgi_with_unchunked_data() {
    // Create a simple Python CGI script that echoes input and shows content length
    let script_content = r#"#!/usr/bin/env python3
import sys
import os

# Read all input from stdin
content_length = os.environ.get('CONTENT_LENGTH', '0')
if content_length != '0':
    data = sys.stdin.read(int(content_length))
else:
    data = ''

# Output CGI headers with proper CRLF line endings
sys.stdout.write('Content-Type: text/plain\r\n')
sys.stdout.write('Status: 200 OK\r\n')
sys.stdout.write('\r\n')  # Blank line separates headers from body

# Output response body
sys.stdout.write(f'Content-Length: {content_length}\n')
sys.stdout.write(f'Received data: {data}\n')
sys.stdout.write(f'Transfer-Encoding: none\n')
"#;

    let script_path = create_test_script("test_unchunked.py", script_content);
    
    // Create request with unchunked data (using Content-Length)
    let body = b"Hello from unchunked request";
    let mut request = Request::new(Method::POST, "/test".to_string(), Version::Http11);
    request.body = body.to_vec();
    request.headers.add("Content-Length".to_string(), body.len().to_string());
    request.headers.add("Content-Type".to_string(), "text/plain".to_string());
    
    // Execute CGI
    let executor = CgiExecutor::new(30);
    let response = executor.execute(
        script_path.clone(),
        Some("python3"),
        &request,
        "localhost",
        8080,
    );
    
    cleanup_script(&script_path);
    
    // Verify response
    assert!(response.is_ok(), "CGI execution should succeed: {:?}", response.err());
    let response = response.unwrap();
    assert_eq!(response.status.as_u16(), 200);
    
    let body_str = String::from_utf8_lossy(&response.body);
    assert!(body_str.contains("Content-Length: 28"), "Should show correct content length");
    assert!(body_str.contains("Hello from unchunked request"), "Should echo the data");
    assert!(body_str.contains("Transfer-Encoding: none"), "Should indicate unchunked");
}

#[test]
fn test_cgi_with_chunked_data() {
    // Create a Python CGI script that reads chunked data
    let script_content = r#"#!/usr/bin/env python3
import sys
import os

# Check if Transfer-Encoding header is present
transfer_encoding = os.environ.get('HTTP_TRANSFER_ENCODING', 'none')

# For chunked encoding, we read until EOF since the server should
# have already decoded the chunks and provided the full body
data = sys.stdin.read()

# Output CGI headers with proper CRLF line endings
sys.stdout.write('Content-Type: text/plain\r\n')
sys.stdout.write('Status: 200 OK\r\n')
sys.stdout.write('\r\n')  # Blank line separates headers from body

# Output response body
sys.stdout.write(f'Transfer-Encoding header: {transfer_encoding}\n')
sys.stdout.write(f'Data length: {len(data)}\n')
sys.stdout.write(f'Received data: {data}\n')
"#;

    let script_path = create_test_script("test_chunked.py", script_content);
    
    // Create request with chunked transfer encoding
    // Note: The body should already be decoded from chunks by the HTTP parser
    let body = b"This is chunked data that has been decoded";
    let mut request = Request::new(Method::POST, "/test".to_string(), Version::Http11);
    request.body = body.to_vec();
    request.headers.add("Transfer-Encoding".to_string(), "chunked".to_string());
    request.headers.add("Content-Type".to_string(), "text/plain".to_string());
    
    // Execute CGI
    let executor = CgiExecutor::new(30);
    let response = executor.execute(
        script_path.clone(),
        Some("python3"),
        &request,
        "localhost",
        8080,
    );
    
    cleanup_script(&script_path);
    
    // Verify response
    assert!(response.is_ok(), "CGI execution should succeed with chunked data: {:?}", response.err());
    let response = response.unwrap();
    assert_eq!(response.status.as_u16(), 200);
    
    let body_str = String::from_utf8_lossy(&response.body);
    assert!(body_str.contains("Transfer-Encoding header: chunked"), "Should detect chunked encoding");
    assert!(body_str.contains("This is chunked data"), "Should have received the decoded data");
    assert!(body_str.contains("Data length: 42"), "Should show correct data length");
}

#[test]
fn test_cgi_with_no_body() {
    // Test CGI with GET request (no body)
    let script_content = r#"#!/usr/bin/env python3
import sys
import os

request_method = os.environ.get('REQUEST_METHOD', 'UNKNOWN')
query_string = os.environ.get('QUERY_STRING', '')

# Output CGI headers with proper CRLF line endings
sys.stdout.write('Content-Type: text/plain\r\n')
sys.stdout.write('Status: 200 OK\r\n')
sys.stdout.write('\r\n')

# Output response body
sys.stdout.write(f'Method: {request_method}\n')
sys.stdout.write(f'Query: {query_string}\n')
sys.stdout.write('Body: (none)\n')
"#;

    let script_path = create_test_script("test_nobody.py", script_content);
    
    // Create GET request with no body
    let request = Request::new(Method::GET, "/test?param=value".to_string(), Version::Http11);
    
    // Execute CGI
    let executor = CgiExecutor::new(30);
    let response = executor.execute(
        script_path.clone(),
        Some("python3"),
        &request,
        "localhost",
        8080,
    );
    
    cleanup_script(&script_path);
    
    // Verify response
    assert!(response.is_ok(), "CGI execution should succeed with no body: {:?}", response.err());
    let response = response.unwrap();
    assert_eq!(response.status.as_u16(), 200);
    
    let body_str = String::from_utf8_lossy(&response.body);
    assert!(body_str.contains("Method: GET"), "Should show GET method");
    assert!(body_str.contains("Query: param=value"), "Should show query string");
    assert!(body_str.contains("Body: (none)"), "Should indicate no body");
}

#[test]
fn test_cgi_with_large_unchunked_data() {
    // Test CGI with large unchunked data
    let script_content = r#"#!/usr/bin/env python3
import sys
import os

content_length = int(os.environ.get('CONTENT_LENGTH', '0'))
if content_length > 0:
    data = sys.stdin.read(content_length)
else:
    data = ''

# Output CGI headers with proper CRLF line endings
sys.stdout.write('Content-Type: text/plain\r\n')
sys.stdout.write('Status: 200 OK\r\n')
sys.stdout.write('\r\n')

# Output response body
sys.stdout.write(f'Received bytes: {len(data)}\n')
sys.stdout.write(f'Content-Length header: {content_length}\n')
sys.stdout.write(f'Match: {len(data) == content_length}\n')
"#;

    let script_path = create_test_script("test_large_unchunked.py", script_content);
    
    // Create request with large body (10KB)
    let body = vec![b'X'; 10240];
    let mut request = Request::new(Method::POST, "/test".to_string(), Version::Http11);
    request.body = body.clone();
    request.headers.add("Content-Length".to_string(), body.len().to_string());
    request.headers.add("Content-Type".to_string(), "application/octet-stream".to_string());
    
    // Execute CGI
    let executor = CgiExecutor::new(30);
    let response = executor.execute(
        script_path.clone(),
        Some("python3"),
        &request,
        "localhost",
        8080,
    );
    
    cleanup_script(&script_path);
    
    // Verify response
    assert!(response.is_ok(), "CGI execution should succeed with large data: {:?}", response.err());
    let response = response.unwrap();
    assert_eq!(response.status.as_u16(), 200);
    
    let body_str = String::from_utf8_lossy(&response.body);
    assert!(body_str.contains("Received bytes: 10240"), "Should receive all bytes");
    assert!(body_str.contains("Content-Length header: 10240"), "Should have correct header");
    assert!(body_str.contains("Match: True"), "Should match exactly");
}

#[test]
fn test_cgi_with_multiple_chunks_simulation() {
    // Simulate multiple chunks by sending data that would typically come in chunks
    let script_content = r#"#!/usr/bin/env python3
import sys
import os

# Read data in chunks to simulate chunked reading
chunks = []
while True:
    chunk = sys.stdin.read(1024)
    if not chunk:
        break
    chunks.append(chunk)

data = ''.join(chunks)

# Output CGI headers with proper CRLF line endings
sys.stdout.write('Content-Type: text/plain\r\n')
sys.stdout.write('Status: 200 OK\r\n')
sys.stdout.write('\r\n')

# Output response body
sys.stdout.write(f'Number of chunks read: {len(chunks)}\n')
sys.stdout.write(f'Total data length: {len(data)}\n')
sys.stdout.write(f'First 50 chars: {data[:50]}\n')
sys.stdout.write(f'Last 50 chars: {data[-50:]}\n')
"#;

    let script_path = create_test_script("test_multi_chunk.py", script_content);
    
    // Create request with data that would typically be chunked (5KB)
    let body = vec![b'A'; 5120];
    let mut request = Request::new(Method::POST, "/test".to_string(), Version::Http11);
    request.body = body.clone();
    request.headers.add("Transfer-Encoding".to_string(), "chunked".to_string());
    request.headers.add("Content-Type".to_string(), "application/octet-stream".to_string());
    
    // Execute CGI
    let executor = CgiExecutor::new(30);
    let response = executor.execute(
        script_path.clone(),
        Some("python3"),
        &request,
        "localhost",
        8080,
    );
    
    cleanup_script(&script_path);
    
    // Verify response
    assert!(response.is_ok(), "CGI execution should succeed with multi-chunk data: {:?}", response.err());
    let response = response.unwrap();
    assert_eq!(response.status.as_u16(), 200);
    
    let body_str = String::from_utf8_lossy(&response.body);
    assert!(body_str.contains("Total data length: 5120"), "Should receive all data");
    assert!(body_str.contains("AAAAA"), "Should contain the expected data");
}

#[test]
fn test_cgi_response_parsing_with_chunked_output() {
    // Test that CGI can output chunked response
    let script_content = r#"#!/usr/bin/env python3
import sys

# Output CGI headers with chunked encoding
sys.stdout.write('Content-Type: text/plain\r\n')
sys.stdout.write('Transfer-Encoding: chunked\r\n')
sys.stdout.write('Status: 200 OK\r\n')
sys.stdout.write('\r\n')

# Output body (the CGI handler should handle chunking if needed)
sys.stdout.write('This is the first part of the response.\n')
sys.stdout.flush()
sys.stdout.write('This is the second part of the response.\n')
sys.stdout.flush()
sys.stdout.write('This is the final part.\n')
"#;

    let script_path = create_test_script("test_output_chunked.py", script_content);
    
    // Create simple GET request
    let request = Request::new(Method::GET, "/test".to_string(), Version::Http11);
    
    // Execute CGI
    let executor = CgiExecutor::new(30);
    let response = executor.execute(
        script_path.clone(),
        Some("python3"),
        &request,
        "localhost",
        8080,
    );
    
    cleanup_script(&script_path);
    
    // Verify response
    assert!(response.is_ok(), "CGI execution should succeed with chunked output: {:?}", response.err());
    let response = response.unwrap();
    assert_eq!(response.status.as_u16(), 200);
    
    // Check that output is received
    let body_str = String::from_utf8_lossy(&response.body);
    assert!(body_str.contains("first part"), "Should contain first part");
    assert!(body_str.contains("second part"), "Should contain second part");
    assert!(body_str.contains("final part"), "Should contain final part");
    
    // Check headers
    if let Some(transfer_encoding) = response.headers.get("Transfer-Encoding") {
        assert_eq!(transfer_encoding, "chunked", "Should have chunked transfer encoding");
    }
}

#[test]
fn test_cgi_environment_with_chunked_header() {
    // Verify that Transfer-Encoding header is properly passed to CGI
    let script_content = r#"#!/usr/bin/env python3
import sys
import os

# Get all HTTP headers
transfer_encoding = os.environ.get('HTTP_TRANSFER_ENCODING', 'NOT_SET')
content_length = os.environ.get('CONTENT_LENGTH', 'NOT_SET')
content_type = os.environ.get('CONTENT_TYPE', 'NOT_SET')

# Output CGI headers with proper CRLF line endings
sys.stdout.write('Content-Type: text/plain\r\n')
sys.stdout.write('Status: 200 OK\r\n')
sys.stdout.write('\r\n')

# Output environment info
sys.stdout.write(f'HTTP_TRANSFER_ENCODING: {transfer_encoding}\n')
sys.stdout.write(f'CONTENT_LENGTH: {content_length}\n')
sys.stdout.write(f'CONTENT_TYPE: {content_type}\n')
"#;

    let script_path = create_test_script("test_env_chunked.py", script_content);
    
    // Create request with chunked encoding
    let mut request = Request::new(Method::POST, "/test".to_string(), Version::Http11);
    request.body = b"test data".to_vec();
    request.headers.add("Transfer-Encoding".to_string(), "chunked".to_string());
    request.headers.add("Content-Type".to_string(), "application/x-www-form-urlencoded".to_string());
    
    // Execute CGI
    let executor = CgiExecutor::new(30);
    let response = executor.execute(
        script_path.clone(),
        Some("python3"),
        &request,
        "localhost",
        8080,
    );
    
    cleanup_script(&script_path);
    
    // Verify response
    assert!(response.is_ok(), "CGI execution should succeed: {:?}", response.err());
    let response = response.unwrap();
    assert_eq!(response.status.as_u16(), 200);
    
    let body_str = String::from_utf8_lossy(&response.body);
    assert!(body_str.contains("HTTP_TRANSFER_ENCODING: chunked"), "Should pass Transfer-Encoding header");
    assert!(body_str.contains("CONTENT_TYPE: application/x-www-form-urlencoded"), "Should pass Content-Type");
}

#[test]
fn test_cgi_comparison_chunked_vs_unchunked() {
    // Test that the same data produces the same output regardless of chunking
    let script_content = r#"#!/usr/bin/env python3
import sys
import os
import hashlib

# Read all data
data = sys.stdin.read()

# Calculate hash
data_hash = hashlib.md5(data.encode() if isinstance(data, str) else data).hexdigest()

transfer_encoding = os.environ.get('HTTP_TRANSFER_ENCODING', 'none')

# Output CGI headers with proper CRLF line endings
sys.stdout.write('Content-Type: text/plain\r\n')
sys.stdout.write('Status: 200 OK\r\n')
sys.stdout.write('\r\n')

# Output result
sys.stdout.write(f'Transfer-Encoding: {transfer_encoding}\n')
sys.stdout.write(f'Data length: {len(data)}\n')
sys.stdout.write(f'MD5 hash: {data_hash}\n')
"#;

    let script_path = create_test_script("test_comparison.py", script_content);
    
    let test_data = b"The quick brown fox jumps over the lazy dog";
    
    // Test 1: Unchunked
    let mut request1 = Request::new(Method::POST, "/test".to_string(), Version::Http11);
    request1.body = test_data.to_vec();
    request1.headers.add("Content-Length".to_string(), test_data.len().to_string());
    request1.headers.add("Content-Type".to_string(), "text/plain".to_string());
    
    let executor = CgiExecutor::new(30);
    let result1 = executor.execute(
        script_path.clone(),
        Some("python3"),
        &request1,
        "localhost",
        8080,
    );
    assert!(result1.is_ok(), "Unchunked request should succeed: {:?}", result1.err());
    let response1 = result1.unwrap();
    
    // Test 2: Chunked
    let mut request2 = Request::new(Method::POST, "/test".to_string(), Version::Http11);
    request2.body = test_data.to_vec();
    request2.headers.add("Transfer-Encoding".to_string(), "chunked".to_string());
    request2.headers.add("Content-Type".to_string(), "text/plain".to_string());
    
    let result2 = executor.execute(
        script_path.clone(),
        Some("python3"),
        &request2,
        "localhost",
        8080,
    );
    assert!(result2.is_ok(), "Chunked request should succeed: {:?}", result2.err());
    let response2 = result2.unwrap();
    
    cleanup_script(&script_path);
    
    // Both should succeed
    assert_eq!(response1.status.as_u16(), 200);
    assert_eq!(response2.status.as_u16(), 200);
    
    let body1 = String::from_utf8_lossy(&response1.body);
    let body2 = String::from_utf8_lossy(&response2.body);
    
    // Extract MD5 hashes
    let hash1 = body1.lines()
        .find(|l| l.starts_with("MD5 hash:"))
        .unwrap()
        .split(": ")
        .nth(1)
        .unwrap();
    
    let hash2 = body2.lines()
        .find(|l| l.starts_with("MD5 hash:"))
        .unwrap()
        .split(": ")
        .nth(1)
        .unwrap();
    
    // Hashes should match - data is identical regardless of chunking
    assert_eq!(hash1, hash2, "Data should be identical for chunked and unchunked");
    
    // Both should report same data length (43 bytes)
    assert!(body1.contains("Data length: 43"));
    assert!(body2.contains("Data length: 43"));
}
