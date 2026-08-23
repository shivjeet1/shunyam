# Shunya

Shunya (working name) is a secure wipe platform that aims to perform cryptographic and physical sanitization of common storage media, proving successful purges through post-wipe cryptographic verification, differential sector sampling, and validated carving. 

This repository contains the full stack:
* **shunyad (Go)**: Privileged daemon orchestrating device probes, sanitization policies, validation scoring, and signing workflows.
* **shunya-engine (Rust)**: High-speed, unprivileged child process for Direct I/O overwrites, carving validations, and DFXML translation.
* **shunya (Go)**: A lightweight CLI.
* **shunya-gui (Rust)**: Native Slint-based GUI (in-progress).

## Build Requirements

* **Go**: 1.20+
* **Rust**: `rustc` and `cargo` (latest stable).
* **Buf**: Protobuf compiler management tool (optional, for regenerating gRPC stubs).

## Building Components

### Go Daemon (`shunyad`) & CLI
```bash
cd go/cmd/shunyad && go build
cd ../shunya && go build
```

### Rust Overwrite Engine & Worker
```bash
cd crates
cargo build --release
```

## Running
Start the daemon first (requires elevated privileges in production):
```bash
./go/cmd/shunyad/shunyad
```
Then use the CLI to interact:
```bash
./go/cmd/shunya/shunya list
```

## Architecture
The platform is broken up into multiple modules utilizing gRPC over IPC (Unix Sockets/Pipes).

- `proto/`: Shared gRPC contract definitions
- `crates/`: Rust workspace consisting of OS-native ioctls, memory-aligned I/O wipers, and PhotoRec validators.
- `go/`: Go daemon and adapters for OS tools (`nvme-cli`, `hdparm`, etc.)
