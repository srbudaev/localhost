#!/bin/bash
# Memory leak detection script
# Usage: ./memory_test.sh [CONFIG_FILE] [DURATION_SECONDS]

CONFIG_FILE=${1:-config.example.toml}
DURATION=${2:-60}

echo "Memory leak detection test"
echo "Config: ${CONFIG_FILE}"
echo "Duration: ${DURATION} seconds"
echo ""

# Check if valgrind is available (Linux)
if command -v valgrind &> /dev/null; then
    echo "Using valgrind for memory leak detection..."
    timeout ${DURATION} valgrind --leak-check=full --show-leak-kinds=all \
        --track-origins=yes \
        ./target/debug/localhost "${CONFIG_FILE}" 2>&1 | tee memory_test_valgrind.log
    echo "Results saved to memory_test_valgrind.log"
    
# Check if heaptrack is available (Linux alternative)
elif command -v heaptrack &> /dev/null; then
    echo "Using heaptrack for memory leak detection..."
    timeout ${DURATION} heaptrack \
        ./target/debug/localhost "${CONFIG_FILE}" 2>&1 | tee memory_test_heaptrack.log
    echo "Results saved to memory_test_heaptrack.log"
    
# Check if leaks is available (macOS)
elif command -v leaks &> /dev/null; then
    echo "Using leaks (macOS) for memory leak detection..."
    timeout ${DURATION} leaks --atExit -- \
        ./target/debug/localhost "${CONFIG_FILE}" 2>&1 | tee memory_test_leaks.log
    echo "Results saved to memory_test_leaks.log"
    
else
    echo "Warning: No memory leak detection tool found"
    echo "Install one of: valgrind (Linux), heaptrack (Linux), or use leaks (macOS)"
    echo ""
    echo "Running server without memory tracking..."
    echo "Monitor memory usage manually with: watch -n 1 'ps aux | grep localhost'"
    timeout ${DURATION} ./target/debug/localhost "${CONFIG_FILE}"
fi

echo ""
echo "Memory test completed"
echo "Review the log file for any memory leaks"








