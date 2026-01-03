@echo off
REM Quick start script for Windows
echo Building and starting Localhost HTTP Server in Docker...
echo.

REM Check if config.toml exists, if not use config.docker.toml
if not exist config.toml (
    echo config.toml not found, using config.docker.toml...
    copy config.docker.toml config.toml >nul
)

REM Start docker-compose
docker-compose up --build

pause

