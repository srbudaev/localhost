# Localhost HTTP Server

A high-performance, asynchronous HTTP/1.1 web server written in Rust. This server provides a lightweight alternative to traditional web servers with support for static file serving, CGI scripts, file uploads, virtual hosting, and more.

## Features

- **HTTP/1.1 Support**: Full HTTP/1.1 protocol implementation with keep-alive connections
- **Asynchronous I/O**: Event-driven architecture using kqueue/epoll for high concurrency
- **Virtual Hosting**: Multiple server instances on different ports with hostname-based routing
- **Static File Serving**: Efficient serving of static files with directory listing support
- **CGI Support**: Execute Python and shell scripts via CGI protocol
- **File Uploads**: Handle multipart/form-data file uploads with automatic directory creation
- **Custom Error Pages**: Configurable error pages for 400, 403, 404, 405, 413, 500 status codes
- **HTTP Redirects**: Support for both 301 (permanent) and 302 (temporary) redirects
- **DELETE Method**: Safe file deletion with proper validation
- **Session Management**: HTTP session handling with configurable timeouts
- **Chunked Transfer Encoding**: Support for chunked request/response bodies
- **Request Body Size Limits**: Configurable maximum body size for uploads

## Installation

### Prerequisites

- Rust 1.70+ (2021 edition)
- Cargo package manager

### Build

```bash
cargo build --release
```

### Run

```bash
cargo run config.example.toml
```

Or use the release binary:

```bash
./target/release/localhost config.example.toml
```

## Configuration

The server is configured via a TOML configuration file. See `config.example.toml` for a complete example.

### Basic Configuration

```toml
client_timeout_secs = 30
client_max_body_size = 10485760  # 10MB

[[servers]]
server_address = "127.0.0.1"
ports = [8080]
server_name = "localhost"
root = "./root"
admin_access = false

[servers.cgi_handlers]
".py" = "python3"
".sh" = "/bin/sh"

[servers.routes."/"]
methods = ["GET", "POST", "DELETE"]
directory = "."
default_file = "index.html"
directory_listing = true
```

### Multiple Server Instances

You can configure multiple server instances for different ports or virtual hosts:

```toml
[[servers]]
server_address = "127.0.0.1"
ports = [8080]
server_name = "localhost"
root = "./root"

[[servers]]
server_address = "127.0.0.1"
ports = [8081]
server_name = "anotherhost"
root = "./root"
```

## Architecture

The server follows a modular, event-driven architecture with clear separation of concerns:

### High-Level Architecture

```
┌─────────────────────────────────────────────────────────────┐
│                      ServerManager                         │
│  - Manages multiple ServerInstances                        │
│  - Coordinates EventLoop and EventManager                  │
│  - Handles connection lifecycle                             │
└─────────────────────────────────────────────────────────────┘
                            │
        ┌───────────────────┼───────────────────┐
        │                   │                   │
        ▼                   ▼                   ▼
┌──────────────┐   ┌──────────────┐   ┌──────────────┐
│  EventLoop   │   │   Router     │   │  Handlers    │
│  (kqueue/    │   │  - Route     │   │  - Static    │
│   epoll)     │   │    matching  │   │  - CGI       │
└──────────────┘   └──────────────┘   │  - Upload    │
                                      │  - Redirect  │
                                      │  - Delete    │
                                      └──────────────┘
```

### Core Modules

#### 1. **Core Layer** (`src/core/`)

**Event System** (`core/event/`):
- `EventLoop`: Main event loop using kqueue (macOS/BSD) or epoll (Linux)
- `EventManager`: Manages file descriptor registration and event notifications
- `Poller`: Platform-specific polling abstraction

**Network Layer** (`core/net/`):
- `Connection`: Represents a client connection with read/write buffers
- `Socket`: Low-level socket operations (TCP server/client sockets)
- `IO`: Non-blocking I/O utilities

#### 2. **HTTP Layer** (`src/http/`)

- `Request`: HTTP request structure with method, path, headers, body
- `Response`: HTTP response structure with status, headers, body
- `Parser`: Incremental HTTP request parser with chunked encoding support
- `Serializer`: HTTP response serializer
- `Method`: HTTP method enum (GET, POST, DELETE, etc.)
- `Status`: HTTP status codes and reason phrases
- `Version`: HTTP version (currently only HTTP/1.1)
- `Headers`: HTTP headers management
- `Cookie`: Cookie parsing and handling

#### 3. **Application Layer** (`src/application/`)

**Server Management** (`application/server/`):
- `ServerManager`: Central coordinator managing all server instances
- `ServerInstance`: Represents a configured server with routes and handlers
- `Listener`: TCP listener wrapper for accepting connections

**Request Handlers** (`application/handler/`):
- `Router`: Route matching and path resolution
- `StaticFileHandler`: Serves static files with MIME type detection
- `DirectoryListingHandler`: Generates directory listings
- `CgiHandler`: Executes CGI scripts (Python, shell)
- `UploadHandler`: Handles file uploads with multipart parsing
- `RedirectionHandler`: HTTP redirects (301/302)
- `DeleteHandler`: Safe file deletion
- `ErrorPageHandler`: Custom error page generation
- `SessionManager`: HTTP session management

**CGI System** (`application/cgi/`):
- `CgiExecutor`: Executes CGI scripts with proper environment setup
- `CgiProcess`: Manages CGI process lifecycle
- `CgiIO`: Handles CGI input/output streams
- `CgiEnv`: Sets up CGI environment variables

**Configuration** (`application/config/`):
- `Loader`: Configuration file loading and parsing
- `Models`: Configuration data structures
- `Parser`: TOML parsing
- `Validator`: Configuration validation

#### 4. **Common Utilities** (`src/common/`)

- `Error`: Unified error types (`ServerError`, `Result`)
- `Logger`: Simple logging utility
- `Buffer`: Efficient buffer management for I/O
- `PathUtils`: Path validation and sanitization
- `Time`: Timeout and time utilities
- `Constants`: Default values and constants

### Request Flow

1. **Connection Accepted**: `ServerManager` accepts new TCP connection
2. **Event Registration**: Connection registered with `EventManager` for read events
3. **Data Reading**: Non-blocking read from socket into connection buffer
4. **Parsing**: `RequestParser` incrementally parses HTTP request
5. **Routing**: `Router` matches request path to configured route
6. **Handler Selection**: Appropriate handler selected based on route configuration
7. **Processing**: Handler processes request and generates `Response`
8. **Serialization**: `ResponseSerializer` converts response to HTTP bytes
9. **Writing**: Non-blocking write to socket
10. **Cleanup**: Connection closed or kept alive for next request

### Event Loop Architecture

The server uses an event-driven architecture:

```
┌─────────────────────────────────────┐
│         Event Loop (run loop)        │
│                                      │
│  1. Poll for events (kqueue/epoll)  │
│  2. Process events:                  │
│     - Listener events → accept()     │
│     - Read events → read data        │
│     - Write events → write data      │
│  3. Handle timeouts                  │
│  4. Cleanup closed connections       │
└─────────────────────────────────────┘
```

### Handler Pattern

All handlers implement the `RequestHandler` trait:

```rust
pub trait RequestHandler {
    fn handle(&self, request: &Request) -> Result<Response>;
}
```

This allows for:
- Consistent error handling
- Easy testing
- Pluggable handler architecture

## Project Structure

```
localhost/
├── src/
│   ├── bin/
│   │   └── main.rs              # Entry point
│   ├── lib.rs                   # Library root
│   ├── core/                    # Core system components
│   │   ├── event/               # Event loop and polling
│   │   └── net/                 # Network abstractions
│   ├── http/                    # HTTP protocol implementation
│   │   ├── request.rs
│   │   ├── response.rs
│   │   ├── parser.rs
│   │   ├── serializer.rs
│   │   └── ...
│   ├── application/             # Application logic
│   │   ├── server/              # Server management
│   │   ├── handler/             # Request handlers
│   │   ├── cgi/                 # CGI execution
│   │   └── config/              # Configuration
│   └── common/                  # Shared utilities
├── root/                        # Document root
│   ├── index.html
│   ├── cgi-bin/
│   ├── uploads/
│   └── errors/
├── config.example.toml         # Example configuration
├── Cargo.toml
└── README.md
```

## Usage Examples

### Static File Serving

```toml
[servers.routes."/static"]
methods = ["GET"]
directory = "./static"
default_file = "index.html"
directory_listing = true
```

### CGI Scripts

```toml
[servers.cgi_handlers]
".py" = "python3"

[servers.routes."/cgi-bin"]
methods = ["GET", "POST"]
directory = "cgi-bin"
cgi_extension = "py"
```

### File Uploads

```toml
[servers.routes."/upload"]
methods = ["POST"]
upload_dir = "uploads"
```

### Redirects

```toml
[servers.routes."/old"]
methods = ["GET"]
redirect = "https://example.com"
redirect_type = "301"  # Permanent redirect
```

### Custom Error Pages

```toml
[servers.errors]
"404" = { filename = "errors/404.html" }
"500" = { filename = "errors/500.html" }
```

## Testing

Run tests:

```bash
cargo test
```

Run specific test suites:

```bash
cargo test --lib http::version
cargo test --test integration_tests
```

## Performance Considerations

- **Non-blocking I/O**: All socket operations are non-blocking
- **Event-driven**: Uses efficient system calls (kqueue/epoll) instead of threads
- **Zero-copy parsing**: Request parsing works directly on socket buffers
- **Connection pooling**: Keep-alive connections reuse TCP connections
- **Efficient routing**: Route matching uses hash maps for O(1) lookup

## Limitations

- **HTTP/1.1 only**: HTTP/2 and HTTP/3 are not supported
- **Single-threaded**: Event loop runs in a single thread (though very efficient)
- **No HTTPS**: TLS/SSL support not included (use reverse proxy for HTTPS)

## License

[Add your license here]

## Contributing

[Add contribution guidelines here]
