// Integration tests for localhost HTTP server
// These tests verify end-to-end functionality

use std::fs;
use std::path::PathBuf;
use std::time::Duration;

use localhost::application::config::models::RouteConfig;

mod common;
use common::{create_test_config, send_request, start_test_server};

#[test]
#[ignore] // Ignore by default - requires server to be running
fn test_single_server_single_port() {
    let port = 8080;
    let config = create_test_config(port, 1024 * 1024);
    
    // Create test file
    let test_root = PathBuf::from(&config.servers[0].root);
    let test_file = test_root.join("index.html");
    fs::write(&test_file, "<html><body>Test</body></html>").unwrap();
    
    // Start server
    let _server_thread = start_test_server(port, 1024 * 1024);
    thread::sleep(Duration::from_millis(500)); // Wait for server to start
    
    // Send GET request
    let request = "GET / HTTP/1.1\r\nHost: localhost\r\n\r\n";
    let response = send_request(port, request);
    
    assert!(response.contains("HTTP/1.1"));
    assert!(response.contains("200") || response.contains("OK"));
    assert!(response.contains("Test"));
}

#[test]
#[ignore]
fn test_get_request() {
    let port = 8081;
    let config = create_test_config(port, 1024 * 1024);
    
    let test_root = PathBuf::from(&config.servers[0].root);
    let test_file = test_root.join("test.txt");
    fs::write(&test_file, "Hello, World!").unwrap();
    
    let _server_thread = start_test_server(port, 1024 * 1024);
    thread::sleep(Duration::from_millis(500));
    
    let request = "GET /test.txt HTTP/1.1\r\nHost: localhost\r\n\r\n";
    let response = send_request(port, request);
    
    assert!(response.contains("200"));
    assert!(response.contains("Hello, World!"));
}

#[test]
#[ignore]
fn test_post_request() {
    let port = 8082;
    let mut config = create_test_config(port, 1024 * 1024);
    
    // Add upload route
    config.servers[0].routes[0].upload_dir = Some("uploads".to_string());
    
    let test_root = PathBuf::from(&config.servers[0].root);
    let upload_dir = test_root.join("uploads");
    fs::create_dir_all(&upload_dir).unwrap();
    
    let _server_thread = start_test_server(port, 1024 * 1024);
    thread::sleep(Duration::from_millis(500));
    
    let body = "test data";
    let request = format!(
        "POST /upload.txt HTTP/1.1\r\nHost: localhost\r\nContent-Length: {}\r\n\r\n{}",
        body.len(),
        body
    );
    
    let response = send_request(port, &request);
    
    assert!(response.contains("200") || response.contains("201"));
}

#[test]
#[ignore]
fn test_delete_request() {
    let port = 8083;
    let config = create_test_config(port, 1024 * 1024);
    
    let test_root = PathBuf::from(&config.servers[0].root);
    let test_file = test_root.join("delete_me.txt");
    fs::write(&test_file, "delete me").unwrap();
    
    let _server_thread = start_test_server(port, 1024 * 1024);
    thread::sleep(Duration::from_millis(500));
    
    // DELETE request
    let request = "DELETE /delete_me.txt HTTP/1.1\r\nHost: localhost\r\n\r\n";
    let response = send_request(port, request);
    
    assert!(response.contains("200") || response.contains("204"));
    
    // Verify file is deleted
    assert!(!test_file.exists());
}

#[test]
#[ignore]
fn test_not_found() {
    let port = 8084;
    let config = create_test_config(port, 1024 * 1024);
    
    let _server_thread = start_test_server(port, 1024 * 1024);
    thread::sleep(Duration::from_millis(500));
    
    let request = "GET /nonexistent.txt HTTP/1.1\r\nHost: localhost\r\n\r\n";
    let response = send_request(port, request);
    
    assert!(response.contains("404"));
}

#[test]
#[ignore]
fn test_body_size_limit() {
    let port = 8085;
    let config = create_test_config(port, 100); // Small limit
    
    let _server_thread = start_test_server(port, 100);
    thread::sleep(Duration::from_millis(500));
    
    // Send request with body larger than limit
    let large_body = "x".repeat(200);
    let request = format!(
        "POST /test HTTP/1.1\r\nHost: localhost\r\nContent-Length: {}\r\n\r\n{}",
        large_body.len(),
        large_body
    );
    
    let response = send_request(port, &request);
    
    assert!(response.contains("413"));
}

#[test]
#[ignore]
fn test_method_not_allowed() {
    let port = 8086;
    let mut config = create_test_config(port, 1024 * 1024);
    // Restrict route to GET only
    config.servers[0].routes[0].methods = vec!["GET".to_string()];
    
    let _server_thread = start_test_server(port, 1024 * 1024);
    thread::sleep(Duration::from_millis(500));
    
    // Try POST on GET-only route
    let request = "POST / HTTP/1.1\r\nHost: localhost\r\n\r\n";
    let response = send_request(port, request);
    
    assert!(response.contains("405"));
}

#[test]
#[ignore]
fn test_directory_listing() {
    let port = 8087;
    let mut config = create_test_config(port, 1024 * 1024);
    config.servers[0].routes[0].directory_listing = true;
    
    let test_root = PathBuf::from(&config.servers[0].root);
    let subdir = test_root.join("subdir");
    fs::create_dir_all(&subdir).unwrap();
    fs::write(subdir.join("file.txt"), "content").unwrap();
    
    let _server_thread = start_test_server(port, 1024 * 1024);
    thread::sleep(Duration::from_millis(500));
    
    let request = "GET /subdir/ HTTP/1.1\r\nHost: localhost\r\n\r\n";
    let response = send_request(port, request);
    
    assert!(response.contains("200"));
    assert!(response.contains("file.txt"));
}

#[test]
#[ignore]
fn test_default_file() {
    let port = 8088;
    let mut config = create_test_config(port, 1024 * 1024);
    config.servers[0].routes[0].default_file = Some("index.html".to_string());
    
    let test_root = PathBuf::from(&config.servers[0].root);
    let index_file = test_root.join("index.html");
    fs::write(&index_file, "<html>Index</html>").unwrap();
    
    let _server_thread = start_test_server(port, 1024 * 1024);
    thread::sleep(Duration::from_millis(500));
    
    // Request directory, should serve index.html
    let request = "GET / HTTP/1.1\r\nHost: localhost\r\n\r\n";
    let response = send_request(port, request);
    
    assert!(response.contains("200"));
    assert!(response.contains("Index"));
}

#[test]
#[ignore]
fn test_redirect() {
    let port = 8089;
    let mut config = create_test_config(port, 1024 * 1024);
    config.servers[0].routes.push(RouteConfig {
        path: "/old".to_string(),
        methods: vec![],
        redirect: Some("/new".to_string()),
        root: None,
        default_file: None,
        cgi_extension: None,
        directory_listing: false,
        upload_dir: None,
    });
    
    let _server_thread = start_test_server(port, 1024 * 1024);
    thread::sleep(Duration::from_millis(500));
    
    let request = "GET /old HTTP/1.1\r\nHost: localhost\r\n\r\n";
    let response = send_request(port, request);
    
    assert!(response.contains("302") || response.contains("301"));
    assert!(response.contains("/new"));
}


