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

    /// Server index by (address, port)
    servers: HashMap<(SocketAddr, u16), usize>,

    /// Default server index for each port (first server for that port)
    default_servers: HashMap<u16, usize>,

    /// Event loop for I/O operations
    event_loop: EventLoop,

    /// Event manager for registering file descriptors
    event_manager: EventManager,

    /// Active connections
    connections: HashMap<i32, Connection>,

    /// Request parsers for each connection
    parsers: HashMap<i32, RequestParser>,

    /// Listener FD to (server_index, address, port) mapping
    listener_to_server: HashMap<i32, (usize, SocketAddr, u16)>,

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

        let mut server_instances = Vec::new();
        let mut default_servers = HashMap::new();
        let mut errors = Vec::new();

        // First pass: create all server instances and determine defaults
        // Collect errors but continue creating other servers
        for (idx, server_config) in config.servers.iter().enumerate() {
            // Determine if this should be default for its ports BEFORE creating
            // (we need to check original config ports, not instance ports)
            let mut is_default = false;
            for port in &server_config.ports {
                if !default_servers.contains_key(port) {
                    // Will set to actual index after we know if creation succeeds
                    is_default = true;
                }
            }
            
            match ServerInstance::new(server_config.clone(), is_default) {
                Ok(instance) => {
                    // Update default_servers with actual index in server_instances
                    for port in instance.ports() {
                        if !default_servers.contains_key(&port) {
                            default_servers.insert(port, server_instances.len());
                        }
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

        // Second pass: register listeners with event loop
        // Collect registration errors but continue
        let mut listener_to_server = HashMap::new();
        let mut registration_errors = Vec::new();
        
        for (idx, instance) in server_instances.iter().enumerate() {
            for port in instance.ports() {
                let addr = SocketAddr::new(instance.config().server_address, port);
                if let Some(listener) = instance.listener(port) {
                    let fd = listener.as_raw_fd();
                    match event_manager.register_read(fd, fd as usize) {
                        Ok(_) => {
                            listener_to_server.insert(fd, (idx, addr, port));
                        }
                        Err(e) => {
                            let error_msg = format!(
                                "Failed to register listener for server {} on port {}: {}",
                                idx, port, e
                            );
                            registration_errors.push(error_msg.clone());
                            crate::common::logger::Logger::error(&error_msg);
                            // Continue with next listener
                        }
                    }
                }
            }
        }

        // Log registration errors but continue if we have at least one listener
        if !registration_errors.is_empty() && listener_to_server.is_empty() {
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

        // Build servers map from instances
        let mut servers = HashMap::new();
        for (idx, instance) in server_instances.iter().enumerate() {
            for port in instance.ports() {
                let addr = SocketAddr::new(instance.config().server_address, port);
                servers.insert((addr, port), idx);
            }
        }

        Ok(Self {
            servers,
            default_servers,
            event_loop,
            event_manager,
            connections: HashMap::new(),
            parsers: HashMap::new(),
            listener_to_server,
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
                    println!("    {} -> [{}]", path, methods);
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
                if let Some(&(server_idx, addr, port)) = self.listener_to_server.get(&fd) {
                    listener_events.push((fd, server_idx, addr, port));
                } else {
                    // Copy event data (kevent is Copy)
                    client_events.push((fd, *event));
                }
            }

            // Second pass: process listener events
            for (fd, _server_idx, addr, port) in listener_events {
                if let Err(e) = self.handle_listener_event(fd, addr, port) {
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
        _addr: SocketAddr,
        _port: u16,
    ) -> Result<()> {
        // Get server index from listener mapping
        if let Some(&(server_idx, _addr, port)) = self.listener_to_server.get(&fd) {
            // Get listener from server instance
            let listener = self.server_instances
                .get(server_idx)
                .and_then(|instance| instance.listener(port));

            if let Some(listener) = listener {
                match listener.accept() {
                    Ok(Some(client_socket)) => {
                        let client_fd = client_socket.as_raw_fd();
                        let connection = Connection::new(client_socket, 30); // 30 second timeout
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
                return self.send_error_response(fd, crate::http::status::StatusCode::PAYLOAD_TOO_LARGE, crate::http::version::Version::Http11);
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
                    // Use HTTP/1.1 as default version (we don't have request yet)
                    return self.send_error_response(fd, crate::http::status::StatusCode::PAYLOAD_TOO_LARGE, crate::http::version::Version::Http11);
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
        // Find server instance based on Host header
        let server_idx = self.find_server_for_request(&request)?;
        let server_instance = &self.server_instances[server_idx];

        // Create router
        let router = Router::new(server_instance.config(), server_instance.root_path().clone());
        
        // Determine which handler to use based on route
        let route = router.match_route(&request);
        let response = if let Some(route) = route {
            // Check for redirect first (highest priority)
            if route.redirect.is_some() {
                use crate::application::handler::redirection_handler::RedirectionHandler;
                let handler = RedirectionHandler::new(router);
                handler.handle(&request)?
            } else if request.method == crate::http::method::Method::DELETE {
                // DELETE request - handle file deletion
                use crate::application::handler::delete_handler::DeleteHandler;
                let handler = DeleteHandler::new(router);
                handler.handle(&request)?
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

                if is_cgi && file_path.exists() && file_path.is_file() {
                    // Execute CGI script
                    use crate::application::handler::cgi_handler::CgiHandler;
                    let cgi_handler = CgiHandler::new(
                        router,
                        server_instance.config().clone(),
                        server_instance.ports()[0], // Use first port
                    );
                    cgi_handler.handle(&request)?
                } else if file_path.is_dir() && router.is_directory_listing_enabled(route) {
                    // Directory listing
                    let handler = DirectoryListingHandler::new(router);
                    handler.handle(&request)?
                } else {
                    // Static file
                    let handler = StaticFileHandler::new(router);
                    handler.handle(&request)?
                }
            }
        } else {
            // No route matched - return 404 with custom error page if configured
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

        // Serialize response
        let response_bytes = ResponseSerializer::serialize_auto(&response)?;

        // Set keep-alive based on request and write response
        {
            let connection = self.get_connection_mut(fd)?;
            connection.set_keep_alive(request.should_keep_alive());
            connection.write_buffer_mut().extend(&response_bytes);
            connection.set_state(ConnectionState::Writing);
        }

        // Register for write events
        self.event_manager.register_write(fd, fd as usize)?;

        Ok(())
    }

    /// Send error response to client
    fn send_error_response(
        &mut self,
        fd: i32,
        status_code: crate::http::status::StatusCode,
        version: crate::http::version::Version,
    ) -> Result<()> {
        // Find default server instance for error page
        let server_instance = self.server_instances.first()
            .ok_or_else(|| ServerError::HttpError("No server instances available".to_string()))?;
        
        // Generate error response
        let response = self.generate_error_response(server_instance, status_code, version)?;
        
        // Serialize response
        let response_bytes = ResponseSerializer::serialize_auto(&response)?;
        
        // Write response to connection buffer
        {
            let connection = self.get_connection_mut(fd)?;
            connection.write_buffer_mut().extend(&response_bytes);
            connection.set_state(ConnectionState::Writing);
            // Don't keep connection alive after error
            connection.set_keep_alive(false);
        }
        
        // Register for write events
        self.event_manager.register_write(fd, fd as usize)?;
        
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

    /// Find server instance for a request based on Host header
    fn find_server_for_request(&self, request: &Request) -> Result<usize> {
        // Try to match by Host header
        if let Some(host) = request.host() {
            // Extract hostname (remove port if present)
            let hostname = host.split(':').next().unwrap_or(host);
            
            // Find server by name
            for (idx, instance) in self.server_instances.iter().enumerate() {
                if instance.server_name() == hostname {
                    return Ok(idx);
                }
            }
        }

        // Fall back to default server for the port
        // For now, return first server (we'll improve this later)
        Ok(0)
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
                let connection = self.get_connection_mut(fd)?;
                connection.set_state(ConnectionState::Closed);
                self.close_connection(fd)?;
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
                    let connection = self.get_connection_mut(fd)?;
                    connection.set_state(ConnectionState::Closed);
                    self.close_connection(fd)?;
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
    fn is_body_size_error(error: &ServerError) -> bool {
        if let ServerError::HttpError(ref msg) = error {
            msg.contains("exceeds maximum allowed size")
        } else {
            false
        }
    }

    /// Close connection on error - helper to reduce code duplication
    fn close_connection_on_error(&mut self, fd: i32) -> Result<()> {
        if let Ok(connection) = self.get_connection_mut(fd) {
            connection.set_state(ConnectionState::Closed);
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
