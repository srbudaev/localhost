use crate::application::config::models::Config;
use crate::application::handler::request_handler::RequestHandler;
use crate::application::handler::router::Router;
use crate::application::handler::static_file_handler::StaticFileHandler;
use crate::application::handler::directory_listing_handler::DirectoryListingHandler;
use crate::application::server::server_instance::ServerInstance;
use crate::common::constants::DEFAULT_BUFFER_SIZE;
use crate::common::error::{Result, ServerError};
use crate::core::event::event_loop::EventLoop;
use crate::core::event::event_manager::EventManager;
use crate::core::event::poller::Kevent;
use crate::core::net::connection::{Connection, ConnectionState};
use crate::core::net::io::{read_non_blocking, write_non_blocking};
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
}

impl ServerManager {
    /// Create a new server manager from configuration
    pub fn new(config: Config) -> Result<Self> {
        let event_loop = EventLoop::new()?;
        let poller = event_loop.poller();
        let event_manager = EventManager::new(poller);

        let mut server_instances = Vec::new();
        let mut default_servers = HashMap::new();

        // First pass: create all server instances and determine defaults
        for (idx, server_config) in config.servers.iter().enumerate() {
            // First server for each port becomes default
            let mut is_default = false;
            for port in &server_config.ports {
                if !default_servers.contains_key(port) {
                    default_servers.insert(*port, idx);
                    is_default = true;
                }
            }

            let instance = ServerInstance::new(server_config.clone(), is_default)?;
            server_instances.push(instance);
        }

        // Second pass: register listeners with event loop
        let mut listener_to_server = HashMap::new();
        for (idx, instance) in server_instances.iter().enumerate() {
            for port in instance.ports() {
                let addr = SocketAddr::new(instance.config().server_address, port);
                if let Some(listener) = instance.listener(port) {
                    let fd = listener.as_raw_fd();
                    listener_to_server.insert(fd, (idx, addr, port));
                    event_manager.register_read(fd, fd as usize)?;
                }
            }
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
        })
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
                self.handle_listener_event(fd, addr, port)?;
            }

            // Third pass: process client events
            for (fd, event) in client_events {
                self.handle_client_event(fd, &event)?;
            }

            // Clean up timed out connections
            self.cleanup_connections()?;
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
                if let Some(client_socket) = listener.accept()? {
                    let client_fd = client_socket.as_raw_fd();
                    let connection = Connection::new(client_socket, 30); // 30 second timeout
                    let parser = RequestParser::new();

                    self.connections.insert(client_fd, connection);
                    self.parsers.insert(client_fd, parser);

                    // Register client socket for read events
                    self.event_manager
                        .register_read(client_fd, client_fd as usize)?;
                }
            }
        }
        Ok(())
    }

    /// Get connection or return error
    fn get_connection(&self, fd: i32) -> Result<&Connection> {
        self.connections.get(&fd)
            .ok_or_else(|| ServerError::NetworkError(format!("Connection {} not found", fd)))
    }

    /// Get mutable connection or return error
    fn get_connection_mut(&mut self, fd: i32) -> Result<&mut Connection> {
        self.connections.get_mut(&fd)
            .ok_or_else(|| ServerError::NetworkError(format!("Connection {} not found", fd)))
    }

    /// Get parser or return error
    fn get_parser_mut(&mut self, fd: i32) -> Result<&mut RequestParser> {
        self.parsers.get_mut(&fd)
            .ok_or_else(|| ServerError::NetworkError(format!("Parser {} not found", fd)))
    }

    /// Handle event on a client connection
    fn handle_client_event(&mut self, fd: i32, _event: &Kevent) -> Result<()> {
        // Get connection state first to avoid borrow issues
        let state = {
            let connection = self.get_connection(fd)?;
            connection.state().clone()
        };

        match state {
            ConnectionState::Reading => {
                self.handle_read(fd)?;
            }
            ConnectionState::Writing => {
                self.handle_write(fd)?;
            }
            ConnectionState::Closed => {
                self.close_connection(fd)?;
            }
        }

        Ok(())
    }

    /// Handle read event - read data and parse request
    fn handle_read(&mut self, fd: i32) -> Result<()> {
        // Read data from socket
        let mut buf = vec![0u8; DEFAULT_BUFFER_SIZE];
        let n = {
            let connection = self.get_connection_mut(fd)?;
            read_non_blocking(connection.socket_mut(), &mut buf)?
        };

        if n == 0 {
            // Connection closed by client
            self.get_connection_mut(fd)?.set_state(ConnectionState::Closed);
            return Ok(());
        }

        // Add data to parser
        self.get_parser_mut(fd)?.add_data(&buf[..n]);

        // Try to parse request
        if let Some(request) = self.get_parser_mut(fd)?.parse()? {
            // Request parsed successfully - process it
            self.process_request(fd, request)?;
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
            let file_path = router.resolve_file_path(&request, route)?;
            
            // Check if this is a CGI script
            let is_cgi = route.cgi_extension.is_some() 
                || (file_path.extension()
                    .and_then(|e| e.to_str())
                    .map(|ext| server_instance.config().cgi_handlers.contains_key(ext))
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
        } else {
            // No route matched - return 404
            let mut response = Response::not_found(request.version);
            response.set_body_str("Not Found");
            response
        };

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
        let connection = self.get_connection_mut(fd)?;

        // Get data to write first
        let data: Vec<u8> = {
            let write_buffer = connection.write_buffer_mut();
            if write_buffer.is_empty() {
                // Nothing to write
                connection.set_state(ConnectionState::Reading);
                self.event_manager.unregister_write(fd)?;
                return Ok(());
            }
            write_buffer.as_slice()
        };

        // Write data
        let n = {
            let socket = connection.socket_mut();
            write_non_blocking(socket, &data)?
        };

        if n > 0 {
            // Remove written data from buffer
            connection.write_buffer_mut().drain(n);
        }

        // Check if all data sent
        let is_empty = connection.write_buffer().is_empty();
        if is_empty {
            // All data sent
            if connection.should_keep_alive() {
                // Reset for next request
                connection.set_state(ConnectionState::Reading);
                connection.read_buffer_mut().clear();
                if let Some(parser) = self.parsers.get_mut(&fd) {
                    parser.reset();
                }
                self.event_manager.unregister_write(fd)?;
            } else {
                // Close connection
                connection.set_state(ConnectionState::Closed);
                self.close_connection(fd)?;
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

    /// Close a connection and clean up resources
    fn close_connection(&mut self, fd: i32) -> Result<()> {
        self.event_manager.unregister_read(fd)?;
        self.event_manager.unregister_write(fd)?;
        self.connections.remove(&fd);
        self.parsers.remove(&fd);
        Ok(())
    }
}
