# Build stage
FROM rust:1.75-alpine AS builder

# Install build dependencies
RUN apk add --no-cache musl-dev openssl-dev openssl-libs-static pkgconfig

WORKDIR /build

# Copy project files
COPY Cargo.toml Cargo.lock ./
COPY src ./src

# Build static binary
ENV RUSTFLAGS='-C link-arg=-s -C target-feature=+crt-static'
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