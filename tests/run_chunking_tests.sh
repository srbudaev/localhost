#!/bin/bash

# Script to run chunking-related tests

echo "=========================================="
echo "Running Chunking Tests"
echo "=========================================="
echo ""

echo "1. Testing CGI with chunked data..."
cargo test test_cgi_with_chunked_data -- --nocapture
echo ""

echo "2. Testing CGI with unchunked data (for comparison)..."
cargo test test_cgi_with_unchunked_data -- --nocapture
echo ""

echo "3. Testing chunked vs unchunked comparison..."
cargo test test_cgi_comparison_chunked_vs_unchunked -- --nocapture
echo ""

echo "4. Testing multiple chunks simulation..."
cargo test test_cgi_with_multiple_chunks_simulation -- --nocapture
echo ""

echo "5. Testing chunked response parsing..."
cargo test test_cgi_response_parsing_with_chunked_output -- --nocapture
echo ""

echo "6. Testing chunked header in environment..."
cargo test test_cgi_environment_with_chunked_header -- --nocapture
echo ""

echo "=========================================="
echo "All chunking tests completed!"
echo "=========================================="
