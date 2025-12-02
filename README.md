# Localhost - HTTP/1.1 Server

High-performance HTTP/1.1 server built from scratch in Rust with event-driven architecture.

## Current Project Status

### ✅ Implemented (Basic Infrastructure)

#### Core Modules (648+ lines of code)
- **core::net** - Network Infrastructure
  - `fd.rs` - File descriptor wrapper with automatic resource management
  - `socket.rs` - Listening and client sockets with non-blocking mode
  - `io.rs` - Non-blocking read/write operations
  - `connection.rs` - Connection state management (read/write, timeouts, keep-alive)

- **core::event** - Event-driven Architecture
  - `poller.rs` - kqueue polling for macOS (ready for epoll extension on Linux)
  - `event.rs` - Event structure
  - `event_manager.rs` - Event registration/deregistration
  - `event_loop.rs` - Main event loop for I/O event processing

#### Common Modules (Utilities)
- `error.rs` - Centralized error handling system
- `constants.rs` - Constants (timeouts, buffer sizes, HTTP constants)
- `buffer.rs` - Buffer for incremental read/write operations
- `time.rs` - Request timeout management
- `logger.rs` - Simple logging system

### 🚧 In Development

#### HTTP Modules (Placeholders)
- `request.rs`, `response.rs`, `parser.rs`, `serializer.rs` - HTTP/1.1 parsing and generation
- `method.rs`, `status.rs`, `headers.rs`, `body.rs`, `cookie.rs` - HTTP components

#### Application Modules (Placeholders)
- `config/` - Configuration file parsing (TOML)
- `server/` - Server instances and listeners management
- `handler/` - Request routing and handlers
- `cgi/` - CGI script execution

## Architecture

```
localhost/
├── src/
│   ├── bin/main.rs          # Entry point
│   ├── core/                 # Low-level infrastructure
│   │   ├── event/            # Event polling (kqueue/epoll)
│   │   └── net/              # Network operations
│   ├── http/                 # HTTP/1.1 protocol
│   ├── application/          # Business logic
│   │   ├── config/           # Configuration
│   │   ├── server/           # Server instances
│   │   ├── handler/          # Request handlers
│   │   └── cgi/              # CGI execution
│   └── common/               # Utilities
```

## What the Program Does Now

**Current Status:** Basic infrastructure is ready, but the program **does not run** yet, as implementations for `application::config::loader` and `application::server::server_manager` modules are missing.

### What Works:
- ✅ Library compiles successfully (`cargo check --lib` passes)
- ✅ Event polling system (kqueue) implemented
- ✅ Non-blocking network operations
- ✅ Connection and timeout management
- ✅ Error handling and logging system

### What Doesn't Work:
- ❌ Program doesn't start (missing modules in `main.rs`)
- ❌ HTTP parsing not implemented
- ❌ Configuration parsing not implemented
- ❌ Server instances not created

## Next Steps

1. Implement HTTP request parsing (`http::request`, `http::parser`)
2. Implement HTTP response generation (`http::response`, `http::serializer`)
3. Implement configuration loading (`application::config`)
4. Implement server manager and listeners
5. Implement basic routing and handlers

## Building

```bash
# Check library (works)
cargo check --lib

# Full build (doesn't work due to missing modules)
cargo build

# After implementing modules:
cargo run -- <config_file>
```

## Technologies

- **Rust** (edition 2021)
- **libc** - for system calls (kqueue on macOS)
- **serde**, **toml** - for configuration parsing (in development)

## License

[Specify license]
