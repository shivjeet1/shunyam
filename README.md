# Shunya Secure Wipe Platform

Shunya is an enterprise-grade secure wipe platform designed to perform cryptographic and physical sanitization of storage media (NVMe, SATA, USB). It strictly enforces compliance workflows (NIST 800-88), verifies the purge via differential sector sampling and file-carving heuristics, and outputs a hardware-signed cryptographic PDF certificate of destruction.

This repository contains the full cross-platform stack (Linux, macOS, Windows):
* **shunyad (Go)**: Privileged daemon orchestrating device probes, SIP/Policy enforcement, SQLite job tracking, and SmartCard signing workflows.
* **shunya-engine (Rust)**: High-speed child process executing `O_DIRECT | O_SYNC` I/O overwrites via a ChaCha20 random stream, carving validations, and hardware token (PIV) PKI interactions.
* **shunya-gui (Rust)**: Native, reactive Slint-based GUI communicating with `shunyad` over TCP/gRPC.
* **shunya (Go)**: A lightweight CLI.
* **iso/**: Tooling to build an Alpine Linux Live USB (ISO) for bare-metal airgapped sanitization.

## Build Requirements

* **Go**: 1.20+
* **Rust**: `rustc` and `cargo` (latest stable).
* **SmartCard Dependencies**: `pcsc-lite` development headers (Linux/macOS) for hardware PKI integration.

## Installation (Make)

A unified Makefile is provided at the repository root to compile the entire Go and Rust stack.

```bash
# Build all components (shunyad, shunya-engine, shunya-gui) into the ./build directory
make build

# Install binaries to your local path (~/.local/bin)
make install
```

## Running

1. **Start the background daemon**:
   (Requires elevated privileges on macOS/Windows to access raw block devices)
   ```bash
   shunyad
   ```

2. **Launch the Kiosk UI**:
   The GUI will connect to the daemon over local gRPC (`127.0.0.1:9090`).
   ```bash
   shunya-gui
   ```

3. **CLI Usage (Optional)**:
   ```bash
   shunya list
   ```

## Architecture

The platform operates across process boundaries using Protocol Buffers (`shunya.proto`):
- `proto/`: Shared gRPC contract definitions.
- `crates/`: Rust workspace (I/O wipers, hardware native IOCTLs, deep structural file carving validators, and `pcsc` smartcard signers).
- `go/`: Go daemon containing the Job State Machine, Policy Gates (checking `/proc/mounts`, enforcing physical presence challenges), and IPC bridge handlers.
- `iso/`: Alpine Linux build scripts (APKBUILDs) and automatic USB certificate extraction daemons.
