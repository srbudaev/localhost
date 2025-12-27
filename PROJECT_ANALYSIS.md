# Localhost HTTP Server - Project Analysis

## Requirements Analysis

### ✅ Implemented Features

#### Core Server Requirements
- ✅ Written in Rust
- ✅ Uses `libc` crate for system calls (kqueue/epoll)
- ✅ Uses `unsafe` only when necessary
- ✅ No forbidden crates (tokio, nix) - verified
- ✅ HTTP/1.1 protocol compatibility
- ✅ Single process, single thread architecture
- ✅ Event-driven architecture (kqueue for macOS)
- ✅ Non-blocking I/O operations
- ✅ Multiple ports and server instances support
- ✅ Request timeout management
- ✅ HTTP methods: GET, POST, DELETE (and more)
- ✅ File upload handling
- ✅ Cookies and sessions management
- ✅ Error pages for: 400, 403, 404, 405, 413, 500
- ✅ Chunked and unchunked request handling
- ✅ Proper HTTP status codes

#### CGI Implementation
- ✅ CGI execution based on file extension
- ✅ Process forking for CGI scripts
- ✅ Script file passed as first argument to interpreter
- ✅ Request body sent to CGI stdin (EOF handling)
- ✅ Working directory set to script's directory
- ✅ Environment variables setup

#### Configuration File
- ✅ TOML configuration format
- ✅ Host (server_address) and multiple ports per server
- ✅ Default server selection (first server for host:port)
- ✅ Custom error page paths
- ✅ Client body size limit for uploads
- ✅ Routes configuration with:
  - ✅ HTTP methods list
  - ✅ HTTP redirections
  - ✅ Directory/file mapping
  - ✅ Default file for directories
  - ✅ CGI extension mapping
  - ✅ Directory listing toggle
- ✅ No comment support (as required)

### ❌ Missing or Incomplete Features

#### Critical Issues

1. **Linux Support (epoll)**
   - ❌ Currently only supports macOS (kqueue)
   - ⚠️ Need to add epoll support for Linux
   - **Status**: Only macOS implementation exists
   - **Priority**: HIGH

2. **PATH_INFO Environment Variable**
   - ❌ Currently always empty
   - ⚠️ Need to calculate PATH_INFO from URL path
   - **Location**: `src/application/cgi/cgi_env.rs:35`
   - **Priority**: HIGH
   - **Requirement**: "The CGI will check PATH_INFO environment variable to define the full path"

3. **REMOTE_ADDR for CGI**
   - ❌ Currently hardcoded to "127.0.0.1"
   - ⚠️ Need to pass actual client address from Connection
   - **Location**: `src/application/cgi/cgi_env.rs:88`
   - **Priority**: HIGH

4. **DOCUMENT_ROOT for CGI**
   - ❌ Currently empty
   - ⚠️ Need to set server root directory
   - **Location**: `src/application/cgi/cgi_env.rs:97`
   - **Priority**: MEDIUM

5. **CGI Process Blocking**
   - ❌ CGI execution uses blocking `process.wait()`
   - ⚠️ This blocks the entire server during CGI execution
   - **Location**: `src/application/cgi/cgi_executor.rs:70`
   - **Priority**: HIGH
   - **Note**: Assignment says "You are allowed to fork a new process to run the CGI" but doesn't specify non-blocking requirement. However, blocking CGI execution violates the "single thread" and "non-blocking I/O" requirements.

6. **CGI Timeout**
   - ❌ No timeout implemented for CGI processes
   - ⚠️ `timeout_secs` field exists but not used
   - **Location**: `src/application/cgi/cgi_executor.rs:13`
   - **Priority**: MEDIUM

7. **epoll/kqueue Call Frequency**
   - ⚠️ Need to verify: "You should call epoll function (or equivalent) only once for each client/server communication"
   - **Current**: `event_loop.wait()` is called once per iteration in main loop
   - **Status**: Appears correct, but needs verification

8. **Error Handling - Server Stability**
   - ⚠️ Need comprehensive review: "It never crashes"
   - **Status**: Most errors are handled, but need thorough testing
   - **Priority**: HIGH

#### Additional Improvements Needed

1. **PATH_TRANSLATED Calculation**
   - Currently empty
   - Should map PATH_INFO to filesystem path

2. **Request Body Size Validation**
   - Configuration has `client_max_body_size` but need to verify it's enforced during parsing

3. **Memory Leak Testing**
   - Assignment requires: "Test possible memory leaks before to submit the project"
   - Need to add memory leak detection tools/tests

4. **Stress Testing**
   - Assignment requires: "Do stress tests with siege -b [IP]:[PORT]"
   - Need to verify 99.5% availability requirement

5. **Comprehensive Testing**
   - Assignment requires: "Create tests for as many cases as you can"
   - Current test coverage is minimal
   - Need tests for:
     - Redirections
     - Bad configuration files
     - Static and dynamic pages
     - Default error pages
     - Edge cases

### 📋 Implementation Details to Fix

#### 1. Linux/epoll Support
**File**: `src/core/event/poller.rs`
- Add `#[cfg(target_os = "linux")]` for epoll
- Implement equivalent functionality using `epoll_create`, `epoll_ctl`, `epoll_wait`
- Keep kqueue for macOS

#### 2. PATH_INFO Implementation
**File**: `src/application/cgi/cgi_env.rs`
- Calculate PATH_INFO from REQUEST_URI and SCRIPT_NAME
- Example: If REQUEST_URI = "/cgi-bin/script.py/path/to/resource" and SCRIPT_NAME = "/cgi-bin/script.py", then PATH_INFO = "/path/to/resource"

#### 3. REMOTE_ADDR Fix
**Files**: 
- `src/application/cgi/cgi_env.rs`
- `src/application/server/server_manager.rs`
- `src/core/net/connection.rs`
- Need to pass client address through the request processing chain

#### 4. DOCUMENT_ROOT Fix
**Files**:
- `src/application/cgi/cgi_env.rs`
- `src/application/server/server_manager.rs`
- Pass server root path to CGI environment builder

#### 5. CGI Non-blocking Execution
**File**: `src/application/cgi/cgi_executor.rs`
- Options:
  a) Use non-blocking process waiting (if possible)
  b) Accept blocking but document it
  c) Use separate thread pool (but assignment says single thread)
- **Note**: This is a design challenge - need to clarify requirements

#### 6. CGI Timeout Implementation
**File**: `src/application/cgi/cgi_executor.rs`
- Implement timeout using `std::time::Instant`
- Kill process if timeout exceeded

---

## Audit Questions Analysis

### Functional Questions

#### 1. How does an HTTP server work?
**Status**: ✅ **PASS** - Can be explained
- Event-driven architecture using kqueue/epoll
- Single-threaded, non-blocking I/O
- Request parsing → Route matching → Handler execution → Response generation
- **Location**: `src/application/server/server_manager.rs:164-197`

#### 2. Which function was used for I/O Multiplexing and how does it work?
**Status**: ✅ **PASS** - Implemented
- **macOS**: `kqueue()` / `kevent()` - kernel event notification system
- **Linux**: ❌ **NOT IMPLEMENTED** - Need epoll support
- **How it works**: 
  - Creates a kernel queue (`kqueue()`)
  - Registers file descriptors for read/write events (`kevent()` with EV_ADD)
  - Waits for events (`kevent()` with timeout)
  - Processes events and performs I/O operations
- **Location**: `src/core/event/poller.rs`

#### 3. Is the server using only one select (or equivalent) to read client requests and write answers?
**Status**: ✅ **PASS** - Verified
- **Single `kevent()` call** per event loop iteration
- **Location**: `src/application/server/server_manager.rs:167` - `self.event_loop.wait(100)?`
- **Implementation**: `src/core/event/event_loop.rs:25-27` - calls `poller.wait()` once
- All events (read/write) are collected in one call, then processed

#### 4. Why is it important to use only one select and how was it achieved?
**Status**: ✅ **PASS** - Can be explained
- **Why important**: 
  - Efficiency: Single system call instead of multiple
  - Scalability: Can handle many connections efficiently
  - Prevents race conditions and ensures consistent event handling
- **How achieved**:
  - Single `event_loop.wait()` call collects all ready events
  - Events are then processed in batches
  - **Location**: `src/application/server/server_manager.rs:164-196`

#### 5. Read the code from select to read/write - is there only one read or write per client per select?
**Status**: ✅ **PASS** - Verified
- **Read**: One `read_non_blocking()` call per event per client
- **Write**: One `write_non_blocking()` call per event per client
- **Location**: 
  - Read: `src/application/server/server_manager.rs:283`
  - Write: `src/application/server/server_manager.rs:471`
- **Note**: Multiple reads/writes may occur across multiple event loop iterations (for large requests/responses), but only one per iteration per client

#### 6. Are the return values for I/O functions checked properly?
**Status**: ⚠️ **PARTIAL** - Needs review
- **Read**: ✅ Checked - `read_non_blocking()` returns `Result<usize>`, errors propagated
- **Write**: ✅ Checked - `write_non_blocking()` returns `Result<usize>`, errors propagated
- **Location**: `src/core/net/io.rs:5-19`
- **Issue**: Errors are returned but need to verify all error cases are handled (connection closed, network errors, etc.)
- **Action needed**: Review error handling in `handle_read()` and `handle_write()`

#### 7. If an error is returned by I/O functions on a socket, is the client removed?
**Status**: ⚠️ **PARTIAL** - Needs verification
- **Current behavior**: 
  - I/O errors return `Result` which propagates up
  - Connection closed (`n == 0`) sets state to `Closed` - ✅
  - **Location**: `src/application/server/server_manager.rs:286-289`
- **Missing**: Need to verify that I/O errors (not just EOF) properly close and remove connections
- **Action needed**: Add explicit error handling to close connection on I/O errors

#### 8. Is writing and reading ALWAYS done through a select (or equivalent)?
**Status**: ✅ **PASS** - Verified
- **All reads**: Only after `kevent()` reports read-ready event
- **All writes**: Only after `kevent()` reports write-ready event
- **Registration**: 
  - Read events registered: `src/application/server/server_manager.rs:224`
  - Write events registered: `src/application/server/server_manager.rs:412`
- **No direct I/O**: All I/O operations are gated by event loop
- **Exception**: ❌ **CGI execution** uses blocking `process.wait()` - violates this principle

---

### Configuration File Questions

#### 1. Setup a single server with a single port
**Status**: ✅ **PASS** - Supported
- **Location**: `config.example.toml` shows example
- **Test**: Should work with basic configuration

#### 2. Setup multiple servers with different ports
**Status**: ✅ **PASS** - Supported
- **Implementation**: `src/application/config/models.rs:40` - `ports: Vec<u16>`
- **Test**: Need to verify with actual test

#### 3. Setup multiple servers with different hostnames
**Status**: ✅ **PASS** - Implemented
- **Implementation**: `src/application/server/server_manager.rs:432-450` - `find_server_for_request()`
- **Matching**: Uses `Host` header to match `server_name`
- **Default fallback**: First server for port if no match
- **Test**: Need to verify with `curl --resolve test.com:80:127.0.0.1 http://test.com/`

#### 4. Setup custom error pages
**Status**: ✅ **PASS** - Implemented
- **Configuration**: `config.example.toml:71-77` shows error page config
- **Implementation**: `src/application/handler/error_page_handler.rs`
- **Supported codes**: 400, 403, 404, 405, 413, 500

#### 5. Limit the client body size
**Status**: ✅ **PASS** - Implemented
- **Configuration**: `config.example.toml:8` - `client_max_body_size`
- **Implementation**: `src/application/config/models.rs:14-15`
- **Enforcement**: ⚠️ Need to verify it's enforced during request parsing
- **Test**: Need to test with `curl -X POST --data "..."` with body larger than limit

#### 6. Setup routes and ensure they are taken into account
**Status**: ✅ **PASS** - Implemented
- **Configuration**: `config.example.toml:42-68` shows route examples
- **Implementation**: `src/application/handler/router.rs`
- **Matching**: Longest prefix match algorithm
- **Test**: Need to verify route matching works correctly

#### 7. Setup a default file in case the path is a directory
**Status**: ✅ **PASS** - Implemented
- **Configuration**: `config.example.toml:45` - `default_file = "index.html"`
- **Implementation**: `src/application/handler/static_file_handler.rs:64-68`
- **Test**: Need to verify default file is served

#### 8. Setup a list of accepted methods for a route
**Status**: ✅ **PASS** - Implemented
- **Configuration**: `config.example.toml:43` - `methods = ["GET", "POST"]`
- **Implementation**: `src/application/handler/router.rs:59-66` - `is_method_allowed()`
- **Validation**: `src/application/handler/router.rs:69-81` - `validate_request()`
- **Test**: Need to test DELETE with and without permission

---

### Methods and Cookies Questions

#### 1. Are GET requests working properly?
**Status**: ✅ **PASS** - Implemented
- **Handler**: `src/application/handler/static_file_handler.rs`
- **Status codes**: Should return 200, 404, 403, etc.
- **Test**: Need to verify with actual requests

#### 2. Are POST requests working properly?
**Status**: ✅ **PASS** - Implemented
- **Handler**: `src/application/handler/upload_handler.rs`
- **Body parsing**: `src/http/parser.rs` handles POST body
- **Test**: Need to verify with actual POST requests

#### 3. Are DELETE requests working properly?
**Status**: ⚠️ **UNKNOWN** - Need to verify
- **Method**: Defined in `src/http/method.rs:10`
- **Handler**: Need to check if DELETE is handled (may need implementation)
- **Test**: Need to test DELETE requests

#### 4. Test a WRONG request - is the server still working properly?
**Status**: ⚠️ **NEEDS TESTING**
- **Error handling**: Errors should be caught and return appropriate status codes
- **Server stability**: Should not crash
- **Test**: Need to test malformed requests, invalid methods, etc.

#### 5. Upload some files and get them back to test they were not corrupted
**Status**: ✅ **PASS** - Implemented
- **Upload**: `src/application/handler/upload_handler.rs`
- **Storage**: Files saved to `upload_dir`
- **Test**: Need to verify file integrity after upload/download

#### 6. A working session and cookies system is present?
**Status**: ✅ **PASS** - Implemented
- **Sessions**: `src/application/handler/session_manager.rs`
- **Cookies**: `src/http/cookie.rs`
- **Integration**: `src/application/server/server_manager.rs:372-388`
- **Test**: Need to verify session persistence and cookie handling

---

### Interaction with Browser Questions

#### 1. Is the browser connecting with the server with no issues?
**Status**: ⚠️ **NEEDS TESTING**
- **Implementation**: Should work, but needs browser testing
- **Test**: Open browser and connect to server

#### 2. Are the request and response headers correct?
**Status**: ✅ **PASS** - Implemented
- **Request parsing**: `src/http/parser.rs`
- **Response generation**: `src/http/serializer.rs`
- **Headers**: `src/http/headers.rs`
- **Test**: Need to verify with browser dev tools

#### 3. Try a wrong URL - is it handled properly?
**Status**: ✅ **PASS** - Implemented
- **404 handling**: `src/application/handler/error_page_handler.rs`
- **Test**: Need to verify 404 responses

#### 4. Try to list a directory - is it handled properly?
**Status**: ✅ **PASS** - Implemented
- **Handler**: `src/application/handler/directory_listing_handler.rs`
- **Configuration**: `directory_listing = true` in route config
- **Test**: Need to verify directory listing works

#### 5. Try a redirected URL - is it handled properly?
**Status**: ✅ **PASS** - Implemented
- **Handler**: `src/application/handler/redirection_handler.rs`
- **Configuration**: `redirect = "/new"` in route config
- **Status**: Returns 302 Found
- **Test**: Need to verify redirects work

#### 6. Check the implemented CGI - does it work with chunked and unchunked data?
**Status**: ⚠️ **PARTIAL**
- **Chunked requests**: ✅ Parsed in `src/http/parser.rs:192-233`
- **Unchunked requests**: ✅ Parsed in `src/http/parser.rs:177-189`
- **CGI execution**: ❌ **BLOCKING** - `process.wait()` blocks server
- **Test**: Need to verify CGI works with both chunked and unchunked requests

---

### Port Issues Questions

#### 1. Configure multiple ports and websites - ensure it works
**Status**: ✅ **PASS** - Implemented
- **Implementation**: `src/application/server/server_instance.rs:57-63`
- **Multiple ports**: Each server can have multiple ports
- **Test**: Need to verify multiple ports work simultaneously

#### 2. Configure the same port multiple times - server should find the error
**Status**: ✅ **PASS** - Implemented
- **Validation**: `src/application/config/validator.rs:51-75` - `validate_port_conflicts()`
- **Error detection**: Checks for same `(address, port)` combination
- **Error message**: Returns clear error with server names
- **Test**: Need to verify error is caught during config loading

#### 3. Configure multiple servers with common ports but different configs - should work if one config fails
**Status**: ⚠️ **NEEDS REVIEW**
- **Current behavior**: Config validation happens before server creation
- **If one server fails**: Entire config loading fails
- **Requirement**: Should allow partial success
- **Action needed**: Review error handling in `ServerManager::new()`
- **Location**: `src/application/server/server_manager.rs:55-111`

---

### Siege & Stress Test Questions

#### 1. Use siege with GET on empty page - availability should be at least 99.5%
**Status**: ❌ **NOT TESTED**
- **Command**: `siege -b [IP]:[PORT]`
- **Requirement**: 99.5% availability
- **Test**: Need to run siege and measure availability
- **Potential issues**: 
  - CGI blocking may cause issues under load
  - Need to verify no memory leaks

#### 2. Check if there is no memory leak
**Status**: ❌ **NOT TESTED**
- **Tools**: `top`, `valgrind`, `heaptrack`, etc.
- **Test**: Need to run memory leak detection tools
- **Potential issues**: 
  - Connection cleanup
  - Session storage
  - Buffer management

#### 3. Check if there is no hanging connection
**Status**: ⚠️ **PARTIAL**
- **Timeout**: Implemented in `src/core/net/connection.rs:65-67`
- **Cleanup**: `src/application/server/server_manager.rs:501-516`
- **Test**: Need to verify connections are cleaned up properly
- **Potential issue**: CGI blocking may cause connection hangs

---

### General Bonus Questions

#### 1. There's more than one CGI system (Python, C++, Perl)
**Status**: ❌ **NOT IMPLEMENTED**
- **Current**: Only Python example (`cgi-bin/test.py`)
- **Configuration**: Supports multiple interpreters via `cgi_handlers`
- **Test**: Need to add examples for other languages

#### 2. There is a second implementation in a different language
**Status**: ❌ **NOT IMPLEMENTED**
- **Current**: Only Rust implementation
- **Requirement**: Bonus feature

---

## Audit Questions Summary

### ✅ Passing Questions (Ready for Audit)
1. HTTP server architecture explanation
2. I/O multiplexing function (kqueue) - but need Linux support
3. Single select/epoll call
4. Importance of single select
5. One read/write per client per select
6. I/O always through select (except CGI)
7. Configuration: single/multiple servers, ports, hostnames
8. Configuration: error pages, body limit, routes, default files, methods
9. GET/POST requests
10. File uploads
11. Sessions and cookies
12. Browser interaction (headers, 404, directory listing, redirects)
13. Port conflict detection

### ⚠️ Partial/Needs Review Questions
1. I/O return value checking (implemented but needs thorough review)
2. Client removal on errors (partial - needs explicit error handling)
3. DELETE method handling (need to verify)
4. Wrong request handling (needs testing)
5. CGI chunked/unchunked (parsing works, but execution blocks)
6. Multiple servers with common ports (needs partial failure handling)
7. Hanging connections (timeout implemented, needs testing)

### ❌ Failing/Not Tested Questions
1. Linux/epoll support (only macOS kqueue)
2. CGI non-blocking execution (currently blocking)
3. Siege stress test (not tested)
4. Memory leak testing (not tested)
5. Multiple CGI systems (only Python)
6. Second language implementation (bonus)

---

## Summary

### Critical Path Items (Must Fix Before Audit)

#### From Assignment Requirements:
1. **Linux/epoll support** - Currently only macOS kqueue
2. **PATH_INFO implementation** - Required for CGI
3. **REMOTE_ADDR fix** - Currently hardcoded
4. **CGI non-blocking execution** - Currently blocks entire server
5. **Server stability verification** - "It never crashes" requirement

#### From Audit Questions:
1. **I/O error handling** - Ensure clients are removed on errors
2. **DELETE method** - Verify implementation
3. **Wrong request handling** - Test malformed requests
4. **Multiple servers with common ports** - Handle partial failures
5. **Comprehensive testing** - All audit scenarios need testing

### Important Items (Should Fix)

#### From Assignment Requirements:
1. **DOCUMENT_ROOT** - Set for CGI environment
2. **CGI timeout** - Implement timeout mechanism
3. **Request body size enforcement** - Verify during parsing
4. **PATH_TRANSLATED** - Calculate from PATH_INFO

#### From Audit Questions:
1. **Memory leak testing** - Use tools like valgrind/heaptrack
2. **Stress testing with siege** - Verify 99.5% availability
3. **Hanging connection testing** - Verify cleanup works
4. **Browser compatibility testing** - Test with actual browsers
5. **File upload integrity** - Verify files aren't corrupted

### Nice to Have / Bonus
1. **Multiple CGI systems** - Add examples for C++, Perl, etc.
2. **Second language implementation** - Bonus feature
3. **Enhanced logging** - Better debugging capabilities
4. **More comprehensive tests** - Edge cases, error scenarios

---

## Files Requiring Changes

1. `src/core/event/poller.rs` - Add epoll support
2. `src/application/cgi/cgi_env.rs` - Fix PATH_INFO, REMOTE_ADDR, DOCUMENT_ROOT
3. `src/application/cgi/cgi_executor.rs` - Add timeout, consider non-blocking
4. `src/application/server/server_manager.rs` - Pass client address to CGI
5. `src/core/net/connection.rs` - Store client address
6. `src/http/request.rs` - Possibly add client address field
7. Test files - Add comprehensive test suite

---

---

## Testing Checklist

### Before Audit - Must Test:
- [ ] Single server, single port
- [ ] Multiple servers, different ports
- [ ] Multiple servers, different hostnames (`curl --resolve`)
- [ ] Custom error pages (400, 403, 404, 405, 413, 500)
- [ ] Client body size limit enforcement
- [ ] Route matching and method restrictions
- [ ] Default file serving
- [ ] GET requests with various status codes
- [ ] POST requests and file uploads
- [ ] DELETE requests
- [ ] Malformed/wrong requests (server stability)
- [ ] File upload and download integrity
- [ ] Session and cookie persistence
- [ ] Browser compatibility (Chrome, Firefox, Safari)
- [ ] Request/response headers correctness
- [ ] Directory listing
- [ ] HTTP redirects (302)
- [ ] CGI with chunked data
- [ ] CGI with unchunked data
- [ ] Port conflict detection
- [ ] Multiple servers with common ports (partial failure)
- [ ] Siege stress test (99.5% availability)
- [ ] Memory leak detection
- [ ] Hanging connection cleanup

### Bonus Tests:
- [ ] Multiple CGI languages (Python, C++, Perl)
- [ ] Second implementation in different language

---

*Last Updated: Analysis completed with audit questions*

