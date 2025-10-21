# Build stage
FROM alpine:3.19 AS builder

# Install Rust and build dependencies
RUN apk add --no-cache \
    curl \
    build-base \
    musl-dev \
    openssl-dev \
    openssl-libs-static \
    pkgconfig \
    sqlite-dev \
    sqlite-static \
    libpq-dev \
    zlib-dev \
    zlib-static \
    protobuf-dev \
    protoc

# Install Rust
RUN curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y --default-toolchain stable
ENV PATH="/root/.cargo/bin:$PATH"
RUN rustup target add x86_64-unknown-linux-musl

WORKDIR /build

# Copy project files
COPY Cargo.toml ./
COPY src ./src
COPY assets ./assets

# Build static binary
ENV RUSTFLAGS='-C link-arg=-s -C target-feature=+crt-static'
ENV SQLX_OFFLINE=true
ENV PROTOC=/usr/bin/protoc
ENV PROTOC_INCLUDE=/usr/include
RUN which protoc && protoc --version
RUN cargo build --release --target x86_64-unknown-linux-musl

# Runtime stage for testing
FROM alpine:3.19

RUN apk add --no-cache \
    qemu-system-x86_64 \
    qemu-img \
    libvirt \
    nginx \
    postfix \
    bridge-utils \
    iptables \
    nftables \
    sqlite

# Copy binary from builder
COPY --from=builder /build/target/x86_64-unknown-linux-musl/release/casvps /usr/local/bin/casvps

# Create required directories
RUN mkdir -p /var/lib/casvps \
    /etc/casvps \
    /var/log/casvps

# Set permissions
RUN chmod +x /usr/local/bin/casvps

EXPOSE 8006 5900-5999 53 67 69

ENTRYPOINT ["/usr/local/bin/casvps"]
CMD ["start"]