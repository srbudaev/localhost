#!/bin/bash
# Stress test script using siege
# Usage: ./stress_test.sh [IP] [PORT] [CONCURRENT_USERS] [REQUESTS_PER_USER]

IP=${1:-127.0.0.1}
PORT=${2:-8080}
CONCURRENT=${3:-10}
REQUESTS=${4:-100}

echo "Starting stress test with siege"
echo "Target: http://${IP}:${PORT}/"
echo "Concurrent users: ${CONCURRENT}"
echo "Requests per user: ${REQUESTS}"
echo ""

# Check if siege is installed
if ! command -v siege &> /dev/null; then
    echo "Error: siege is not installed"
    echo "Install with: sudo apt-get install siege (Ubuntu/Debian)"
    echo "Or: brew install siege (macOS)"
    exit 1
fi

# Create siege URL file
URL_FILE=$(mktemp)
echo "http://${IP}:${PORT}/" > "${URL_FILE}"

# Run siege benchmark mode (-b)
echo "Running siege benchmark test..."
siege -b -c ${CONCURRENT} -r ${REQUESTS} -f "${URL_FILE}"

# Cleanup
rm -f "${URL_FILE}"

echo ""
echo "Stress test completed"
echo "Check availability percentage above - should be >= 99.5%"








