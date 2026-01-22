# Audit Answers - Localhost HTTP Server

## Functional Questions

### 1. How does an HTTP server work?

**Answer:**
An HTTP server follows a client-server model:
1. **Listen**: Server binds to a port and listens for incoming TCP connections
2. **Accept**: When a client connects, server accepts the connection
3. **Read**: Server reads HTTP request from the client socket
4. **Parse**: Server parses the HTTP request (method, path, headers, body)
5. **Route**: Server matches the request path to a configured route
6. **Process**: Server processes the request using appropriate handler (static file, CGI, upload, etc.)
7. **Respond**: Server generates HTTP response and writes it back to the client
8. **Close/Keep-alive**: Server either closes connection or keeps it alive for next request

**Implementation location:**
- Main loop: `src/application/server/server_manager.rs:285-338` (`run()` method)
- Request processing: `src/application/server/server_manager.rs:558-750` (`process_request()` method)

### 2. Which function was used for I/O Multiplexing and how does it work?

**Answer:**
The server uses **kqueue** (on macOS/BSD) or **epoll** (on Linux) for I/O multiplexing.

**How kqueue works:**
- `kqueue()` creates a kernel event queue
- `kevent()` registers file descriptors for monitoring (read/write events)
- `kevent()` with NULL changelist waits for events (like `select()` but more efficient)
- Returns when events occur or timeout expires
- Allows monitoring many file descriptors efficiently in a single system call

**Implementation:**
- Poller: `src/core/event/poller.rs`
- Event registration: `src/core/event/poller.rs:28-71` (`register_read()`, `register_write()`)
- Event waiting: `src/core/event/poller.rs:100-137` (`wait()` method)
- Main loop: `src/application/server/server_manager.rs:285-338` (calls `event_loop.wait(100)`)

**Why kqueue/epoll over select:**
- More efficient for large numbers of file descriptors (O(1) vs O(n))
- Supports edge-triggered events
- Better performance with many connections

### 3. Is the server using only one select (or equivalent) to read the client requests and write answers?

**Answer:**
**Yes!** The server uses a single `kevent()` call (equivalent to `select()`) in the main event loop.

**Evidence:**
- Single wait call: `src/application/server/server_manager.rs:288` - `let events = self.event_loop.wait(100)?;`
- This single call monitors ALL file descriptors:
  - Listener sockets (for accepting new connections)
  - Client sockets (for reading requests)
  - Client sockets (for writing responses)
- After the wait, events are processed in batches

### 4. Why is it important to use only one select and how was it achieved?

**Answer:**
**Why important:**
- **Efficiency**: Single system call instead of multiple reduces overhead
- **Scalability**: Can handle thousands of connections efficiently
- **Non-blocking**: Server doesn't block waiting on individual connections
- **Event-driven**: All I/O operations are coordinated through one event loop

**How achieved:**
1. **Single event loop**: One `EventLoop` instance manages all file descriptors
2. **Event registration**: All sockets (listeners and clients) registered with same `kqueue`
3. **Single wait**: One `kevent()` call waits for events on all registered FDs
4. **Event processing**: After wait returns, process all ready events in batches
5. **State management**: Connection state (`Reading`, `Writing`, `Closed`) determines what to do

**Implementation:**
- Event loop: `src/core/event/event_loop.rs`
- Single wait: `src/application/server/server_manager.rs:288`
- Event processing: `src/application/server/server_manager.rs:290-327`

### 5. Read the code that goes from the select (or equivalent) to the read and write of a client, is there only one read or write per client per select (or equivalent)?

**Answer:**
**Yes, typically one read or write per client per event.**

**Read flow:**
1. `kevent()` returns events (`server_manager.rs:288`)
2. For each client event, check connection state (`server_manager.rs:456-494`)
3. If state is `Reading`, call `handle_read()` (`server_manager.rs:497-555`)
4. `handle_read()` performs **one** `read_non_blocking()` call (`server_manager.rs:502`)
5. Reads up to `DEFAULT_BUFFER_SIZE` (8KB) bytes
6. If more data needed, connection stays in `Reading` state for next event

**Write flow:**
1. After processing request, response written to connection's write buffer
2. Connection state changed to `Writing`
3. On next `kevent()` return, if state is `Writing`, call `handle_write()` (`server_manager.rs:951-1020`)
4. `handle_write()` performs **one** `write_non_blocking()` call (`server_manager.rs:981`)
5. If buffer not empty, stays in `Writing` state for next event

**Key points:**
- One I/O operation per event per client
- If data remains, connection stays in same state for next event
- Non-blocking I/O ensures we don't wait

**Code references:**
- Read: `src/application/server/server_manager.rs:497-555`
- Write: `src/application/server/server_manager.rs:951-1020`
- I/O functions: `src/core/net/io.rs`

### 6. Are the return values for I/O functions checked properly?

**Answer:**
**Yes, all I/O return values are checked.**

**Read operations:**
```rust
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
```
Location: `src/application/server/server_manager.rs:500-509`

**Write operations:**
```rust
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
```
Location: `src/application/server/server_manager.rs:978-989`

**Also checked:**
- EOF (n == 0): `server_manager.rs:512-516`
- Accept errors: `server_manager.rs:376-383`
- Event registration errors: `server_manager.rs:361-370`

### 7. If an error is returned by the previous functions on a socket, is the client removed?

**Answer:**
**Yes, clients are removed on errors.**

**Error handling:**
1. **I/O errors**: `close_connection_on_error()` called (`server_manager.rs:507, 986`)
2. **EOF (connection closed)**: `close_connection_on_error()` called (`server_manager.rs:514`)
3. **Parse errors**: `close_connection_on_error()` called (`server_manager.rs:525, 549`)
4. **Registration errors**: Connection removed immediately (`server_manager.rs:363-364`)

**Cleanup process:**
```rust
fn close_connection_on_error(&mut self, fd: i32) -> Result<()> {
    self.set_connection_state_and_close(fd, ConnectionState::Closed)
}
```
Location: `src/application/server/server_manager.rs:1070-1072`

**What gets cleaned up:**
- Connection removed from `connections` HashMap
- Parser removed from `parsers` HashMap
- Events unregistered from event manager
- Socket closed

**Code references:**
- `close_connection_on_error()`: `server_manager.rs:1070-1072`
- `close_connection()`: `server_manager.rs:1083-1110`
- Error paths: `server_manager.rs:507, 514, 525, 549, 986`

### 8. Is writing and reading ALWAYS done through a select (or equivalent)?

**Answer:**
**Yes, absolutely!** All reading and writing goes through the event loop.

**Reading:**
1. Client socket registered for read events on accept (`server_manager.rs:361`)
2. When data arrives, `kevent()` returns with read event
3. `handle_read()` called from event handler (`server_manager.rs:468`)
4. Read performed only when event indicates data available

**Writing:**
1. After generating response, data added to write buffer
2. Connection state set to `Writing`
3. Write event registered (`server_manager.rs:1020-1025`)
4. When socket ready for writing, `kevent()` returns with write event
5. `handle_write()` called from event handler (`server_manager.rs:478`)
6. Write performed only when event indicates socket ready

**No direct I/O:**
- No blocking `read()` or `write()` calls
- All I/O is non-blocking and event-driven
- Everything goes through `kevent()` → event handler → I/O operation

**Code flow:**
1. Event loop: `server_manager.rs:285-338`
2. Event handling: `server_manager.rs:456-494`
3. Read: `server_manager.rs:497-555`
4. Write: `server_manager.rs:951-1020`

## Configuration File Testing

### Setup a single server with a single port

**Config:**
```toml
[[servers]]
server_address = "127.0.0.1"
ports = [8080]
server_name = "localhost"
root = "./root"
```

**Test:**
```bash
curl http://localhost:8080/
```

### Setup multiple servers with different ports

**Config:**
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

**Test:**
```bash
curl http://localhost:8080/
curl http://localhost:8081/
```

### Setup multiple servers with different hostnames

**Config:**
```toml
[[servers]]
server_address = "127.0.0.1"
ports = [8080]
server_name = "localhost"
root = "./root"

[[servers]]
server_address = "127.0.0.1"
ports = [8080]
server_name = "test.com"
root = "./root"
```

**Test:**
```bash
curl --resolve test.com:8080:127.0.0.1 http://test.com:8080/
curl --resolve localhost:8080:127.0.0.1 http://localhost:8080/
```

**Implementation:**
- Virtual host matching: `server_manager.rs:574` - `find_server_for_request()`
- Host header parsing: `http/request.rs` - `host()` method
- Server lookup: `server_manager.rs:52` - `server_lookup: HashMap<(u16, String), usize>`

### Setup custom error pages

**Config:**
```toml
[servers.errors]
"400" = { filename = "errors/400.html" }
"403" = { filename = "errors/403.html" }
"404" = { filename = "errors/404.html" }
"405" = { filename = "errors/405.html" }
"413" = { filename = "errors/413.html" }
"500" = { filename = "errors/500.html" }
```

**Test:**
```bash
curl http://localhost:8080/nonexistent  # 404
curl -X POST http://localhost:8080/static  # 405 (if POST not allowed)
```

**Implementation:**
- Error page handler: `src/application/handler/error_page_handler.rs`
- Error response generation: `server_manager.rs:805-817`

### Limit the client body

**Config:**
```toml
client_max_body_size = 10485760  # 10MB
```

**Test:**
```bash
# Should succeed (under limit)
dd if=/dev/zero bs=1024 count=1024 | curl -X POST http://localhost:8080/upload --data-binary @-

# Should fail with 413 (over limit)
dd if=/dev/zero bs=1024 count=11000 | curl -X POST http://localhost:8080/upload --data-binary @-
```

**Implementation:**
- Body size check: `http/parser.rs` - `add_data()` method
- Error handling: `server_manager.rs:521-523, 545-546`

### Setup routes and ensure they are taken into account

**Config:**
```toml
[servers.routes."/"]
methods = ["GET", "POST"]
directory = "."

[servers.routes."/api"]
methods = ["GET"]
directory = "./api"
```

**Test:**
```bash
curl http://localhost:8080/        # Matches "/"
curl http://localhost:8080/api     # Matches "/api"
curl http://localhost:8080/other   # 404
```

**Implementation:**
- Route matching: `src/application/handler/router.rs` - `match_route()`
- Route processing: `server_manager.rs:609-750`

### Setup a default file in case the path is a directory

**Config:**
```toml
[servers.routes."/"]
methods = ["GET"]
directory = "."
default_file = "index.html"
```

**Test:**
```bash
curl http://localhost:8080/        # Serves index.html
curl http://localhost:8080/static/ # Serves static/index.html if exists
```

**Implementation:**
- Default file handling: `router.rs` - `resolve_file_path()`
- Static file handler: `src/application/handler/static_file_handler.rs`

### Setup a list of accepted methods for a route

**Config:**
```toml
[servers.routes."/uploads"]
methods = ["GET", "POST"]
upload_dir = "uploads"

[servers.routes."/delete"]
methods = ["DELETE"]
directory = "."
```

**Test:**
```bash
curl -X DELETE http://localhost:8080/delete/file.txt  # Should work
curl -X POST http://localhost:8080/delete/file.txt     # Should return 405
```

**Implementation:**
- Method validation: `router.rs` - `validate_request()`
- 405 response: `response.rs` - `method_not_allowed()`

## Methods and Cookies

### GET requests

**Test:**
```bash
curl -v http://localhost:8080/
curl -v http://localhost:8080/index.html
```

**Expected:** 200 OK with content

### POST requests

**Test:**
```bash
curl -X POST http://localhost:8080/upload -F "file=@test.txt"
curl -X POST http://localhost:8080/cgi-bin/echo.py -d "data=test"
```

**Expected:** 200 OK

### DELETE requests

**Test:**
```bash
# Create a test file first
echo "test" > root/test_delete.txt

# Delete it
curl -v -X DELETE http://localhost:8080/test_delete.txt

# Verify it's gone
curl http://localhost:8080/test_delete.txt  # Should return 404
```

**Expected:** 200 OK or 204 No Content

**Implementation:**
- Delete handler: `src/application/handler/delete_handler.rs`

### Wrong requests

**Test:**
```bash
# Invalid HTTP version
printf "GET / HTTP/2.0\r\n\r\n" | nc localhost 8080

# Invalid method
curl -X INVALID http://localhost:8080/

# Malformed request
printf "GET / \r\n\r\n" | nc localhost 8080
```

**Expected:** Server should handle gracefully, return 400 or close connection, but continue serving other clients

**Implementation:**
- Error handling: `server_manager.rs:542-551`
- Parse errors: `http/parser.rs`

### Upload files and get them back

**Test:**
```bash
# Upload
curl -X POST http://localhost:8080/upload -F "file=@test.jpg" -o upload_response.txt

# Get uploaded file back
curl http://localhost:8080/uploads/[filename_from_response]
```

**Expected:** File should be uploaded correctly and retrievable without corruption

**Implementation:**
- Upload handler: `src/application/handler/upload_handler.rs`
- Multipart parsing: `upload_handler.rs:26-259`

### Session and cookies system

**Test:**
```bash
# Request with cookie
curl -v -H "Cookie: session_id=abc123" http://localhost:8080/

# Check if server sets cookies
curl -v http://localhost:8080/cgi-bin/echo.py 2>&1 | grep -i set-cookie
```

**Expected:** Server should handle cookies and sessions

**Implementation:**
- Cookie parsing: `src/http/cookie.rs`
- Session manager: `src/application/handler/session_manager.rs`

## Port Issues

### Configure multiple ports

**Config:**
```toml
[[servers]]
ports = [8080, 8081, 8082]
```

**Test:**
```bash
curl http://localhost:8080/
curl http://localhost:8081/
curl http://localhost:8082/
```

**Expected:** All ports should work

### Configure same port multiple times

**Config:**
```toml
[[servers]]
ports = [8080]

[[servers]]
ports = [8080]  # Duplicate!
```

**Expected:** Server should detect error and handle appropriately

**Implementation:**
- Port conflict detection: `server_manager.rs:73-212`
- Error handling: `server_manager.rs:207-212`

### Multiple servers with common ports but different configs

**Config:**
```toml
[[servers]]
ports = [8080]
server_name = "localhost"
root = "./root"

[[servers]]
ports = [8080]
server_name = "test.com"
root = "./root"

[[servers]]
ports = [8080]
server_name = "invalid"  # Invalid config
root = "/nonexistent"
```

**Expected:** Server should work for valid configs even if one is invalid

**Implementation:**
- Partial failure handling: `server_manager.rs:73-212`
- Error collection: `server_manager.rs:71, 207-212`
