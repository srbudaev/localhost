# Test Commands - Localhost HTTP Server

This document contains curl commands for testing all server functionality.

## Prerequisites

Start the server:
```bash
cargo run config.example.toml
```

## Basic Configuration Tests

### Single Server, Single Port

```bash
# Basic GET request
curl -v http://localhost:8080/

# Check response headers
curl -I http://localhost:8080/
```

### Multiple Servers, Different Ports

```bash
# Port 8080
curl -v http://localhost:8080/

# Port 8081
curl -v http://localhost:8081/

# Port 8082
curl -v http://localhost:8082/
```

### Multiple Servers, Different Hostnames (Virtual Hosting)

```bash
# Test localhost hostname
curl -v --resolve localhost:8080:127.0.0.1 http://localhost:8080/

# Test anotherhost hostname
curl -v --resolve anotherhost:8080:127.0.0.1 http://anotherhost:8080/

# Test test.com hostname
curl -v --resolve test.com:8080:127.0.0.1 http://test.com:8080/
```

## HTTP Methods

### GET Requests

```bash
# Basic GET
curl -v http://localhost:8080/

# GET with query string
curl -v "http://localhost:8080/?param1=value1&param2=value2"

# GET specific file
curl -v http://localhost:8080/index.html

# GET static file
curl -v http://localhost:8080/static/index_new.html

# GET with headers
curl -v -H "User-Agent: TestClient" -H "Accept: text/html" http://localhost:8080/
```

### POST Requests

```bash
# POST with form data
curl -v -X POST http://localhost:8080/upload \
  -H "Content-Type: application/x-www-form-urlencoded" \
  -d "name=test&value=123"

# POST with JSON
curl -v -X POST http://localhost:8080/upload \
  -H "Content-Type: application/json" \
  -d '{"key":"value"}'

# POST to CGI script
curl -v -X POST http://localhost:8080/cgi-bin/echo.py \
  -H "Content-Type: text/plain" \
  -d "Hello from POST"

# POST with file upload (multipart)
curl -v -X POST http://localhost:8080/upload \
  -F "file=@test.txt" \
  -F "description=Test file"
```

### DELETE Requests

```bash
# Create a test file first (if not exists)
echo "test content" > root/test_delete.txt

# DELETE request
curl -v -X DELETE http://localhost:8080/test_delete.txt

# DELETE non-existent file (should return 404)
curl -v -X DELETE http://localhost:8080/nonexistent.txt

# DELETE directory (should return 403)
curl -v -X DELETE http://localhost:8080/uploads/
```

### Wrong/Malformed Requests

```bash
# Invalid HTTP method
curl -v -X INVALID http://localhost:8080/

# Missing Host header
printf "GET / HTTP/1.1\r\n\r\n" | nc localhost 8080

# Malformed request line
printf "GET / \r\nHost: localhost\r\n\r\n" | nc localhost 8080

# Invalid HTTP version (should reject)
printf "GET / HTTP/2.0\r\nHost: localhost\r\n\r\n" | nc localhost 8080
```

## Error Pages

### 404 Not Found

```bash
# Non-existent file
curl -v http://localhost:8080/nonexistent.html

# Non-existent directory
curl -v http://localhost:8080/nonexistent/
```

### 403 Forbidden

```bash
# Try to DELETE directory
curl -v -X DELETE http://localhost:8080/uploads/
```

### 405 Method Not Allowed

```bash
# POST to route that only allows GET
curl -v -X POST http://localhost:8080/static/

# DELETE to route that doesn't allow DELETE
curl -v -X DELETE http://localhost:8080/static/
```

### 400 Bad Request

```bash
# Malformed request
printf "GET / HTTP/1.1\r\nHost: localhost\r\nInvalid-Header\r\n\r\n" | nc localhost 8080
```

### 413 Payload Too Large

```bash
# Create large file (over limit - default 10MB)
dd if=/dev/zero bs=1024 count=11000 > large_file.bin

# Try to upload (should fail with 413)
curl -v -X POST http://localhost:8080/upload \
  --data-binary @large_file.bin \
  -H "Content-Type: application/octet-stream"

# Cleanup
rm large_file.bin
```

### 500 Internal Server Error

```bash
# Trigger server error (if possible)
# This depends on server implementation
```

## File Upload Tests

### Upload and Retrieve

```bash
# Create test file
echo "Test content for upload" > test_upload.txt

# Upload file
curl -v -X POST http://localhost:8080/upload \
  -F "file=@test_upload.txt" \
  -o upload_response.txt

# Extract filename from response (manual step)
# Then retrieve it
curl -v http://localhost:8080/uploads/[filename]

# Upload image
curl -v -X POST http://localhost:8080/upload \
  -F "file=@test.jpg" \
  -F "description=Test image"

# Verify image integrity
curl http://localhost:8080/uploads/[filename] -o downloaded.jpg
diff test.jpg downloaded.jpg  # Should be identical
```

### Upload with Different Content Types

```bash
# Upload text file
curl -v -X POST http://localhost:8080/upload \
  -F "file=@test.txt"

# Upload binary file
curl -v -X POST http://localhost:8080/upload \
  -F "file=@test.bin" \
  -H "Content-Type: application/octet-stream"

# Upload without multipart
curl -v -X POST http://localhost:8080/upload \
  --data-binary @test.txt \
  -H "Content-Type: text/plain" \
  -H "Content-Disposition: attachment; filename=test.txt"
```

## Directory Listing

```bash
# List root directory
curl -v http://localhost:8080/

# List static directory
curl -v http://localhost:8080/static/

# List uploads directory
curl -v http://localhost:8080/uploads/
```

## Default Files

```bash
# Request directory - should serve default file
curl -v http://localhost:8080/

# Request directory with trailing slash
curl -v http://localhost:8080/static/

# Request directory without trailing slash
curl -v http://localhost:8080/static
```

## Redirects

### Temporary Redirect (302)

```bash
# Follow redirect
curl -v -L http://localhost:8080/old

# Don't follow redirect (see 302 response)
curl -v http://localhost:8080/old
```

### Permanent Redirect (301)

```bash
# Follow redirect
curl -v -L http://localhost:8080/old

# Check redirect type
curl -v -I http://localhost:8080/old | grep -i location
```

## CGI Scripts

### Basic CGI

```bash
# GET request to CGI script
curl -v http://localhost:8080/cgi-bin/echo.py

# GET with query string
curl -v "http://localhost:8080/cgi-bin/echo.py?param1=value1&param2=value2"
```

### CGI with POST (Unchunked)

```bash
# POST with Content-Length
curl -v -X POST http://localhost:8080/cgi-bin/echo.py \
  -H "Content-Type: text/plain" \
  -H "Content-Length: 13" \
  -d "Hello, World!"
```

### CGI with Chunked Data

```bash
# POST with Transfer-Encoding: chunked
curl -v -X POST http://localhost:8080/cgi-bin/echo.py \
  -H "Content-Type: text/plain" \
  -H "Transfer-Encoding: chunked" \
  --data-binary "This is chunked data"
```

### CGI with Large Data

```bash
# Create test data
dd if=/dev/urandom bs=1024 count=100 > test_data.bin

# POST large data
curl -v -X POST http://localhost:8080/cgi-bin/echo.py \
  --data-binary @test_data.bin \
  -H "Content-Type: application/octet-stream"

# Cleanup
rm test_data.bin
```

## Cookies and Sessions

### Send Cookie

```bash
# Request with cookie
curl -v -H "Cookie: session_id=abc123; user=test" http://localhost:8080/

# Multiple cookies
curl -v -H "Cookie: cookie1=value1; cookie2=value2" http://localhost:8080/
```

### Receive Cookie

```bash
# Check if server sets cookies
curl -v -c cookies.txt http://localhost:8080/cgi-bin/echo.py

# Use saved cookies
curl -v -b cookies.txt http://localhost:8080/

# View cookies
cat cookies.txt
```

### Session Management

```bash
# First request (should set session cookie)
curl -v -c session.txt http://localhost:8080/cgi-bin/echo.py

# Subsequent request with session
curl -v -b session.txt http://localhost:8080/cgi-bin/echo.py
```

## Keep-Alive Connections

```bash
# Single request
curl -v http://localhost:8080/

# Multiple requests on same connection
curl -v --keepalive-time 10 \
  http://localhost:8080/ \
  http://localhost:8080/index.html \
  http://localhost:8080/static/
```

## Route Configuration Tests

### Route Matching

```bash
# Match root route
curl -v http://localhost:8080/

# Match /static route
curl -v http://localhost:8080/static/

# Match /cgi-bin route
curl -v http://localhost:8080/cgi-bin/echo.py

# Non-matching route (404)
curl -v http://localhost:8080/nonexistent/route
```

### Method Restrictions

```bash
# Allowed method
curl -v -X GET http://localhost:8080/static/

# Disallowed method (405)
curl -v -X POST http://localhost:8080/static/

# DELETE where allowed
curl -v -X DELETE http://localhost:8080/test.txt
```

## Stress Testing

### Basic Stress Test

```bash
# Simple GET stress test
for i in {1..100}; do
  curl -s http://localhost:8080/ > /dev/null
done
```

### Siege Test

```bash
# Install siege if not available
# macOS: brew install siege
# Linux: sudo apt-get install siege

# Basic siege test (benchmark mode)
siege -b -c 10 -t 30s http://localhost:8080/

# Siege with more connections
siege -b -c 50 -t 1m http://localhost:8080/

# Siege with verbose output
siege -b -c 10 -t 30s -v http://localhost:8080/
```

### Concurrent Requests

```bash
# Run 10 concurrent requests
for i in {1..10}; do
  curl -s http://localhost:8080/ > /dev/null &
done
wait
```

### Memory Leak Test

```bash
# Monitor memory while running requests
# Terminal 1: Start server and monitor
cargo run config.example.toml &
SERVER_PID=$!
watch -n 1 "ps -p $SERVER_PID -o rss,vsz"

# Terminal 2: Run stress test
siege -b -c 100 -t 5m http://localhost:8080/
```

## Browser Testing

### Open in Browser

```bash
# macOS
open http://localhost:8080/

# Linux
xdg-open http://localhost:8080/

# Or manually navigate to:
# http://localhost:8080/
```

### Browser Developer Tools Tests

1. Open browser developer tools (F12)
2. Navigate to Network tab
3. Test various requests and verify:
   - Request headers are correct
   - Response headers are correct
   - Status codes are correct
   - Content is served correctly
   - Keep-alive connections work
   - Cookies are handled properly

## Port Configuration Tests

### Multiple Ports

```bash
# Test all configured ports
curl http://localhost:8080/
curl http://localhost:8081/
curl http://localhost:8082/
```

### Port Conflicts (Should Fail)

Create a config with duplicate ports and verify server handles error:

```toml
[[servers]]
ports = [8080]

[[servers]]
ports = [8080]  # Duplicate - should error
```

```bash
# Server should detect and report error
cargo run duplicate_ports.toml
```

## Virtual Host Tests

### Different Hostnames, Same Port

```bash
# Configure in config.toml:
# [[servers]]
# server_name = "localhost"
# ports = [8080]
#
# [[servers]]
# server_name = "test.com"
# ports = [8080]

# Test localhost
curl -v --resolve localhost:8080:127.0.0.1 http://localhost:8080/

# Test test.com
curl -v --resolve test.com:8080:127.0.0.1 http://test.com:8080/

# Test wrong hostname (should use default)
curl -v --resolve wrong.com:8080:127.0.0.1 http://wrong.com:8080/
```

## Request/Response Header Verification

### Check Request Headers

```bash
# Verbose output shows request headers
curl -v http://localhost:8080/ 2>&1 | grep ">"
```

### Check Response Headers

```bash
# Show only headers
curl -I http://localhost:8080/

# Show headers and body
curl -i http://localhost:8080/

# Check specific header
curl -I http://localhost:8080/ | grep -i "content-type"
curl -I http://localhost:8080/ | grep -i "content-length"
curl -I http://localhost:8080/ | grep -i "connection"
```

## Chunked Transfer Encoding

### Send Chunked Request

```bash
# POST with chunked encoding
curl -v -X POST http://localhost:8080/cgi-bin/echo.py \
  -H "Transfer-Encoding: chunked" \
  -H "Content-Type: text/plain" \
  --data-binary "This is chunked data"
```

### Receive Chunked Response

```bash
# Request that triggers chunked response
curl -v http://localhost:8080/cgi-bin/echo.py
```

## Error Handling Tests

### Connection Errors

```bash
# Server not running (connection refused)
curl -v http://localhost:9999/

# Wrong port
curl -v http://localhost:8080:9999/
```

### Timeout Tests

```bash
# Test with timeout
curl --max-time 5 http://localhost:8080/

# Test keep-alive timeout
# (Keep connection open longer than server timeout)
```

## Complete Test Suite

Run all tests:

```bash
#!/bin/bash

echo "=== Basic GET Test ==="
curl -s -o /dev/null -w "%{http_code}" http://localhost:8080/
echo ""

echo "=== POST Test ==="
curl -s -X POST http://localhost:8080/upload -d "test=data" -o /dev/null -w "%{http_code}"
echo ""

echo "=== DELETE Test ==="
echo "test" > root/test_delete.txt
curl -s -X DELETE http://localhost:8080/test_delete.txt -o /dev/null -w "%{http_code}"
echo ""

echo "=== 404 Test ==="
curl -s -o /dev/null -w "%{http_code}" http://localhost:8080/nonexistent
echo ""

echo "=== CGI Test ==="
curl -s http://localhost:8080/cgi-bin/echo.py -o /dev/null -w "%{http_code}"
echo ""

echo "=== Virtual Host Test ==="
curl -s --resolve test.com:8080:127.0.0.1 http://test.com:8080/ -o /dev/null -w "%{http_code}"
echo ""

echo "Tests complete!"
```

Save as `run_tests.sh`, make executable (`chmod +x run_tests.sh`), and run.
