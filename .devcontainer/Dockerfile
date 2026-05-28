FROM mcr.microsoft.com/devcontainers/rust:1-bookworm

# Install tools for Rust development, out-of-tree kernel module builds, module
# signing, and ACPI/WMI inspection.
RUN apt-get update && apt-get install -y \
    acpica-tools \
    bc \
    bindgen \
    bison \
    build-essential \
    clang \
    curl \
    dwarves \
    flex \
    kmod \
    libclang-dev \
    libelf-dev \
    lld \
    llvm \
    mokutil \
    openssl \
    pkg-config \
    sqlite3 libsqlite3-dev \
    && apt-get clean && rm -rf /var/lib/apt/lists/*
