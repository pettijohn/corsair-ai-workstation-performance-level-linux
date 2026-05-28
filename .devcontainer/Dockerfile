FROM mcr.microsoft.com/devcontainers/rust:1-bookworm

# Install SQLite development libraries and other tools
RUN apt-get update && apt-get install -y \
    sqlite3 libsqlite3-dev \
    curl \
    && apt-get clean && rm -rf /var/lib/apt/lists/*
