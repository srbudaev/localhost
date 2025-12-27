// Stress test helpers and utilities
// Note: Actual stress testing should be done with siege tool

use std::fs;
use std::io::{Read, Write};
use std::net::TcpStream;
use std::path::PathBuf;
use std::thread;
use std::time::{Duration, Instant};

use localhost::application::config::models::{Config, ServerConfig, RouteConfig};
use localhost::application::server::server_manager::ServerManager;

fn create_test_config(port: u16) -> Config {
    let test_root = std::env::temp_dir().join("localhost_stress_test");
    fs::create_dir_all(&test_root).unwrap();
    
    // Create a simple test file
    let test_file = test_root.join("test.html");
    fs::write(&test_file, "<html><body>Test</body></html>").unwrap();
    
    Config {
        client_timeout_secs: 30,
        client_max_body_size: 1024 * 1024,
        servers: vec![ServerConfig {
            server_name: "localhost".to_string(),
            server_address: std::net::IpAddr::V4(std::net::Ipv4Addr::new(127, 0, 0, 1)),
            ports: vec![port],
            root: test_root.to_string_lossy().to_string(),
            routes: vec![RouteConfig {
                path: "/".to_string(),
                methods: vec![],
                redirect: None,
                root: None,
                default_file: Some("test.html".to_string()),
                cgi_extension: None,
                directory_listing: false,
                upload_dir: None,
            }],
            error_pages: std::collections::HashMap::new(),
            cgi_handlers: std::collections::HashMap::new(),
            admin_access: false,
        }],
        admin: None,
    }
}

fn send_request(port: u16, request: &str) -> Result<String, std::io::Error> {
    let mut stream = TcpStream::connect(format!("127.0.0.1:{}", port))?;
    stream.set_read_timeout(Some(Duration::from_secs(5)))?;
    stream.set_write_timeout(Some(Duration::from_secs(5)))?;
    
    stream.write_all(request.as_bytes())?;
    stream.flush()?;
    
    let mut response = String::new();
    stream.read_to_string(&mut response)?;
    Ok(response)
}

fn start_test_server(port: u16) -> thread::JoinHandle<()> {
    let config = create_test_config(port);
    let mut server_manager = ServerManager::new(config).unwrap();
    
    thread::spawn(move || {
        let _ = server_manager.run();
    })
}

/// Helper function to send multiple concurrent requests
pub fn send_concurrent_requests(port: u16, num_requests: usize) -> Vec<Result<String, std::io::Error>> {
    let request = "GET / HTTP/1.1\r\nHost: localhost\r\n\r\n";
    let mut handles = Vec::new();
    
    for _ in 0..num_requests {
        let req = request.to_string();
        let handle = thread::spawn(move || {
            send_request(port, &req)
        });
        handles.push(handle);
    }
    
    handles.into_iter().map(|h| h.join().unwrap()).collect()
}

#[test]
#[ignore] // Manual stress test
fn test_concurrent_requests() {
    let port = 9100;
    let _server_thread = start_test_server(port);
    thread::sleep(Duration::from_millis(500));
    
    let num_requests = 100;
    let start = Instant::now();
    
    let results = send_concurrent_requests(port, num_requests);
    
    let duration = start.elapsed();
    let successful = results.iter().filter(|r| r.is_ok()).count();
    
    println!("Sent {} requests in {:?}", num_requests, duration);
    println!("Successful: {}/{}", successful, num_requests);
    println!("Success rate: {:.2}%", (successful as f64 / num_requests as f64) * 100.0);
    
    // At least 95% should succeed
    assert!(successful >= num_requests * 95 / 100);
}

#[test]
#[ignore] // Manual stress test
fn test_rapid_requests() {
    let port = 9101;
    let _server_thread = start_test_server(port);
    thread::sleep(Duration::from_millis(500));
    
    let num_requests = 1000;
    let request = "GET / HTTP/1.1\r\nHost: localhost\r\n\r\n";
    
    let start = Instant::now();
    let mut successful = 0;
    
    for _ in 0..num_requests {
        if send_request(port, request).is_ok() {
            successful += 1;
        }
    }
    
    let duration = start.elapsed();
    println!("Sent {} requests sequentially in {:?}", num_requests, duration);
    println!("Successful: {}/{}", successful, num_requests);
    println!("Requests per second: {:.2}", num_requests as f64 / duration.as_secs_f64());
    
    // At least 99% should succeed
    assert!(successful >= num_requests * 99 / 100);
}

#[test]
#[ignore] // Manual stress test
fn test_connection_cleanup() {
    let port = 9102;
    let _server_thread = start_test_server(port);
    thread::sleep(Duration::from_millis(500));
    
    // Open many connections and close them
    let num_connections = 100;
    let mut connections = Vec::new();
    
    for _ in 0..num_connections {
        if let Ok(mut stream) = TcpStream::connect(format!("127.0.0.1:{}", port)) {
            let request = "GET / HTTP/1.1\r\nHost: localhost\r\nConnection: close\r\n\r\n";
            let _ = stream.write_all(request.as_bytes());
            connections.push(stream);
        }
    }
    
    // Close all connections
    drop(connections);
    
    thread::sleep(Duration::from_millis(1000));
    
    // Server should still accept new connections
    let request = "GET / HTTP/1.1\r\nHost: localhost\r\n\r\n";
    let result = send_request(port, request);
    
    assert!(result.is_ok());
}

/// Helper to measure server response time
pub fn measure_response_time(port: u16, num_samples: usize) -> Duration {
    let request = "GET / HTTP/1.1\r\nHost: localhost\r\n\r\n";
    let mut total_time = Duration::new(0, 0);
    
    for _ in 0..num_samples {
        let start = Instant::now();
        let _ = send_request(port, request);
        total_time += start.elapsed();
    }
    
    total_time / num_samples as u32
}

#[test]
#[ignore] // Manual performance test
fn test_response_time() {
    let port = 9103;
    let _server_thread = start_test_server(port);
    thread::sleep(Duration::from_millis(500));
    
    let avg_time = measure_response_time(port, 100);
    println!("Average response time: {:?}", avg_time);
    
    // Response should be reasonably fast (< 100ms for simple request)
    assert!(avg_time < Duration::from_millis(100));
}


