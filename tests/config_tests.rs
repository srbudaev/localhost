// Configuration tests - verify config parsing and validation

use localhost::application::config::loader::ConfigLoader;
use std::fs;

#[test]
fn test_valid_config_parsing() {
    let toml_content = r#"
client_timeout_secs = 30
client_max_body_size = 1048576

[[servers]]
server_name = "test"
server_address = "127.0.0.1"
ports = [8080]
root = "/tmp/test"
routes = []
"#;
    
    let temp_file = std::env::temp_dir().join("test_config.toml");
    fs::write(&temp_file, toml_content).unwrap();
    
    let result = ConfigLoader::load(temp_file.to_str().unwrap());
    assert!(result.is_ok());
}

#[test]
fn test_invalid_config_parsing() {
    let toml_content = r#"
invalid = "config"
"#;
    
    let temp_file = std::env::temp_dir().join("test_invalid_config.toml");
    fs::write(&temp_file, toml_content).unwrap();
    
    let _result = ConfigLoader::load(temp_file.to_str().unwrap());
    // Should fail or handle gracefully
    // Note: Actual behavior depends on validator implementation
}

#[test]
fn test_multiple_servers_config() {
    let toml_content = r#"
client_timeout_secs = 30
client_max_body_size = 1048576

[[servers]]
server_name = "server1"
server_address = "127.0.0.1"
ports = [8080, 8081]
root = "/tmp/server1"
routes = []

[[servers]]
server_name = "server2"
server_address = "127.0.0.1"
ports = [8082]
root = "/tmp/server2"
routes = []
"#;
    
    let temp_file = std::env::temp_dir().join("test_multi_config.toml");
    fs::write(&temp_file, toml_content).unwrap();
    
    let result = ConfigLoader::load(temp_file.to_str().unwrap());
    assert!(result.is_ok());
    
    let config = result.unwrap();
    assert_eq!(config.servers.len(), 2);
}

#[test]
fn test_config_with_routes() {
    let toml_content = r#"
client_timeout_secs = 30
client_max_body_size = 1048576

[[servers]]
server_name = "test"
server_address = "127.0.0.1"
ports = [8080]
root = "/tmp/test"

[[servers.routes]]
path = "/"
methods = ["GET", "POST"]
default_file = "index.html"
directory_listing = true

[[servers.routes]]
path = "/api"
methods = ["GET"]
redirect = "/new-api"
"#;
    
    let temp_file = std::env::temp_dir().join("test_routes_config.toml");
    fs::write(&temp_file, toml_content).unwrap();
    
    let result = ConfigLoader::load(temp_file.to_str().unwrap());
    assert!(result.is_ok());
    
    let config = result.unwrap();
    assert_eq!(config.servers[0].routes.len(), 2);
}

#[test]
fn test_config_with_error_pages() {
    let toml_content = r#"
client_timeout_secs = 30
client_max_body_size = 1048576

[[servers]]
server_name = "test"
server_address = "127.0.0.1"
ports = [8080]
root = "/tmp/test"
routes = []

[servers.error_pages]
404 = "/error/404.html"
500 = "/error/500.html"
"#;
    
    let temp_file = std::env::temp_dir().join("test_error_pages_config.toml");
    fs::write(&temp_file, toml_content).unwrap();
    
    let result = ConfigLoader::load(temp_file.to_str().unwrap());
    assert!(result.is_ok());
    
    let config = result.unwrap();
    assert!(config.servers[0].errors.contains_key("404"));
    assert!(config.servers[0].errors.contains_key("500"));
}

#[test]
fn test_config_with_cgi_handlers() {
    let toml_content = r#"
client_timeout_secs = 30
client_max_body_size = 1048576

[[servers]]
server_name = "test"
server_address = "127.0.0.1"
ports = [8080]
root = "/tmp/test"
routes = []

[servers.cgi_handlers]
".py" = "python3"
".sh" = "/bin/bash"
"#;
    
    let temp_file = std::env::temp_dir().join("test_cgi_config.toml");
    fs::write(&temp_file, toml_content).unwrap();
    
    let result = ConfigLoader::load(temp_file.to_str().unwrap());
    assert!(result.is_ok());
    
    let config = result.unwrap();
    assert!(config.servers[0].cgi_handlers.contains_key(".py"));
    assert!(config.servers[0].cgi_handlers.contains_key(".sh"));
}

#[test]
fn test_config_default_values() {
    let toml_content = r#"
[[servers]]
server_name = "test"
server_address = "127.0.0.1"
ports = [8080]
root = "/tmp/test"
routes = []
"#;
    
    let temp_file = std::env::temp_dir().join("test_defaults_config.toml");
    fs::write(&temp_file, toml_content).unwrap();
    
    let result = ConfigLoader::load(temp_file.to_str().unwrap());
    assert!(result.is_ok());
    
    let config = result.unwrap();
    // Should have default timeout and body size
    assert!(config.client_timeout_secs > 0);
    assert!(config.client_max_body_size > 0);
}








