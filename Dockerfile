FROM --platform=linux/amd64 rust:1.88.0-bookworm as builder
ENV CARGO_NET_GIT_FETCH_WITH_CLI=true

RUN curl -fsSL https://deb.nodesource.com/setup_lts.x | bash - && \
    apt-get update && apt-get install -y \
    pkg-config \
    libssl-dev \
    clang \
    libclang-dev \
    nodejs \
    && rm -rf /var/lib/apt/lists/*

WORKDIR /app
COPY . .

# Build for standard Linux (glibc) instead of musl
RUN RUSTFLAGS='-C target-cpu=native' cargo build --release --features jemalloc,reth --workspace --exclude rindexer_rust_playground

FROM --platform=linux/amd64 debian:bookworm-slim
RUN apt-get update && apt-get install -y \
    libssl3 \
    ca-certificates \
    curl \
    git \
    && rm -rf /var/lib/apt/lists/*

RUN curl -L https://foundry.paradigm.xyz | bash
RUN /root/.foundry/bin/foundryup

# Copy the entire project directory
COPY --from=builder /app /app/
# Copy SSL certificates
COPY --from=builder /etc/ssl/certs/ca-certificates.crt /etc/ssl/certs/

WORKDIR /app

CMD ["/app/target/release/rindexer_cli"]