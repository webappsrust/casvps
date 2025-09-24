# CasVPS - Complete Application Server for Virtualization

A comprehensive virtualization platform that runs on everything from Raspberry Pi 4 to enterprise datacenters.

## Features

- **Single Static Binary**: ~800MB-1GB Rust binary with all functionality embedded
- **Universal Platform Support**: Runs on Raspberry Pi 4/5, homelab servers, and enterprise hardware
- **Complete Virtualization**: QEMU/KVM VMs, Incus containers, Docker CE support
- **Built-in Clustering**: Raft consensus for high availability
- **Per-User SDN**: Isolated networks for each user
- **Always-On Security**: GeoIP, fail2ban, Suricata, ClamAV enabled by default
- **Smart Resource Management**: Automatic optimization based on available hardware
- **Database-Only Configuration**: All settings in SQLite, no config files
- **Web Interface**: Modern web UI on port 8006

## Quick Start

```bash
# Download latest release
wget https://github.com/casapps/casvps/releases/latest/download/casvps-linux-amd64
chmod +x casvps-linux-amd64
sudo mv casvps-linux-amd64 /usr/local/bin/casvps

# Start CasVPS
sudo casvps start

# Access web interface
# https://your-server:8006
```

## System Requirements

### Minimum (Raspberry Pi 4)
- 2GB RAM
- 20GB storage
- 4 CPU cores

### Recommended (Homelab)
- 32GB RAM
- 500GB SSD
- 8+ CPU cores
- Virtualization extensions (VT-x/AMD-V)

### Enterprise
- 128GB+ RAM
- Multi-TB storage
- 32+ CPU cores
- All virtualization features

## Development

### Using Docker

```bash
# Build the project
docker-compose run --rm dev cargo build

# Run tests
docker-compose run --rm dev cargo test

# Start development environment
docker-compose up dev

# Start test instance
docker-compose up test
```

### Manual Build

```bash
# Install Rust
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh

# Build release binary
cargo build --release --target x86_64-unknown-linux-musl

# Binary will be at target/x86_64-unknown-linux-musl/release/casvps
```

## Documentation

Full documentation available at `/support` when CasVPS is running.

## License

MIT License - See [LICENSE.md](LICENSE.md) for details.

## Support

- GitHub Issues: https://github.com/casapps/casvps/issues
- Documentation: Access via web interface at `/support`

## Project Structure

```
casvps/
├── src/                  # Rust source code
│   ├── main.rs          # CLI entry point
│   ├── core/            # Core system logic
│   ├── database/        # SQLite database layer
│   ├── services/        # Service management
│   ├── network/         # Network management
│   ├── virtualization/  # VM/container management
│   └── web/             # Web interface
├── Cargo.toml           # Rust dependencies
├── Dockerfile           # Docker build environment
├── docker-compose.yml   # Development environment
├── LICENSE.md           # MIT license
└── README.md            # This file
```

## Contributing

Contributions are welcome! Please submit pull requests to the main repository.

---

**CasVPS** - Enterprise virtualization for everyone, from Pi to datacenter.

Copyright (c) 2025 CasjaysDev Apps