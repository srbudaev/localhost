# Task Division Between Two Developers

This document divides all required improvements and fixes between two developers to minimize merge conflicts by working on different files.

---

## Developer 1: Core Infrastructure & CGI

### Critical Tasks

#### 1. Linux/epoll Support
**Files to modify:**
- `src/core/event/poller.rs` - Add epoll implementation for Linux
- `src/core/event/mod.rs` - Update exports if needed

**Tasks:**
- Add `#[cfg(target_os = "linux")]` conditional compilation
- Implement `epoll_create()`, `epoll_ctl()`, `epoll_wait()` equivalents
- Keep existing kqueue implementation for macOS
- Ensure same API interface for both platforms
- Test on Linux system

**Estimated time:** 4-6 hours

---

#### 2. PATH_INFO Implementation for CGI
**Files to modify:**
- `src/application/cgi/cgi_env.rs` - Calculate PATH_INFO from URL
- `src/application/handler/router.rs` - May need helper methods

**Tasks:**
- Parse REQUEST_URI to extract PATH_INFO
- Calculate PATH_INFO = REQUEST_URI - SCRIPT_NAME
- Handle edge cases (root paths, query strings)
- Update PATH_TRANSLATED calculation based on PATH_INFO
- Add tests for PATH_INFO calculation

**Estimated time:** 3-4 hours

---

#### 3. REMOTE_ADDR and DOCUMENT_ROOT for CGI
**Files to modify:**
- `src/application/cgi/cgi_env.rs` - Accept and use client address and document root
- `src/core/net/connection.rs` - Ensure client address is accessible
- `src/application/server/server_manager.rs` - Pass client address and root to CGI

**Tasks:**
- Add `client_addr` parameter to `CgiEnvironment::build()`
- Add `document_root` parameter to `CgiEnvironment::build()`
- Extract client address from Connection in server_manager
- Pass server root path to CGI environment builder
- Update all CGI handler calls to pass new parameters
- Test REMOTE_ADDR with actual client connections

**Estimated time:** 2-3 hours

---

#### 4. CGI Timeout Implementation
**Files to modify:**
- `src/application/cgi/cgi_executor.rs` - Implement timeout mechanism
- `src/application/cgi/cgi_process.rs` - May need timeout support

**Tasks:**
- Use `std::time::Instant` to track CGI execution time
- Check timeout before/after process.wait()
- Kill process if timeout exceeded
- Return appropriate error (504 Gateway Timeout)
- Add timeout tests

**Estimated time:** 2-3 hours

---

#### 5. CGI Non-blocking Execution (Research & Implementation)
**Files to modify:**
- `src/application/cgi/cgi_executor.rs` - Make execution non-blocking
- `src/application/cgi/cgi_process.rs` - May need async support
- `src/application/server/server_manager.rs` - Handle async CGI execution

**Tasks:**
- Research non-blocking process waiting in Rust
- Implement non-blocking CGI execution (or document why blocking is acceptable)
- Handle CGI process completion asynchronously
- Ensure server doesn't block during CGI execution
- Test with multiple concurrent CGI requests

**Estimated time:** 4-6 hours (research + implementation)

---

### Testing Tasks

#### 6. CGI Testing Suite
**Files to create/modify:**
- `tests/cgi_tests.rs` - Comprehensive CGI tests
- `cgi-bin/test_chunked.py` - Test chunked requests
- `cgi-bin/test_unchunked.py` - Test unchunked requests

**Tasks:**
- Test CGI with chunked request body
- Test CGI with unchunked request body
- Test PATH_INFO extraction
- Test REMOTE_ADDR passing
- Test DOCUMENT_ROOT setting
- Test CGI timeout
- Test multiple concurrent CGI requests

**Estimated time:** 3-4 hours

---

### Developer 1 File Ownership

**Files that Developer 1 owns (Developer 2 should NOT modify):**
- `src/core/event/poller.rs`
- `src/application/cgi/cgi_env.rs`
- `src/application/cgi/cgi_executor.rs`
- `src/application/cgi/cgi_process.rs`
- `src/application/cgi/cgi_io.rs`
- `tests/cgi_tests.rs`
- `cgi-bin/*.py` (new test files)

**Files that Developer 1 will modify (coordinate with Developer 2):**
- `src/core/net/connection.rs` - Need to expose client address accessor
- `src/application/server/server_manager.rs` - Pass client_addr and document_root to CGI (Developer 1 does this first, then Developer 2 adds error handling)

---

### Developer 1 Success Criteria

- [ ] Linux/epoll support implemented and tested
- [ ] PATH_INFO correctly calculated and tested
- [ ] REMOTE_ADDR and DOCUMENT_ROOT set correctly
- [ ] CGI timeout implemented
- [ ] CGI execution doesn't block server (or documented why)
- [ ] CGI tests pass (chunked, unchunked, PATH_INFO, etc.)

**Total Estimated Time for Developer 1: 18-26 hours**

---

## Developer 2: Server Stability & Testing

### Critical Tasks

#### 1. I/O Error Handling and Client Cleanup
**Files to modify:**
- `src/application/server/server_manager.rs` - Improve error handling in handle_read/handle_write
- `src/core/net/io.rs` - Review error handling
- `src/core/net/connection.rs` - May need error state tracking

**Tasks:**
- Ensure all I/O errors properly close connections
- Remove clients on socket errors (not just EOF)
- Add explicit error handling in `handle_read()`
- Add explicit error handling in `handle_write()`
- Test with network errors (connection reset, timeout, etc.)
- Ensure no hanging connections on errors

**Estimated time:** 3-4 hours

---

#### 2. Request Body Size Enforcement
**Files to modify:**
- `src/http/parser.rs` - Enforce body size limit during parsing
- `src/application/config/models.rs` - Ensure config is accessible
- `src/application/server/server_manager.rs` - Pass config to parser

**Tasks:**
- Check body size during parsing
- Return 413 Payload Too Large if exceeded
- Test with bodies larger than limit
- Test with bodies exactly at limit
- Ensure proper error response

**Estimated time:** 2-3 hours

---

#### 3. DELETE Method Implementation
**Files to modify:**
- `src/application/handler/delete_handler.rs` - Create new handler (if needed)
- `src/application/handler/mod.rs` - Export new handler
- `src/application/server/server_manager.rs` - Route DELETE requests

**Tasks:**
- Implement DELETE handler (or verify existing support)
- Handle file deletion safely
- Return appropriate status codes (200, 404, 403, 500)
- Test DELETE requests
- Test DELETE with method restrictions

**Estimated time:** 2-3 hours

---

#### 4. Multiple Servers with Common Ports - Partial Failure Handling
**Files to modify:**
- `src/application/server/server_manager.rs` - Handle partial server creation failures
- `src/application/config/validator.rs` - May need to adjust validation strategy

**Tasks:**
- Allow server creation to continue if one server fails
- Collect and report all errors, not just first one
- Ensure working servers start even if others fail
- Test with mix of valid and invalid server configs
- Document behavior

**Estimated time:** 3-4 hours

---

#### 5. Server Stability - Comprehensive Error Handling Review
**Files to review/modify:**
- `src/application/server/server_manager.rs` - All error paths
- `src/http/parser.rs` - Malformed request handling
- `src/application/handler/*.rs` - All handlers error handling
- `src/core/event/*.rs` - Event system error handling

**Tasks:**
- Review all `unwrap()` calls - replace with proper error handling
- Review all `panic!()` calls - ensure they're not reachable
- Add error handling for all edge cases
- Test with malformed requests
- Test with invalid HTTP methods
- Test with oversized headers
- Ensure server never crashes

**Estimated time:** 4-5 hours

---

### Testing Tasks

#### 6. Comprehensive Test Suite
**Files to create:**
- `tests/integration_tests.rs` - Integration tests
- `tests/config_tests.rs` - Configuration tests
- `tests/error_tests.rs` - Error handling tests
- `tests/stress_tests.rs` - Stress test helpers

**Tasks:**
- Test single server, single port
- Test multiple servers, different ports
- Test multiple servers, different hostnames
- Test custom error pages
- Test client body size limit
- Test route matching
- Test default files
- Test method restrictions
- Test GET/POST/DELETE requests
- Test malformed requests
- Test file upload/download integrity
- Test sessions and cookies
- Test directory listing
- Test redirects
- Test port conflicts

**Estimated time:** 6-8 hours

---

#### 7. Browser Compatibility Testing
**Files to create:**
- `tests/browser_tests.md` - Browser test results documentation
- Test HTML files for various scenarios

**Tasks:**
- Test with Chrome/Chromium
- Test with Firefox
- Test with Safari (if on macOS)
- Verify request/response headers
- Test wrong URLs
- Test directory listing in browser
- Test redirects in browser
- Document any browser-specific issues

**Estimated time:** 2-3 hours

---

#### 8. Stress Testing and Memory Leak Detection
**Files to create:**
- `tests/stress_test.sh` - Siege test script
- `tests/memory_test.sh` - Memory leak detection script
- `docs/stress_test_results.md` - Test results documentation

**Tasks:**
- Set up siege for stress testing
- Run `siege -b [IP]:[PORT]` and verify 99.5% availability
- Use `valgrind` or `heaptrack` for memory leak detection
- Test with various load levels
- Monitor connection cleanup
- Document results and fix any issues found

**Estimated time:** 4-5 hours

---

### Developer 2 File Ownership

**Files that Developer 2 owns (Developer 1 should NOT modify):**
- `src/http/parser.rs` (body size enforcement parts)
- `src/application/handler/delete_handler.rs` (new file)
- `src/application/config/validator.rs` (partial failure handling)
- `tests/integration_tests.rs`
- `tests/config_tests.rs`
- `tests/error_tests.rs`
- `tests/stress_tests.rs`
- `tests/browser_tests.md`
- `tests/stress_test.sh`
- `tests/memory_test.sh`

**Files that Developer 2 will modify (coordinate with Developer 1):**
- `src/application/server/server_manager.rs` - I/O error handling (Developer 2 adds this after Developer 1's CGI changes)
- `src/core/net/connection.rs` - Error state tracking (coordinate with Developer 1's address accessor)
- `src/application/handler/mod.rs` - May add DELETE handler export (low conflict risk)

---

### Developer 2 Success Criteria

- [ ] All I/O errors properly handled
- [ ] Clients removed on errors
- [ ] Body size limit enforced
- [ ] DELETE method works
- [ ] Partial server failures handled gracefully
- [ ] Server never crashes on malformed requests
- [ ] All integration tests pass
- [ ] Browser compatibility verified
- [ ] Siege shows 99.5%+ availability
- [ ] No memory leaks detected

**Total Estimated Time for Developer 2: 26-35 hours**

---

## Shared Files Coordination

### Files that Both Developers Will Modify

#### `src/application/server/server_manager.rs`
- **Developer 1 changes:** CGI-related (passing client_addr, document_root to CGI)
- **Developer 2 changes:** I/O error handling in handle_read/handle_write
- **Solution:** Developer 1 makes changes first, Developer 2 adds error handling around Developer 1's code

#### `src/core/net/connection.rs`
- **Developer 1 changes:** Expose client address accessor method
- **Developer 2 changes:** Add error state tracking
- **Solution:** Coordinate - Developer 1 adds address accessor first, Developer 2 adds error states

#### `src/application/handler/mod.rs`
- **Developer 1 changes:** None expected
- **Developer 2 changes:** May add DELETE handler export
- **Solution:** Low conflict risk, coordinate if needed

---

## Workflow Recommendations

### To Avoid Merge Conflicts:

1. **Communication:**
   - Use shared chat/document to coordinate on shared files
   - Update status when starting work on a file
   - Notify when completing work on shared files

2. **Branch Strategy:**
   - Developer 1: Work on `feature/cgi-improvements` branch
   - Developer 2: Work on `feature/stability-testing` branch
   - Merge Developer 1's branch first (fewer conflicts)
   - Then merge Developer 2's branch

3. **File Modification Order:**
   - Developer 1 completes CGI-related changes first
   - Developer 2 reviews Developer 1's changes
   - Developer 2 adds error handling around Developer 1's code
   - Both test together before final merge

4. **Testing:**
   - Developer 1: Focus on CGI functionality tests
   - Developer 2: Focus on integration and stress tests
   - Both: Run full test suite before merging

---

## Priority Order

### Week 1 (Critical Path):
**Developer 1:**
1. Linux/epoll support (blocks Linux testing)
2. PATH_INFO implementation (required for CGI)
3. REMOTE_ADDR and DOCUMENT_ROOT (required for CGI)

**Developer 2:**
1. I/O error handling (server stability)
2. Request body size enforcement (security)
3. DELETE method (required method)

### Week 2 (Important):
**Developer 1:**
4. CGI timeout
5. CGI non-blocking execution research

**Developer 2:**
4. Multiple servers partial failure handling
5. Comprehensive error handling review

### Week 3 (Testing & Polish):
**Both:**
- Complete test suites
- Browser compatibility testing
- Stress testing and memory leak detection
- Documentation

---

## Joint Success Criteria

### Final Checklist (Both Developers):
- [ ] All tests pass
- [ ] Code review completed
- [ ] Documentation updated
- [ ] Ready for audit

---

*Last Updated: Task division created*
