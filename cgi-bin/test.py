#!/usr/bin/env python3
# Example CGI script for Localhost HTTP Server

import os
import sys

print("Content-Type: text/html\n")
print("""
<!DOCTYPE html>
<html lang="en">
<head>
    <meta charset="UTF-8">
    <title>CGI Test</title>
    <style>
        body { font-family: Arial, sans-serif; max-width: 800px; margin: 50px auto; padding: 20px; }
        h1 { color: #333; }
        .info { background-color: #e3f2fd; padding: 15px; border-radius: 5px; margin: 10px 0; }
    </style>
</head>
<body>
    <h1>CGI Script Test</h1>
    <div class="info">
        <h2>Environment Variables:</h2>
        <p><strong>REQUEST_METHOD:</strong> {}</p>
        <p><strong>SCRIPT_NAME:</strong> {}</p>
        <p><strong>QUERY_STRING:</strong> {}</p>
        <p><strong>SERVER_NAME:</strong> {}</p>
        <p><strong>SERVER_PORT:</strong> {}</p>
    </div>
    <p>CGI script executed successfully!</p>
</body>
</html>
""".format(
    os.environ.get('REQUEST_METHOD', 'N/A'),
    os.environ.get('SCRIPT_NAME', 'N/A'),
    os.environ.get('QUERY_STRING', 'N/A'),
    os.environ.get('SERVER_NAME', 'N/A'),
    os.environ.get('SERVER_PORT', 'N/A')
))

