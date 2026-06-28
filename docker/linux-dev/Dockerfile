ARG BASE_IMAGE=ubuntu:24.04
FROM ${BASE_IMAGE}

ENV DEBIAN_FRONTEND=noninteractive
ENV RUSTUP_HOME=/usr/local/rustup
ENV CARGO_HOME=/usr/local/cargo
ENV PATH=/usr/local/cargo/bin:${PATH}
ARG APT_MIRROR=
ARG RUSTUP_INIT_URL=https://sh.rustup.rs
ARG RUSTUP_INIT_FALLBACK_URL=https://sh.rustup.rs
ARG RUSTUP_DIST_SERVER=https://static.rust-lang.org
ARG RUSTUP_UPDATE_ROOT=https://static.rust-lang.org/rustup
ARG RUSTUP_INSTALL_TIMEOUT_SECONDS=600
ENV RUSTUP_DIST_SERVER=${RUSTUP_DIST_SERVER}
ENV RUSTUP_UPDATE_ROOT=${RUSTUP_UPDATE_ROOT}

RUN if [ -n "${APT_MIRROR}" ]; then \
      sed -i "s|http://archive.ubuntu.com/ubuntu|${APT_MIRROR}|g; s|http://security.ubuntu.com/ubuntu|${APT_MIRROR}|g" /etc/apt/sources.list /etc/apt/sources.list.d/*.sources 2>/dev/null || true; \
    fi

RUN apt-get update && apt-get install -y --no-install-recommends \
    bash \
    build-essential \
    ca-certificates \
    clang \
    curl \
    dpkg-dev \
    file \
    git \
    iproute2 \
    jq \
    libbpf-dev \
    libclang-dev \
    libelf-dev \
    linux-tools-common \
    llvm \
    make \
    pkg-config \
    python3 \
    rpm \
    zlib1g-dev \
 && rm -rf /var/lib/apt/lists/*

RUN if ! curl --fail --location --show-error --silent "${RUSTUP_INIT_URL}" -o /tmp/rustup-init; then \
      echo "warning: rustup bootstrap failed from ${RUSTUP_INIT_URL}; retrying ${RUSTUP_INIT_FALLBACK_URL}" >&2; \
      curl --fail --location --show-error --silent "${RUSTUP_INIT_FALLBACK_URL}" -o /tmp/rustup-init; \
    fi \
 && chmod +x /tmp/rustup-init \
 && timeout "${RUSTUP_INSTALL_TIMEOUT_SECONDS}" /tmp/rustup-init -y --profile minimal --default-toolchain stable \
    || { status="$?"; \
      if [ "${status}" = "124" ]; then \
        echo "error: rustup bootstrap timed out after ${RUSTUP_INSTALL_TIMEOUT_SECONDS}s" >&2; \
      fi; \
      exit "${status}"; \
    } \
 && rm -f /tmp/rustup-init

WORKDIR /workspace

CMD ["/bin/bash"]
