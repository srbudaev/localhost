use crate::http::request::Request;
use std::collections::HashMap;
use std::path::PathBuf;

/// Build CGI environment variables from HTTP request
pub struct CgiEnvironment;

impl CgiEnvironment {
    /// Build environment variables for CGI script execution
    pub fn build(request: &Request, script_path: &PathBuf, server_name: &str, server_port: u16) -> HashMap<String, String> {
        let mut env_vars = HashMap::new();

        // Request method
        env_vars.insert("REQUEST_METHOD".to_string(), request.method.to_string());

        // Request URI
        env_vars.insert("REQUEST_URI".to_string(), request.target.clone());
        
        // Script name (path portion of URI)
        if let Some(query_pos) = request.target.find('?') {
            env_vars.insert("SCRIPT_NAME".to_string(), request.target[..query_pos].to_string());
        } else {
            env_vars.insert("SCRIPT_NAME".to_string(), request.target.clone());
        }

        // Query string
        if let Some(query) = request.query_string() {
            env_vars.insert("QUERY_STRING".to_string(), query.to_string());
        } else {
            env_vars.insert("QUERY_STRING".to_string(), String::new());
        }

        // Path info (additional path after script name)
        // For now, empty - can be enhanced later
        env_vars.insert("PATH_INFO".to_string(), String::new());
        env_vars.insert("PATH_TRANSLATED".to_string(), String::new());

        // Server information
        env_vars.insert("SERVER_NAME".to_string(), server_name.to_string());
        env_vars.insert("SERVER_PORT".to_string(), server_port.to_string());
        env_vars.insert("SERVER_PROTOCOL".to_string(), format!("{}", request.version));
        env_vars.insert("SERVER_SOFTWARE".to_string(), "localhost/0.1.0".to_string());

        // Content information
        if let Some(content_type) = request.content_type() {
            env_vars.insert("CONTENT_TYPE".to_string(), content_type.clone());
        }

        if let Some(content_length) = request.content_length() {
            env_vars.insert("CONTENT_LENGTH".to_string(), content_length.to_string());
        } else {
            env_vars.insert("CONTENT_LENGTH".to_string(), "0".to_string());
        }

        // HTTP headers as environment variables
        // Format: HTTP_<HEADER_NAME> (uppercase, dashes replaced with underscores)
        for (name, values) in request.headers.iter() {
            // Use first value if multiple values exist
            if let Some(value) = values.first() {
                let env_name = format!("HTTP_{}", name.to_uppercase().replace('-', "_"));
                env_vars.insert(env_name, value.clone());
            }
        }

        // Standard CGI variables from headers
        if let Some(host) = request.host() {
            env_vars.insert("HTTP_HOST".to_string(), host.clone());
        }

        if let Some(user_agent) = request.headers.get("User-Agent") {
            env_vars.insert("HTTP_USER_AGENT".to_string(), user_agent.clone());
        }

        if let Some(accept) = request.headers.get("Accept") {
            env_vars.insert("HTTP_ACCEPT".to_string(), accept.clone());
        }

        if let Some(accept_language) = request.headers.get("Accept-Language") {
            env_vars.insert("HTTP_ACCEPT_LANGUAGE".to_string(), accept_language.clone());
        }

        if let Some(accept_encoding) = request.headers.get("Accept-Encoding") {
            env_vars.insert("HTTP_ACCEPT_ENCODING".to_string(), accept_encoding.clone());
        }

        // Remote address (if available)
        // Note: This would need to be passed from connection
        env_vars.insert("REMOTE_ADDR".to_string(), "127.0.0.1".to_string());
        env_vars.insert("REMOTE_HOST".to_string(), String::new());

        // Script filename (absolute path)
        if let Ok(absolute_path) = std::fs::canonicalize(script_path) {
            env_vars.insert("SCRIPT_FILENAME".to_string(), absolute_path.to_string_lossy().to_string());
        }

        // Document root (can be enhanced)
        env_vars.insert("DOCUMENT_ROOT".to_string(), String::new());

        env_vars
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::http::method::Method;
    use crate::http::version::Version;

    #[test]
    fn test_build_cgi_env() {
        let mut request = Request::new(
            Method::GET,
            "/cgi/test.py?param=value".to_string(),
            Version::Http11,
        );
        request.headers.add("Host".to_string(), "localhost:8080".to_string());

        let script_path = PathBuf::from("/var/www/cgi/test.py");
        let env_vars = CgiEnvironment::build(&request, &script_path, "localhost", 8080);

        assert_eq!(env_vars.get("REQUEST_METHOD"), Some(&"GET".to_string()));
        assert_eq!(env_vars.get("QUERY_STRING"), Some(&"param=value".to_string()));
        assert_eq!(env_vars.get("SERVER_NAME"), Some(&"localhost".to_string()));
    }
}
