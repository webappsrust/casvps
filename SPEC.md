# Project-Specific Overrides

## CasVPS Server Host Platform

The CasVPS server executable and server runtime are supported on Linux hosts only, on the AMD64 and ARM64 architectures. Windows, macOS, and FreeBSD are not supported server hosts. This overrides AI.md PART 3's operating-system support requirements for the CasVPS server executable and runtime.

The server requires host virtualization support through QEMU and libvirt. This override does not change support for Windows or macOS as guest operating systems, and it does not change AI.md platform requirements for a separately built companion client.
