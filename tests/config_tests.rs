// Configuration tests - verify TOML parsing AND validator behaviour.
//
// These tests directly target the audit requirement:
//   "Verify Configuration Validation by ensuring the server correctly
//    identifies conflicting ports, invalid paths, and out-of-bounds body
//    size limits during startup."
//
// All tests use the real `Config` / `ServerConfig` / `RouteConfig` schema
// (HashMap-based routes/errors, struct-based ErrorPageConfig, etc.) so that
// the TOML strings here stay in lock-step with the production deserializer
// in `src/application/config/models.rs`.

use localhost::application::config::loader::ConfigLoader;
use std::fs;
use std::path::PathBuf;

/// Create (and return) a unique, existing temp directory that can be used as
/// a server `root`. The validator requires `root` to exist and be a directory.
fn make_temp_root(tag: &str) -> PathBuf {
    let path =
        std::env::temp_dir().join(format!("localhost_cfg_test_{}_{}", tag, std::process::id()));
    fs::create_dir_all(&path).unwrap();
    path
}

fn write_temp_toml(name: &str, contents: &str) -> PathBuf {
    let path = std::env::temp_dir().join(format!(
        "localhost_cfg_{}_{}.toml",
        name,
        std::process::id()
    ));
    fs::write(&path, contents).unwrap();
    path
}

// ---------------------------------------------------------------------------
// Happy paths
// ---------------------------------------------------------------------------

#[test]
fn test_valid_single_server_single_port() {
    let root = make_temp_root("single");
    let toml = format!(
        r#"
client_timeout_secs = 30
client_max_body_size = 1048576

[[servers]]
server_name = "test"
server_address = "127.0.0.1"
ports = [8080]
root = "{root}"

[servers.routes."/"]
methods = ["GET"]
directory = "."
"#,
        root = root.to_string_lossy()
    );

    let path = write_temp_toml("single_valid", &toml);
    let result = ConfigLoader::load(path.to_str().unwrap());
    assert!(result.is_ok(), "expected Ok, got {:?}", result.err());

    let cfg = result.unwrap();
    assert_eq!(cfg.servers.len(), 1);
    assert_eq!(cfg.servers[0].ports, vec![8080]);
    assert_eq!(cfg.servers[0].server_name, "test");
}

#[test]
fn test_valid_multiple_servers_different_ports() {
    let root_a = make_temp_root("multi_a");
    let root_b = make_temp_root("multi_b");

    let toml = format!(
        r#"
client_timeout_secs = 30
client_max_body_size = 1048576

[[servers]]
server_name = "server1"
server_address = "127.0.0.1"
ports = [8080, 8081]
root = "{a}"

[servers.routes."/"]
methods = ["GET"]
directory = "."

[[servers]]
server_name = "server2"
server_address = "127.0.0.1"
ports = [8082]
root = "{b}"

[servers.routes."/"]
methods = ["GET"]
directory = "."
"#,
        a = root_a.to_string_lossy(),
        b = root_b.to_string_lossy(),
    );

    let path = write_temp_toml("multi_ports", &toml);
    let cfg = ConfigLoader::load(path.to_str().unwrap()).expect("load");
    assert_eq!(cfg.servers.len(), 2);
    assert_eq!(cfg.servers[0].ports.len(), 2);
}

#[test]
fn test_valid_shared_port_with_different_server_names() {
    // Same port, different server_name → must be accepted (virtual hosting).
    let root = make_temp_root("shared_port");
    let toml = format!(
        r#"
client_timeout_secs = 30
client_max_body_size = 1048576

[[servers]]
server_name = "alpha"
server_address = "127.0.0.1"
ports = [8080]
root = "{r}"

[servers.routes."/"]
methods = ["GET"]
directory = "."

[[servers]]
server_name = "beta"
server_address = "127.0.0.1"
ports = [8080]
root = "{r}"

[servers.routes."/"]
methods = ["GET"]
directory = "."
"#,
        r = root.to_string_lossy()
    );

    let path = write_temp_toml("shared_port_ok", &toml);
    let result = ConfigLoader::load(path.to_str().unwrap());
    assert!(
        result.is_ok(),
        "shared port with distinct server_name must be accepted, got {:?}",
        result.err()
    );
}

#[test]
fn test_valid_config_with_cgi_handlers_and_error_pages() {
    let root = make_temp_root("cgi_errors");
    let toml = format!(
        r#"
client_timeout_secs = 30
client_max_body_size = 1048576

[[servers]]
server_name = "test"
server_address = "127.0.0.1"
ports = [8080]
root = "{r}"

[servers.routes."/"]
methods = ["GET"]
directory = "."

[servers.cgi_handlers]
".py" = "python3"
".sh" = "/bin/sh"

[servers.errors]
"404" = {{ filename = "errors/404.html" }}
"500" = {{ filename = "errors/500.html" }}
"#,
        r = root.to_string_lossy()
    );

    let path = write_temp_toml("cgi_errors", &toml);
    let cfg = ConfigLoader::load(path.to_str().unwrap()).expect("load");

    assert_eq!(
        cfg.servers[0].cgi_handlers.get(".py").map(String::as_str),
        Some("python3")
    );
    assert_eq!(
        cfg.servers[0].cgi_handlers.get(".sh").map(String::as_str),
        Some("/bin/sh")
    );
    assert!(cfg.servers[0].errors.contains_key("404"));
    assert!(cfg.servers[0].errors.contains_key("500"));
}

#[test]
fn test_default_global_values_are_applied() {
    let root = make_temp_root("defaults");
    let toml = format!(
        r#"
[[servers]]
server_name = "test"
server_address = "127.0.0.1"
ports = [8080]
root = "{r}"

[servers.routes."/"]
methods = ["GET"]
directory = "."
"#,
        r = root.to_string_lossy()
    );

    let path = write_temp_toml("defaults", &toml);
    let cfg = ConfigLoader::load(path.to_str().unwrap()).expect("load");
    assert!(cfg.client_timeout_secs > 0);
    assert!(cfg.client_max_body_size > 0);
}

// ---------------------------------------------------------------------------
// Audit-required failure paths
// ---------------------------------------------------------------------------

/// Same `(port, server_name)` pair twice → must be rejected.
/// Spec: "Configure the same port multiple times. The server should find the error."
#[test]
fn test_invalid_duplicate_port_with_same_server_name() {
    let root = make_temp_root("dup_port");
    let toml = format!(
        r#"
client_timeout_secs = 30
client_max_body_size = 1048576

[[servers]]
server_name = "duplicate"
server_address = "127.0.0.1"
ports = [8080]
root = "{r}"

[servers.routes."/"]
methods = ["GET"]
directory = "."

[[servers]]
server_name = "duplicate"
server_address = "127.0.0.1"
ports = [8080]
root = "{r}"

[servers.routes."/"]
methods = ["GET"]
directory = "."
"#,
        r = root.to_string_lossy()
    );

    let path = write_temp_toml("dup_port_same_name", &toml);
    let result = ConfigLoader::load(path.to_str().unwrap());
    assert!(
        result.is_err(),
        "duplicate (port, server_name) must be rejected, got Ok"
    );
}

/// Same port, different addresses → must be rejected.
/// Spec: "ensure that an error in one server's configuration is detected".
#[test]
fn test_invalid_same_port_different_address() {
    let root = make_temp_root("addr_conflict");
    let toml = format!(
        r#"
client_timeout_secs = 30
client_max_body_size = 1048576

[[servers]]
server_name = "a"
server_address = "127.0.0.1"
ports = [8080]
root = "{r}"

[servers.routes."/"]
methods = ["GET"]
directory = "."

[[servers]]
server_name = "b"
server_address = "127.0.0.2"
ports = [8080]
root = "{r}"

[servers.routes."/"]
methods = ["GET"]
directory = "."
"#,
        r = root.to_string_lossy()
    );

    let path = write_temp_toml("port_addr_conflict", &toml);
    let result = ConfigLoader::load(path.to_str().unwrap());
    assert!(
        result.is_err(),
        "same port on different addresses must be rejected"
    );
}

/// `root` pointing to non-existent path → must be rejected.
/// Spec: "Verify Configuration Validation … invalid paths …".
#[test]
fn test_invalid_nonexistent_root_path() {
    let toml = r#"
client_timeout_secs = 30
client_max_body_size = 1048576

[[servers]]
server_name = "test"
server_address = "127.0.0.1"
ports = [8080]
root = "/this/path/does/not/exist/anywhere"

[servers.routes."/"]
methods = ["GET"]
directory = "."
"#;

    let path = write_temp_toml("bad_root", toml);
    let result = ConfigLoader::load(path.to_str().unwrap());
    assert!(result.is_err(), "non-existent root must be rejected");
}

/// `client_max_body_size = 0` → must be rejected.
/// Spec: "Verify Configuration Validation … out-of-bounds body size limits".
#[test]
fn test_invalid_zero_body_size() {
    let root = make_temp_root("body_zero");
    let toml = format!(
        r#"
client_timeout_secs = 30
client_max_body_size = 0

[[servers]]
server_name = "test"
server_address = "127.0.0.1"
ports = [8080]
root = "{r}"

[servers.routes."/"]
methods = ["GET"]
directory = "."
"#,
        r = root.to_string_lossy()
    );

    let path = write_temp_toml("zero_body", &toml);
    let result = ConfigLoader::load(path.to_str().unwrap());
    assert!(result.is_err(), "client_max_body_size = 0 must be rejected");
}

#[test]
fn test_invalid_zero_timeout() {
    let root = make_temp_root("timeout_zero");
    let toml = format!(
        r#"
client_timeout_secs = 0
client_max_body_size = 1048576

[[servers]]
server_name = "test"
server_address = "127.0.0.1"
ports = [8080]
root = "{r}"

[servers.routes."/"]
methods = ["GET"]
directory = "."
"#,
        r = root.to_string_lossy()
    );

    let path = write_temp_toml("zero_timeout", &toml);
    let result = ConfigLoader::load(path.to_str().unwrap());
    assert!(result.is_err(), "client_timeout_secs = 0 must be rejected");
}

#[test]
fn test_invalid_port_zero() {
    let root = make_temp_root("port_zero");
    let toml = format!(
        r#"
client_timeout_secs = 30
client_max_body_size = 1048576

[[servers]]
server_name = "test"
server_address = "127.0.0.1"
ports = [0]
root = "{r}"

[servers.routes."/"]
methods = ["GET"]
directory = "."
"#,
        r = root.to_string_lossy()
    );

    let path = write_temp_toml("zero_port", &toml);
    let result = ConfigLoader::load(path.to_str().unwrap());
    assert!(result.is_err(), "port 0 must be rejected");
}

#[test]
fn test_invalid_empty_server_name() {
    let root = make_temp_root("empty_name");
    let toml = format!(
        r#"
client_timeout_secs = 30
client_max_body_size = 1048576

[[servers]]
server_name = ""
server_address = "127.0.0.1"
ports = [8080]
root = "{r}"

[servers.routes."/"]
methods = ["GET"]
directory = "."
"#,
        r = root.to_string_lossy()
    );

    let path = write_temp_toml("empty_name", &toml);
    let result = ConfigLoader::load(path.to_str().unwrap());
    assert!(result.is_err(), "empty server_name must be rejected");
}

#[test]
fn test_invalid_route_method() {
    let root = make_temp_root("bad_method");
    let toml = format!(
        r#"
client_timeout_secs = 30
client_max_body_size = 1048576

[[servers]]
server_name = "test"
server_address = "127.0.0.1"
ports = [8080]
root = "{r}"

[servers.routes."/"]
methods = ["FROBNICATE"]
directory = "."
"#,
        r = root.to_string_lossy()
    );

    let path = write_temp_toml("bad_method", &toml);
    let result = ConfigLoader::load(path.to_str().unwrap());
    assert!(result.is_err(), "unknown HTTP method must be rejected");
}

#[test]
fn test_invalid_error_code() {
    let root = make_temp_root("bad_err_code");
    let toml = format!(
        r#"
client_timeout_secs = 30
client_max_body_size = 1048576

[[servers]]
server_name = "test"
server_address = "127.0.0.1"
ports = [8080]
root = "{r}"

[servers.routes."/"]
methods = ["GET"]
directory = "."

[servers.errors]
"999" = {{ filename = "errors/999.html" }}
"#,
        r = root.to_string_lossy()
    );

    let path = write_temp_toml("bad_err_code", &toml);
    let result = ConfigLoader::load(path.to_str().unwrap());
    assert!(result.is_err(), "error code 999 must be rejected");
}

#[test]
fn test_invalid_route_multiple_targets() {
    // filename + directory + redirect on the same route → mutually exclusive.
    let root = make_temp_root("multi_target");
    let toml = format!(
        r#"
client_timeout_secs = 30
client_max_body_size = 1048576

[[servers]]
server_name = "test"
server_address = "127.0.0.1"
ports = [8080]
root = "{r}"

[servers.routes."/x"]
methods = ["GET"]
filename = "a.html"
directory = "."
"#,
        r = root.to_string_lossy()
    );

    let path = write_temp_toml("multi_target", &toml);
    let result = ConfigLoader::load(path.to_str().unwrap());
    assert!(
        result.is_err(),
        "route with both filename and directory must be rejected"
    );
}

#[test]
fn test_invalid_no_servers() {
    let toml = r#"
client_timeout_secs = 30
client_max_body_size = 1048576
"#;

    let path = write_temp_toml("no_servers", toml);
    let result = ConfigLoader::load(path.to_str().unwrap());
    assert!(
        result.is_err(),
        "config without any servers must be rejected"
    );
}
