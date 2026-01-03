# Multi-stage build for Rust HTTP Server
# NOTE: This project is Linux-only and uses Unix-specific system calls (kqueue/epoll, fcntl, etc.)
# Docker allows running this Linux-native application on Windows/macOS
# Stage 1: Build
FROM rust:1.82-slim as builder

# Install build dependencies for Linux
RUN apt-get update && apt-get install -y \
    pkg-config \
    libssl-dev \
    build-essential \
    && rm -rf /var/lib/apt/lists/*

# Set working directory
WORKDIR /app

# Copy manifest files
COPY Cargo.toml Cargo.lock* ./

# Copy source code
COPY src ./src

# Build the application for Linux
# Since we're already in a Linux container, cargo will build for Linux by default
RUN cargo build --release

# Stage 2: Runtime
# Using Debian Linux (required - project uses Linux-specific system calls)
FROM debian:bookworm-slim

# Install runtime dependencies for CGI scripts and Linux system libraries
RUN apt-get update && apt-get install -y \
    python3 \
    perl \
    ruby \
    ca-certificates \
    && rm -rf /var/lib/apt/lists/*

# Create app user for security
RUN useradd -m -u 1000 appuser

# Set working directory
WORKDIR /app

# Copy binary from builder
COPY --from=builder /app/target/release/localhost /app/localhost

# Copy configuration and project files
COPY config.example.toml ./config.example.toml
COPY config.docker.toml ./config.docker.toml
COPY root ./root
COPY static ./static
COPY cgi-bin ./cgi-bin
COPY errors ./errors
COPY uploads ./uploads

# Use docker config as default if config.toml is not provided via volume
RUN cp config.docker.toml config.toml

# Change ownership to appuser
RUN chown -R appuser:appuser /app

# Switch to non-root user
USER appuser

# Expose port 8080
EXPOSE 8080

# Run the server
CMD ["./localhost", "config.toml"]

