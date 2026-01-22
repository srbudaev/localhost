#!/usr/bin/env python3
import sys
import os
import urllib.parse

# Get request method
request_method = os.environ.get('REQUEST_METHOD', 'GET')

# Parse user input based on method
user_input = ''

if request_method == 'GET':
    # GET: read from QUERY_STRING
    query_string = os.environ.get('QUERY_STRING', '')
    if query_string:
        params = urllib.parse.parse_qs(query_string)
        if 'input' in params:
            user_input = params['input'][0]
elif request_method == 'POST':
    # POST: read from stdin
    content_length = int(os.environ.get('CONTENT_LENGTH', '0'))
    if content_length > 0:
        post_data = sys.stdin.read(content_length)
        params = urllib.parse.parse_qs(post_data)
        if 'input' in params:
            user_input = params['input'][0]

# URL decode the input
user_input = urllib.parse.unquote_plus(user_input)

# Output CGI headers with proper CRLF line endings
sys.stdout.write('Content-Type: text/html\r\n')
sys.stdout.write('Status: 200 OK\r\n')
sys.stdout.write('\r\n')  # Blank line separates headers from body

# Output HTML response
html_body = f'''<!DOCTYPE html>
<html lang="en">
<head>
    <meta charset="UTF-8">
    <meta name="viewport" content="width=device-width, initial-scale=1.0">
    <title>CGI Echo - User Input</title>
    <style>
        body {{
            font-family: Arial, sans-serif;
            max-width: 800px;
            margin: 50px auto;
            padding: 20px;
            background-color: #f5f5f5;
        }}
        h1 {{
            color: #333;
        }}
        .result {{
            background-color: #e3f2fd;
            border-left: 4px solid #2196F3;
            padding: 20px;
            margin: 20px 0;
            border-radius: 4px;
        }}
        .input-display {{
            font-size: 1.2em;
            color: #1976D2;
            font-weight: bold;
            word-wrap: break-word;
        }}
        .method {{
            color: #666;
            font-size: 0.9em;
        }}
        a {{
            color: #2196F3;
            text-decoration: none;
        }}
        a:hover {{
            text-decoration: underline;
        }}
    </style>
</head>
<body>
    <h1>CGI Script Response</h1>
    <div class="result">
        <p class="method">Method: <strong>{request_method}</strong></p>
        <p>Your input:</p>
        <p class="input-display">{user_input if user_input else '(no input provided)'}</p>
    </div>
    <p><a href="/cgi-bin.html">← Back to form</a></p>
</body>
</html>'''

sys.stdout.write(html_body)
sys.stdout.flush()
