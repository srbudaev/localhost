// Common test utilities to reduce code duplication

use std::fs;
use std::io::{Read, Write};
use std::net::TcpStream;
use std::thread;

use localhost::application::config::models::{Config, ServerConfig, RouteConfig};
use localhost::application::server::server_manager::ServerManager;

/// Create a test configuration with specified port and optional body size limit
pub fn create_test_config(port: u16, body_size: usize) -> Config {
    let test_root = std::env::temp_dir().join(format!("localhost_test_{}", port));
    fs::create_dir_all(&test_root).unwrap();
    
    let mut routes = std::collections::HashMap::new();
    routes.insert("/".to_string(), RouteConfig {
        methods: vec![],
        filename: None,
        directory: None,
        redirect: None,
        redirect_type: None,
        default_file: Some("index.html".to_string()),
        cgi_extension: None,
        directory_listing: true,
        upload_dir: None,
    });
    
    Config {
        client_timeout_secs: 30,
        client_max_body_size: body_size,
        servers: vec![ServerConfig {
            server_name: "localhost".to_string(),
            server_address: std::net::IpAddr::V4(std::net::Ipv4Addr::new(127, 0, 0, 1)),
            ports: vec![port],
            root: test_root.to_string_lossy().to_string(),
            routes,
            errors: std::collections::HashMap::new(),
            cgi_handlers: std::collections::HashMap::new(),
            admin_access: false,
        }],
        admin: None,
    }
}

/// Send HTTP request and get response.
///
/// IMPORTANT: This uses `read_to_string`, which waits until the server closes the
/// connection. If the caller-supplied request does NOT include a
/// `Connection: close` header, this function will inject one (placing it right
/// before the headers terminator `\r\n\r\n`) so that the server closes the
/// socket after responding and the read does not hang on keep-alive.
#[allow(dead_code)] // Used in integration_tests.rs and error_tests.rs
pub fn send_request(port: u16, request: &str) -> String {
    let request_with_close = ensure_connection_close(request);

    let mut stream = TcpStream::connect(format!("127.0.0.1:{}", port))
        .expect("Failed to connect to server");

    stream
        .set_read_timeout(Some(std::time::Duration::from_secs(10)))
        .ok();

    stream.write_all(request_with_close.as_bytes()).unwrap();
    stream.flush().unwrap();

    let mut response = String::new();
    let _ = stream.read_to_string(&mut response);
    response
}

#[allow(dead_code)]
fn ensure_connection_close(request: &str) -> String {
    if request.to_ascii_lowercase().contains("connection:") {
        return request.to_string();
    }
    if let Some(idx) = request.find("\r\n\r\n") {
        let (head, tail) = request.split_at(idx);
        format!("{}\r\nConnection: close{}", head, tail)
    } else {
        request.to_string()
    }
}

/// Start test server in background thread
/// Note: ServerManager is created inside the thread to avoid Send requirement
/// since it contains Rc<Poller> which is not Send.
#[allow(dead_code)] // Used in integration_tests.rs and error_tests.rs
pub fn start_test_server(port: u16, body_size: usize) -> thread::JoinHandle<()> {
    let config = create_test_config(port, body_size);
    
    thread::spawn(move || {
        let mut server_manager = ServerManager::new(config).unwrap();
        let _ = server_manager.run();
    })
}



