FROM ubuntu:24.04

ENV DEBIAN_FRONTEND=noninteractive
ENV RUSTUP_HOME=/usr/local/rustup
ENV CARGO_HOME=/usr/local/cargo
ENV PATH=/usr/local/cargo/bin:${PATH}

RUN apt-get update && apt-get install -y --no-install-recommends \
    bash \
    build-essential \
    ca-certificates \
    clang \
    curl \
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
    zlib1g-dev \
 && rm -rf /var/lib/apt/lists/*

RUN curl https://sh.rustup.rs -sSf | bash -s -- -y --profile minimal --default-toolchain stable

WORKDIR /workspace

CMD ["/bin/bash"]
