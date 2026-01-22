use crate::application::config::models::Config;
use crate::application::handler::request_handler::RequestHandler;
use crate::application::handler::router::Router;
use crate::application::handler::session_manager::SessionManager;
use crate::application::handler::static_file_handler::StaticFileHandler;
use crate::application::handler::directory_listing_handler::DirectoryListingHandler;
use crate::application::server::server_instance::ServerInstance;
use crate::common::constants::{DEFAULT_BUFFER_SIZE, DEFAULT_SESSION_TIMEOUT_SECS};
use crate::common::error::{Result, ServerError};
use crate::core::event::event_loop::EventLoop;
use crate::core::event::event_manager::EventManager;
use crate::core::event::poller::Kevent;
use crate::core::net::connection::{Connection, ConnectionState};
use crate::core::net::io::{read_non_blocking, write_non_blocking};
use crate::http::cookie::Cookie;
use crate::http::parser::RequestParser;
use crate::http::request::Request;
use crate::http::response::Response;
use crate::http::serializer::ResponseSerializer;
use std::collections::HashMap;
use std::net::SocketAddr;

/// Manages multiple server instances and coordinates the event loop
pub struct ServerManager {
    /// Server instances
    server_instances: Vec<ServerInstance>,


    /// Event loop for I/O operations
    event_loop: EventLoop,

    /// Event manager for registering file descriptors
    event_manager: EventManager,

    /// Active connections
    connections: HashMap<i32, Connection>,

    /// Request parsers for each connection
    parsers: HashMap<i32, RequestParser>,

    /// Listener FD to port mapping (one listener per port, shared by all servers)
    listener_to_port: HashMap<i32, u16>,

    /// Port to listener mapping (one listener per port, shared by all servers)
    port_to_listener: HashMap<u16, crate::application::server::listener::Listener>,

    /// Default server index for each port (first server configured for that port)
    default_servers: HashMap<u16, usize>,

    /// Server lookup: (port, hostname) -> server index
    /// Used for virtual host routing
    server_lookup: HashMap<(u16, String), usize>,
 
    /// Session manager for handling HTTP sessions
    session_manager: SessionManager,

    /// Maximum client body size from configuration
    max_body_size: usize,
}

impl ServerManager {
    /// Create a new server manager from configuration
    pub fn new(config: Config) -> Result<Self> {
        let event_loop = EventLoop::new()?;
        let poller = event_loop.poller();
        let event_manager = EventManager::new(poller);

        let mut server_instances: Vec<ServerInstance> = Vec::new();
        let mut default_servers = HashMap::new();
        let mut server_lookup = HashMap::new();
        let mut errors = Vec::new();

        // First pass: create all server instances WITHOUT listeners
        // Collect errors but continue creating other servers
        for (idx, server_config) in config.servers.iter().enumerate() {
            // Determine if this should be default for its ports BEFORE creating
            let mut is_default = false;
            for port in &server_config.ports {
                if !default_servers.contains_key(port) {
                    is_default = true;
                }
            }
            
            match ServerInstance::new_without_listeners(server_config.clone(), is_default) {
                Ok(instance) => {
                    // Update default_servers with actual index in server_instances
                    let current_server_idx = server_instances.len();
                    let new_server_name = instance.server_name().to_string();
                    
                    for port in instance.ports() {
                        if !default_servers.contains_key(&port) {
                            default_servers.insert(port, current_server_idx);
                        } else {
                            // Warn about multiple servers on same port
                            // Note: We can't access server_instances here due to borrow checker,
                            // so we'll log this after pushing the instance
                            let existing_idx = default_servers[&port];
                            // Store the existing server name for later warning
                            let existing_server_name = server_instances[existing_idx].server_name().to_string();
                            crate::common::logger::Logger::warn(&format!(
                                "Multiple servers configured for port {}: '{}' (default) and '{}'. \
                                Server selection will use Host header matching, falling back to '{}' if no match.",
                                port,
                                existing_server_name,
                                new_server_name,
                                existing_server_name
                            ));
                        }
                        // Build server lookup: (port, hostname) -> server index
                        let hostname = instance.server_name().to_lowercase();
                        server_lookup.insert((port, hostname), current_server_idx);
                    }
                    server_instances.push(instance);
                }
                Err(e) => {
                    let error_msg = format!(
                        "Failed to create server instance {} (server_name: {:?}): {}",
                        idx,
                        server_config.server_name,
                        e
                    );
                    errors.push(error_msg.clone());
                    crate::common::logger::Logger::error(&error_msg);
                    // Continue with next server
                }
            }
        }

        // Check if we have at least one server instance
        if server_instances.is_empty() {
            let error_msg = format!(
                "Failed to create any server instances. Errors: {}",
                errors.join("; ")
            );
            return Err(ServerError::ConfigError(error_msg));
        }

        // Log errors but continue if we have at least one working server
        if !errors.is_empty() {
            crate::common::logger::Logger::error(&format!(
                "Some server instances failed to start. Errors: {}",
                errors.join("; ")
            ));
        }

        // Second pass: create ONE listener per port (shared by all servers on that port)
        // Group servers by port and create listeners
        let mut port_to_listener: HashMap<u16, crate::application::server::listener::Listener> = HashMap::new();
        let mut listener_to_port = HashMap::new();
        let mut registration_errors = Vec::new();
        
        // Collect all unique ports from all servers
        let mut all_ports = std::collections::HashSet::new();
        for instance in &server_instances {
            for port in instance.ports() {
                all_ports.insert(port);
            }
        }

        // Create one listener per port
        for port in all_ports {
            // Use the first server's address for this port (all servers on same port should use same address)
            let first_server_idx = default_servers.get(&port)
                .copied()
                .ok_or_else(|| ServerError::ConfigError(format!("No server found for port {}", port)))?;
            let first_server = &server_instances[first_server_idx];
            let addr = SocketAddr::new(first_server.config().server_address, port);
            
            match crate::application::server::listener::Listener::new(addr) {
                Ok(listener) => {
                    let fd = listener.as_raw_fd();
                    match event_manager.register_read(fd, fd as usize) {
                        Ok(_) => {
                            port_to_listener.insert(port, listener);
                            listener_to_port.insert(fd, port);
                        }
                        Err(e) => {
                            let error_msg = format!(
                                "Failed to register listener for port {}: {}",
                                port, e
                            );
                            registration_errors.push(error_msg.clone());
                            crate::common::logger::Logger::error(&error_msg);
                        }
                    }
                }
                Err(e) => {
                    let error_msg = format!(
                        "Failed to create listener for port {}: {}",
                        port, e
                    );
                    registration_errors.push(error_msg.clone());
                    crate::common::logger::Logger::error(&error_msg);
                }
            }
        }

        // Log registration errors but continue if we have at least one listener
        if !registration_errors.is_empty() && listener_to_port.is_empty() {
            let error_msg = format!(
                "Failed to register any listeners. Errors: {}",
                registration_errors.join("; ")
            );
            return Err(ServerError::NetworkError(error_msg));
        }

        if !registration_errors.is_empty() {
            crate::common::logger::Logger::error(&format!(
                "Some listeners failed to register. Errors: {}",
                registration_errors.join("; ")
            ));
        }

        Ok(Self {
            event_loop,
            event_manager,
            connections: HashMap::new(),
            parsers: HashMap::new(),
            listener_to_port,
            port_to_listener,
            default_servers,
            server_lookup,
            server_instances,
            session_manager: SessionManager::new(DEFAULT_SESSION_TIMEOUT_SECS),
            max_body_size: config.client_max_body_size,
        })
    }

    /// Print information about all running servers
    pub fn print_server_info(&self) {
        println!("Localhost HTTP Server v0.1.0");
        println!("================================");
        
        for (idx, instance) in self.server_instances.iter().enumerate() {
            let server_name = instance.server_name();
            let address = instance.config().server_address;
            let ports = instance.ports();
            let root = instance.root_path().display();
            
            println!("\nServer {}: {}", idx, server_name);
            println!("  Address: {}", address);
            println!("  Ports: {}", ports.iter().map(|p| p.to_string()).collect::<Vec<_>>().join(", "));
            println!("  Root: {}", root);
            
            if instance.is_default() {
                println!("  Status: Default server for port(s)");
            }
            
            if instance.has_admin_access() {
                println!("  Admin access: Enabled");
            }
            
            // Print routes
            if !instance.config().routes.is_empty() {
                println!("  Routes:");
                for (path, route) in &instance.config().routes {
                    let methods = if route.methods.is_empty() {
                        "ALL".to_string()
                    } else {
                        route.methods.join(", ")
                    };
                    let redirect_info = if let Some(ref redirect) = route.redirect {
                        format!(" -> redirect: {}", redirect)
                    } else {
                        String::new()
                    };
                    println!("    {} -> [{}]{}", path, methods, redirect_info);
                }
            }
            
            // Print CGI handlers
            if !instance.config().cgi_handlers.is_empty() {
                println!("  CGI handlers:");
                for (ext, interpreter) in &instance.config().cgi_handlers {
                    println!("    {} -> {}", ext, interpreter);
                }
            }
        }
        
        println!("\n================================");
        println!("Server is running. Press Ctrl+C to stop.\n");
    }

    /// Run the main server loop
    pub fn run(&mut self) -> Result<()> {
        loop {
            // Wait for events (100ms timeout)
            let events = self.event_loop.wait(100)?;

            // Collect events to process to avoid borrow checker issues
            let mut listener_events = Vec::new();
            let mut client_events = Vec::new();

            // First pass: collect event data
            for event in events {
                let fd = event.ident as i32;
                if let Some(&port) = self.listener_to_port.get(&fd) {
                    listener_events.push((fd, port));
                } else {
                    // Copy event data (kevent is Copy)
                    client_events.push((fd, *event));
                }
            }

            // Second pass: process listener events
            for (fd, port) in listener_events {
                if let Err(e) = self.handle_listener_event(fd, port) {
                    // Log error but continue processing other events
                    crate::common::logger::Logger::error(&format!(
                        "Error handling listener event for fd {}: {}",
                        fd, e
                    ));
                }
            }

            // Third pass: process client events
            for (fd, event) in client_events {
                if let Err(e) = self.handle_client_event(fd, &event) {
                    // Log error but continue processing other events
                    // Note: handle_client_event should not return errors for client events
                    // as errors are handled internally, but we log just in case
                    crate::common::logger::Logger::error(&format!(
                        "Unexpected error handling client event for fd {}: {}",
                        fd, e
                    ));
                }
            }

            // Clean up timed out connections
            if let Err(e) = self.cleanup_connections() {
                // Log cleanup errors but don't stop server
                crate::common::logger::Logger::error(&format!(
                    "Error during connection cleanup: {}",
                    e
                ));
            }
        }
    }

    /// Handle event on a listening socket
    fn handle_listener_event(
        &mut self,
        fd: i32,
        port: u16,
    ) -> Result<()> {
        // Get the listener for this port
        let listener = self.port_to_listener.get_mut(&port)
            .ok_or_else(|| ServerError::NetworkError(format!("No listener found for port {}", port)))?;
        
        match listener.accept() {
            Ok(Some(client_socket)) => {
                let client_fd = client_socket.as_raw_fd();
                // Create connection with port tracking
                let connection = Connection::with_port(client_socket, 30, port); // 30 second timeout
                let parser = RequestParser::with_max_body_size(self.max_body_size);

                self.connections.insert(client_fd, connection);
                self.parsers.insert(client_fd, parser);

                // Register client socket for read events
                if let Err(e) = self.event_manager.register_read(client_fd, client_fd as usize) {
                    // Failed to register - clean up connection
                    self.connections.remove(&client_fd);
                    self.parsers.remove(&client_fd);
                    crate::common::logger::Logger::error(&format!(
                        "Failed to register read event for new connection fd {}: {}",
                        client_fd, e
                    ));
                    return Err(e);
                }
            }
            Ok(None) => {
                // No connection available (non-blocking accept)
                // This is normal, just return
            }
            Err(e) => {
                // Error accepting connection - log but don't crash
                crate::common::logger::Logger::error(&format!(
                    "Error accepting connection on listener fd {}: {}",
                    fd, e
                ));
                return Err(e);
            }
        }
        Ok(())
    }

    /// Get connection or return error
    /// Helper to create "not found" error for resources
    fn not_found_error(resource: &str, id: i32) -> ServerError {
        ServerError::NetworkError(format!("{} {} not found", resource, id))
    }

    fn get_connection(&self, fd: i32) -> Result<&Connection> {
        self.connections.get(&fd)
            .ok_or_else(|| Self::not_found_error("Connection", fd))
    }

    /// Get mutable connection or return error
    fn get_connection_mut(&mut self, fd: i32) -> Result<&mut Connection> {
        self.connections.get_mut(&fd)
            .ok_or_else(|| Self::not_found_error("Connection", fd))
    }

    /// Get parser or return error
    fn get_parser_mut(&mut self, fd: i32) -> Result<&mut RequestParser> {
        self.parsers.get_mut(&fd)
            .ok_or_else(|| Self::not_found_error("Parser", fd))
    }

    /// Get server port from connection (helper to reduce redundancy)
    fn get_connection_port(&self, fd: i32) -> Result<u16> {
        self.get_connection(fd)?
            .server_port()
            .ok_or_else(|| ServerError::HttpError("Connection missing server port".to_string()))
    }

    /// Get default server index for a port (helper to reduce redundancy)
    fn get_default_server_for_port(&self, port: u16) -> Result<usize> {
        self.default_servers.get(&port)
            .copied()
            .ok_or_else(|| ServerError::HttpError(format!("No server found for port {}", port)))
    }

    /// Get server instance by index (helper to reduce redundancy)
    fn get_server_instance(&self, idx: usize) -> Result<&ServerInstance> {
        self.server_instances.get(idx)
            .ok_or_else(|| ServerError::HttpError(format!("Server instance {} not found", idx)))
    }

    /// Write response to connection and register for write events (helper to reduce redundancy)
    fn write_response_to_connection(
        &mut self,
        fd: i32,
        response: &Response,
        keep_alive: bool,
    ) -> Result<()> {
        // Serialize response
        let response_bytes = ResponseSerializer::serialize_auto(response)?;

        // Write response to connection buffer
        {
            let connection = self.get_connection_mut(fd)?;
            connection.set_keep_alive(keep_alive);
            connection.write_buffer_mut().extend(&response_bytes);
            connection.set_state(ConnectionState::Writing);
        }

        // Register for write events
        self.event_manager.register_write(fd, fd as usize)?;

        Ok(())
    }

    /// Handle event on a client connection
    fn handle_client_event(&mut self, fd: i32, _event: &Kevent) -> Result<()> {
        // Get connection state first to avoid borrow issues
        let state = match self.get_connection(fd) {
            Ok(connection) => connection.state().clone(),
            Err(_) => {
                // Connection not found - already closed, ignore
                return Ok(());
            }
        };

        match state {
            ConnectionState::Reading => {
                if let Err(e) = self.handle_read(fd) {
                    // Error already handled in handle_read (connection closed)
                    // Log error but don't propagate to avoid breaking event loop
                    crate::common::logger::Logger::error(&format!(
                        "Error handling read event for fd {}: {}",
                        fd, e
                    ));
                }
            }
            ConnectionState::Writing => {
                if let Err(e) = self.handle_write(fd) {
                    // Error already handled in handle_write (connection closed)
                    // Log error but don't propagate to avoid breaking event loop
                    crate::common::logger::Logger::error(&format!(
                        "Error handling write event for fd {}: {}",
                        fd, e
                    ));
                }
            }
            ConnectionState::Closed => {
                // Connection already marked as closed - clean it up
                let _ = self.close_connection(fd);
            }
        }

        Ok(())
    }

    /// Handle read event - read data and parse request
    fn handle_read(&mut self, fd: i32) -> Result<()> {
        // Read data from socket
        let mut buf = vec![0u8; DEFAULT_BUFFER_SIZE];
        let n = match {
            let connection = self.get_connection_mut(fd)?;
            read_non_blocking(connection.socket_mut(), &mut buf)
        } {
            Ok(n) => n,
            Err(e) => {
                // I/O error occurred - close connection
                self.close_connection_on_error(fd)?;
                return Err(e);
            }
        };

        if n == 0 {
            // Connection closed by client (EOF)
            self.close_connection_on_error(fd)?;
            return Ok(());
        }

        // Add data to parser
        if let Err(e) = self.get_parser_mut(fd)?.add_data(&buf[..n]) {
            // Body size error - send 413 response
            if Self::is_body_size_error(&e) {
                let version = self.get_request_version_or_default(fd);
                return self.send_error_response(fd, crate::http::status::StatusCode::PAYLOAD_TOO_LARGE, version);
            }
            // Other error - close connection
            self.close_connection_on_error(fd)?;
            return Err(e);
        }

        // Try to parse request
        match self.get_parser_mut(fd)?.parse() {
            Ok(Some(request)) => {
                // Request parsed successfully - process it
                if let Err(e) = self.process_request(fd, request) {
                    // Error processing request - close connection
                    self.close_connection_on_error(fd)?;
                    return Err(e);
                }
            }
            Ok(None) => {
                // Need more data - continue reading
            }
            Err(e) => {
                // Check if it's a body size error
                if Self::is_body_size_error(&e) {
                    // Send 413 Payload Too Large response
                    // Try to get HTTP version from parser if available
                    let version = self.get_request_version_or_default(fd);
                    return self.send_error_response(fd, crate::http::status::StatusCode::PAYLOAD_TOO_LARGE, version);
                }
                // Other parse error - close connection
                self.close_connection_on_error(fd)?;
                return Err(e);
            }
        }

        Ok(())
    }

    /// Process a parsed HTTP request
    fn process_request(&mut self, fd: i32, request: Request) -> Result<()> {
        // Get connection to find the port it came in on
        let port = self.get_connection_port(fd)?;
        
        // Log EVERY request at the very start
        crate::common::logger::Logger::info(&format!(
            "═══════════════════════════════════════════════════════════",
        ));
        crate::common::logger::Logger::info(&format!(
            "📥 NEW REQUEST: {} {} on port {}",
            request.method,
            request.path(),
            port
        ));
        
        // Find server instance based on Host header and port
        let server_idx = self.find_server_for_request(&request, port)?;
        let server_instance = self.get_server_instance(server_idx)?;

        // Create router
        let router = Router::new(server_instance.config(), server_instance.root_path().clone());
        
        // Log available routes for this server
        let available_routes: Vec<String> = server_instance.config().routes
            .iter()
            .map(|(path, route)| {
                let redirect_info = route.redirect.as_ref()
                    .map(|r| format!(" -> redirect: {}", r))
                    .unwrap_or_default();
                format!("{}[{}]{}", path, route.methods.join(","), redirect_info)
            })
            .collect();
        crate::common::logger::Logger::info(&format!(
            "Server '{}' (idx: {}) has {} routes: [{}]",
            server_instance.server_name(),
            server_idx,
            available_routes.len(),
            available_routes.join(", ")
        ));
        
        // Log the request path and which server instance is handling it
        crate::common::logger::Logger::info(&format!(
            "Processing request: {} {} (Host: {}) -> Server: '{}' (idx: {})",
            request.method,
            request.path(),
            request.host().map(|h| h.as_str()).unwrap_or("none"),
            server_instance.server_name(),
            server_idx
        ));
        
        // Determine which handler to use based on route
        let route_match = router.match_route_with_path(&request);
        let response = if let Some((matched_path, route)) = route_match {
            // Log matched route with more details including which route path was matched
            crate::common::logger::Logger::info(&format!(
                "✓ Matched route '{}' for request '{}' on server '{}': redirect={:?}, directory={:?}, filename={:?}, methods={:?}",
                matched_path,
                request.path(),
                server_instance.server_name(),
                route.redirect,
                route.directory,
                route.filename,
                route.methods
            ));
            
            // Extra validation: warn if matched path doesn't match request path
            if matched_path != request.path() && !request.path().starts_with(matched_path) {
                crate::common::logger::Logger::warn(&format!(
                    "⚠ Route mismatch detected! Request '{}' matched route '{}'",
                    request.path(),
                    matched_path
                ));
            }
            
            // Check for redirect first (highest priority)
            if route.redirect.is_some() {
                let redirect_value = route.redirect.as_ref().unwrap();
                crate::common::logger::Logger::info(&format!(
                    "→ Redirect detected! Route '{}' has redirect='{}', creating RedirectionHandler",
                    matched_path,
                    redirect_value
                ));
                use crate::application::handler::redirection_handler::RedirectionHandler;
                let handler = RedirectionHandler::new(router);
                handler.handle(&request)?
            } else if request.method == crate::http::method::Method::DELETE {
                // DELETE request - check if route allows DELETE method
                if router.is_method_allowed(&request, route) {
                    // DELETE request - handle file deletion
                    use crate::application::handler::delete_handler::DeleteHandler;
                    let handler = DeleteHandler::new(router);
                    handler.handle(&request)?
                } else {
                    // Route doesn't allow DELETE method
                    Response::method_not_allowed_with_message(
                        request.version,
                        "Method Not Allowed"
                    )
                }
            } else if route.upload_dir.is_some() && request.method == crate::http::method::Method::POST {
                // File upload - check upload_dir before other handlers
                use crate::application::handler::upload_handler::UploadHandler;
                let upload_dir = if let Some(ref dir) = route.upload_dir {
                    router.resolve_path(dir)
                } else {
                    return Err(ServerError::HttpError("Upload directory not configured".to_string()));
                };
                let handler = UploadHandler::new(router, upload_dir);
                handler.handle(&request)?
            } else {
                let file_path = router.resolve_file_path(&request, route)?;
                
                // Check if this is a CGI script
                let is_cgi = route.cgi_extension.is_some() 
                    || (file_path.extension()
                        .and_then(|e| e.to_str())
                        .map(|ext| {
                            let ext_with_dot = format!(".{}", ext);
                            server_instance.config().cgi_handlers.contains_key(&ext_with_dot)
                        })
                        .unwrap_or(false));

                if is_cgi && crate::common::path_utils::is_valid_file(&file_path) {
                    // Execute CGI script
                    use crate::application::handler::cgi_handler::CgiHandler;
                    let cgi_handler = CgiHandler::new(
                        router,
                        server_instance.config().clone(),
                        port, // Use the port from the connection
                    );
                    cgi_handler.handle(&request)?
                } else if file_path.is_dir() {
                    // If directory_listing is enabled, show directory listing instead of default_file
                    if router.is_directory_listing_enabled(route) {
                        let handler = DirectoryListingHandler::new(router);
                        self.handle_with_error_fallback(
                            handler,
                            &request,
                            server_instance,
                            crate::http::status::StatusCode::NOT_FOUND,
                        )?
                    } else if let Some(default_file) = router.get_default_file(route) {
                        // Directory listing disabled, check for default_file
                        let default_path = file_path.join(default_file);
                        if crate::common::path_utils::is_valid_file(&default_path) {
                            // Serve default file via StaticFileHandler
                            let handler = StaticFileHandler::new(router);
                            self.handle_with_error_fallback(
                                handler,
                                &request,
                                server_instance,
                                crate::http::status::StatusCode::NOT_FOUND,
                            )?
                        } else {
                            // Default file doesn't exist and directory listing disabled - return 403
                            Response::forbidden_with_message(request.version, "Forbidden")
                        }
                    } else {
                        // No default_file and directory listing disabled - return 403
                        Response::forbidden_with_message(request.version, "Forbidden")
                    }
                } else {
                    // Static file
                    let handler = StaticFileHandler::new(router);
                    self.handle_with_error_fallback(
                        handler,
                        &request,
                        server_instance,
                        crate::http::status::StatusCode::NOT_FOUND,
                    )?
                }
            }
        } else {
            // No route matched - log and return 404
            crate::common::logger::Logger::warn(&format!(
                "No route matched for: {} {}",
                request.method,
                request.path()
            ));
            self.generate_error_response(
                server_instance,
                crate::http::status::StatusCode::NOT_FOUND,
                request.version,
            )?
        };

        // Handle session management - get or create session
        let mut response = response;
        let session_id = request.cookie(self.session_manager.cookie_name());
        let session_id = self.session_manager.get_or_create_session(session_id.as_deref());
        
        if let Some(sid) = session_id {
            // Set session cookie in response
            let cookie = Cookie::new(
                self.session_manager.cookie_name().to_string(),
                sid.clone(),
            )
            .set_path("/".to_string())
            .set_http_only(true)
            .set_max_age(self.session_manager.timeout_secs());
            
            response.add_cookie(cookie);
        }

        // Cleanup expired sessions periodically (every 100 requests - simplified)
        // In production, use a background task or timer
        static mut CLEANUP_COUNTER: u64 = 0;
        unsafe {
            CLEANUP_COUNTER += 1;
            if CLEANUP_COUNTER % 100 == 0 {
                self.session_manager.cleanup_expired();
            }
        }

        // Write response to connection
        self.write_response_to_connection(fd, &response, request.should_keep_alive())?;

        Ok(())
    }

    /// Send error response to client
    fn send_error_response(
        &mut self,
        fd: i32,
        status_code: crate::http::status::StatusCode,
        version: crate::http::version::Version,
    ) -> Result<()> {
        // Get connection to find the port it came in on
        let port = self.get_connection_port(fd)?;
        
        // Find default server instance for this port
        let server_idx = self.get_default_server_for_port(port)?;
        let server_instance = self.get_server_instance(server_idx)?;
        
        // Generate error response
        let response = self.generate_error_response(server_instance, status_code, version)?;
        
        // Write response to connection (don't keep connection alive after error)
        self.write_response_to_connection(fd, &response, false)?;
        
        Ok(())
    }

    /// Generate error response with custom error page if configured
    fn generate_error_response(
        &self,
        server_instance: &ServerInstance,
        status_code: crate::http::status::StatusCode,
        version: crate::http::version::Version,
    ) -> Result<Response> {
        use crate::application::handler::error_page_handler::ErrorPageHandler;
        let error_handler = ErrorPageHandler::new(
            server_instance.config(),
            server_instance.root_path().clone(),
        );
        error_handler.generate_error_response(status_code, version)
    }

    /// Handle a request handler and return response, falling back to error page on failure
    /// Helper function to reduce redundancy in handler error handling
    fn handle_with_error_fallback<H: RequestHandler>(
        &self,
        handler: H,
        request: &Request,
        server_instance: &ServerInstance,
        error_status: crate::http::status::StatusCode,
    ) -> Result<Response> {
        match handler.handle(request) {
            Ok(response) => Ok(response),
            Err(_) => {
                // Handler failed - use custom error page
                self.generate_error_response(server_instance, error_status, request.version)
            }
        }
    }

    /// Find server instance for a request based on Host header and port
    fn find_server_for_request(&self, request: &Request, port: u16) -> Result<usize> {
        // Log the raw Host header for debugging
        let raw_host = request.host().map(|h| h.as_str()).unwrap_or("(missing)");
        crate::common::logger::Logger::info(&format!(
            "Server selection for {} {}: port={}, Host header='{}'",
            request.method,
            request.path(),
            port,
            raw_host
        ));
        
        // Try to match by Host header and port
        if let Some(host) = request.host() {
            // Extract hostname (remove port if present)
            let hostname = host.split(':').next().unwrap_or(host).to_lowercase();
            
            // Handle common localhost variations: 127.0.0.1 and ::1 should match "localhost"
            let normalized_hostname = if hostname == "127.0.0.1" || hostname == "::1" || hostname == "[::1]" {
                "localhost".to_string()
            } else {
                hostname.clone()
            };
            
            // Log available servers for this port for debugging
            let available_servers: Vec<String> = self.server_lookup
                .iter()
                .filter(|((p, _), _)| *p == port)
                .map(|((_, h), idx)| {
                    format!("'{}' (idx: {})", h, idx)
                })
                .collect();
            
            // Log default server for this port
            let default_idx = self.default_servers.get(&port).copied();
            let default_info = if let Some(idx) = default_idx {
                if let Ok(server) = self.get_server_instance(idx) {
                    format!("'{}' (idx: {})", server.server_name(), idx)
                } else {
                    format!("(idx: {})", idx)
                }
            } else {
                "none".to_string()
            };
            
            crate::common::logger::Logger::info(&format!(
                "Looking for server match: Host header='{}' (normalized from '{}' -> '{}'), port={}, available servers: [{}], default: {}",
                normalized_hostname,
                hostname,
                host,
                port,
                available_servers.join(", "),
                default_info
            ));
            
            // Try exact match first
            if let Some(&server_idx) = self.server_lookup.get(&(port, normalized_hostname.clone())) {
                let server_instance = self.get_server_instance(server_idx)?;
                crate::common::logger::Logger::info(&format!(
                    "Request {} {} -> Resolved server_name: '{}' (matched by Host header: '{}') on port {}",
                    request.method,
                    request.path(),
                    server_instance.config().server_name,
                    normalized_hostname,
                    port
                ));
                return Ok(server_idx);
            }
            
            // Try original hostname if normalization changed it
            if normalized_hostname != hostname {
                if let Some(&server_idx) = self.server_lookup.get(&(port, hostname.clone())) {
                    let server_instance = self.get_server_instance(server_idx)?;
                    crate::common::logger::Logger::info(&format!(
                        "Request {} {} -> Resolved server_name: '{}' (matched by Host header: '{}') on port {}",
                        request.method,
                        request.path(),
                        server_instance.config().server_name,
                        hostname,
                        port
                    ));
                    return Ok(server_idx);
                }
            }
            
            crate::common::logger::Logger::warn(&format!(
                "No server match found for Host header '{}' (normalized: '{}', original: '{}') on port {}, falling back to default server",
                normalized_hostname,
                hostname,
                host,
                port
            ));
        } else {
            crate::common::logger::Logger::warn(&format!(
                "No Host header present for request {} {} on port {}, falling back to default server",
                request.method,
                request.path(),
                port
            ));
        }

        // Fall back to default server for this port
        let server_idx = self.get_default_server_for_port(port)?;
        let server_instance = self.get_server_instance(server_idx)?;
        let host_header = request.host().map(|h| h.as_str()).unwrap_or("none");
        crate::common::logger::Logger::info(&format!(
            "Request {} {} -> Resolved server_name: '{}' (default server, Host header: '{}') on port {}",
            request.method,
            request.path(),
            server_instance.config().server_name,
            host_header,
            port
        ));
        Ok(server_idx)
    }

    /// Handle write event - send response to client
    fn handle_write(&mut self, fd: i32) -> Result<()> {
        // Get data to write first
        let data: Vec<u8> = {
            let connection = self.get_connection_mut(fd)?;
            let write_buffer = connection.write_buffer_mut();
            if write_buffer.is_empty() {
                // Nothing to write
                connection.set_state(ConnectionState::Reading);
                // Connection reference dropped at end of this block
                Vec::new()
            } else {
                // Copy the data so we can drop the connection reference
                write_buffer.as_slice().to_vec()
            }
        };
        
        // Handle empty write buffer case after dropping connection reference
        if data.is_empty() {
            if let Err(e) = self.event_manager.unregister_write(fd) {
                // Error unregistering - close connection
                self.set_connection_state_and_close(fd, ConnectionState::Closed)?;
                return Err(e);
            }
            return Ok(());
        }

        // Write data
        let n = match {
            let connection = self.get_connection_mut(fd)?;
            let socket = connection.socket_mut();
            write_non_blocking(socket, &data)
        } {
            Ok(n) => n,
            Err(e) => {
                // I/O error occurred - close connection
                self.close_connection_on_error(fd)?;
                return Err(e);
            }
        };

        if n > 0 {
            // Remove written data from buffer
            self.get_connection_mut(fd)?.write_buffer_mut().drain(n);
        }

        // Check if all data sent
        let is_empty = self.get_connection(fd)?.write_buffer().is_empty();
        if is_empty {
            // All data sent
            let should_keep_alive = self.get_connection(fd)?.should_keep_alive();
            if should_keep_alive {
                // Reset for next request
                {
                    let connection = self.get_connection_mut(fd)?;
                    connection.set_state(ConnectionState::Reading);
                    connection.read_buffer_mut().clear();
                }
                // Reset parser after dropping connection reference
                if let Some(parser) = self.parsers.get_mut(&fd) {
                    parser.reset();
                }
                // Unregister write after dropping connection reference
                if let Err(e) = self.event_manager.unregister_write(fd) {
                    // Error unregistering - close connection
                    self.set_connection_state_and_close(fd, ConnectionState::Closed)?;
                    return Err(e);
                }
            } else {
                // Close connection
                self.close_connection_on_error(fd)?;
            }
        }

        Ok(())
    }

    /// Clean up timed out or closed connections
    fn cleanup_connections(&mut self) -> Result<()> {
        let mut to_remove = Vec::new();

        for (fd, connection) in &self.connections {
            if connection.is_timeout() {
                to_remove.push(*fd);
            }
        }

        for fd in to_remove {
            self.close_connection(fd)?;
        }

        Ok(())
    }

    /// Check if error is a body size violation
    /// Checks for all possible body size error message patterns from the parser
    fn is_body_size_error(error: &ServerError) -> bool {
        if let ServerError::HttpError(ref msg) = error {
            // Match all body size error patterns used in RequestParser:
            // - "exceeds maximum allowed size" (used in multiple places)
            // - "would exceed maximum allowed size" (used in add_data and chunked parsing)
            msg.contains("exceeds maximum allowed size") 
                || msg.contains("would exceed maximum allowed size")
        } else {
            false
        }
    }

    /// Get HTTP version from parser if available, otherwise default to HTTP/1.1
    fn get_request_version_or_default(&self, _fd: i32) -> crate::http::version::Version {
        // Try to get version from partially parsed request if available
        // The parser's request field is private, so we default to HTTP/1.1
        // This is acceptable since HTTP/1.1 is the most common version
        crate::http::version::Version::Http11
    }

    /// Close connection on error - helper to reduce code duplication
    fn close_connection_on_error(&mut self, fd: i32) -> Result<()> {
        self.set_connection_state_and_close(fd, ConnectionState::Closed)
    }

    /// Set connection state and close connection (helper to reduce redundancy)
    fn set_connection_state_and_close(&mut self, fd: i32, state: ConnectionState) -> Result<()> {
        if let Ok(connection) = self.get_connection_mut(fd) {
            connection.set_state(state);
        }
        self.close_connection(fd)
    }

    /// Close a connection and clean up resources
    fn close_connection(&mut self, fd: i32) -> Result<()> {
        // Try to unregister events, but ignore errors if they're already unregistered
        // This can happen during cleanup or when connection is already closed
        let _ = self.event_manager.unregister_read(fd);
        let _ = self.event_manager.unregister_write(fd);
        
        self.connections.remove(&fd);
        self.parsers.remove(&fd);
        Ok(())
    }
}
