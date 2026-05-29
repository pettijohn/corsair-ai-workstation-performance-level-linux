FROM ubuntu:26.04

ARG DEBIAN_FRONTEND=noninteractive
ARG USERNAME=vscode
ARG USER_UID=1000
ARG USER_GID=1000

# Match the target host family. The Ubuntu 26.04 / Linux 7.0+ kernel headers
# expect a modern glibc and compiler toolchain; Debian 12 is too old for them.
RUN apt-get update && apt-get install -y --no-install-recommends \
    acpica-tools \
    bc \
    bindgen \
    bison \
    build-essential \
    ca-certificates \
    clang \
    curl \
    dwarves \
    flex \
    git \
    g++ \
    gcc \
    kmod \
    libclang-dev \
    libelf-dev \
    lld \
    llvm \
    make \
    mokutil \
    openssl \
    pkg-config \
    rust-clippy \
    rust-src \
    rustc \
    cargo \
    rustfmt \
    sudo \
    sqlite3 libsqlite3-dev \
    zsh \
    && apt-get clean && rm -rf /var/lib/apt/lists/*

# /usr/src is bind-mounted from the host at runtime. Keep a copy of the Ubuntu
# rust-src package in /opt so Kbuild can always find matching Rust stdlib source.
RUN cp -a /usr/src/rustc-1.93.1 /opt/rustc-1.93.1

RUN set -eux; \
    if getent group "${USER_GID}" >/dev/null; then \
        existing_group="$(getent group "${USER_GID}" | cut -d: -f1)"; \
        if [ "${existing_group}" != "${USERNAME}" ]; then \
            groupmod -n "${USERNAME}" "${existing_group}"; \
        fi; \
    else \
        groupadd --gid "${USER_GID}" "${USERNAME}"; \
    fi; \
    if getent passwd "${USER_UID}" >/dev/null; then \
        existing_user="$(getent passwd "${USER_UID}" | cut -d: -f1)"; \
        if [ "${existing_user}" != "${USERNAME}" ]; then \
            usermod -l "${USERNAME}" "${existing_user}"; \
        fi; \
        usermod --gid "${USER_GID}" --home "/home/${USERNAME}" --move-home --shell /bin/zsh "${USERNAME}"; \
    else \
        useradd --uid "${USER_UID}" --gid "${USER_GID}" -m -s /bin/zsh "${USERNAME}"; \
    fi; \
    echo "${USERNAME} ALL=(ALL) NOPASSWD:ALL" >"/etc/sudoers.d/${USERNAME}"; \
    chmod 0440 "/etc/sudoers.d/${USERNAME}"; \
    mkdir -p "/home/${USERNAME}/.vscode-server"; \
    chown -R "${USER_UID}:${USER_GID}" "/home/${USERNAME}"

USER ${USERNAME}
ENV CARGO_HOME=/home/${USERNAME}/.cargo
ENV RUST_LIB_SRC=/opt/rustc-1.93.1/library
