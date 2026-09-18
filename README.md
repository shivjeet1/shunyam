# Shunya Secure Wipe Platform

Shunya is an enterprise-grade secure wipe platform designed to cryptographically and physically sanitize storage media (NVMe, SATA, USB). It enforces NIST 800-88 compliance workflows, verifies every purge via deep structural file-carving heuristics, and outputs a hardware-signed cryptographic PDF certificate of destruction.

---

## Components

| Binary | Language | Role |
|---|---|---|
| `shunyad` | Go | Privileged background daemon — device probing, SQLite job tracking, Policy Gate (physical presence challenge), gRPC server on `127.0.0.1:9090` |
| `shunya-engine` | Rust | High-speed wipe engine — `O_DIRECT \| O_SYNC` ChaCha20 overwrite, deep JPEG/PNG carving validation, SmartCard PIV certificate signing |
| `shunya-gui` | Rust/Slint | Native cross-platform GUI — connects to `shunyad` over gRPC, real-time progress streaming, wipe-standard selection, compliance certificate export |
| `iso/` | Shell/Alpine | Bootable Alpine Linux Live-USB builder for air-gapped bare-metal sanitization |

---

## Build Requirements

| Dependency | Minimum Version | Notes |
|---|---|---|
| Go | 1.20+ | For `shunyad` daemon |
| Rust + Cargo | stable (latest) | For `shunya-engine`, `shunya-gui` |
| `pcsc-lite` dev headers | any | Linux/macOS — required for SmartCard/YubiKey PIV signing |
| `libclang` / `clang` | any | Required by Rust bindgen for native IOCTLs |

Install `pcsc-lite` headers on common distros:
```bash
# Debian / Ubuntu
sudo apt install libpcsclite-dev

# Arch / Manjaro
sudo pacman -S ccid pcsclite

# macOS (Homebrew)
brew install pcsc-lite
```

---

## Quick Start

### 1. Build

```bash
# Build everything into ./build/
make build

# Or build components individually
make build-go     # shunyad only
make build-rust   # shunya-engine + shunya-gui only
```

### 2. Install to `~/.local/bin`

```bash
make install
```

This copies `shunyad`, `shunya-engine`, and `shunya-gui` to `~/.local/bin`.  
Ensure the directory is in your `$PATH`:

```bash
export PATH="$PATH:$HOME/.local/bin"
# Add to ~/.bashrc or ~/.zshrc to persist
```

### 3. Run

**Option A — GUI auto-manages the daemon (recommended)**

Simply launch the GUI. The **"Start shunyad"** button will spawn the daemon automatically if it is not already running:

```bash
shunya-gui
```

**Option B — Manual**

```bash
# Terminal 1: start the privileged daemon
shunyad

# Terminal 2: launch the GUI (connects automatically once daemon is live)
shunya-gui
```

**Option C — Make helpers**

```bash
# Start daemon in foreground (Ctrl-C to stop)
make run-daemon

# In a second terminal, start the GUI
make run-gui
```

### 4. Uninstall

```bash
make uninstall
```

---

## GUI Workflow

```
┌──────────────┐     ┌──────────────────┐     ┌──────────────────────┐
│  Setup       │────▶│  Ready           │────▶│  Active              │
│  (Offline)   │     │  (Daemon Live)   │     │  (Main Application)  │
└──────────────┘     └──────────────────┘     └──────────────────────┘
                                                        │
                           ┌────────────────────────────┤
                           ▼                            ▼
                    Select Device              Wipe Method Dropdown
                    Authorize (Challenge)      (NIST Crypto Erase / Quick Format / Purge / Clear / DoD)
                    Execute Wipe                        │
                           │                            ▼
                           ├──── Wiping ──── Live progress stream
                           ├──── Verifying ── Deep carving audit (shunya-carve)
                           └──── Done ──── Generate Compliance Certificate
                                           (SmartCard-signed PDF via shunya-cert)
```

### Physical Presence Challenge

Before any destructive wipe is authorized, the daemon issues a one-time 4-byte hex challenge string (e.g. `WIPE-A3F9`). The operator must type it verbatim into the GUI. This prevents remote or automated execution of wipes.

---

## Architecture

```
shunya-gui  ──gRPC──▶  shunyad (Go)  ──subprocess──▶  shunya-engine (Rust)
   (Slint)            127.0.0.1:9090     stdin/stdout      (O_DIRECT I/O)
                           │                                     │
                       SQLite DB                          shunya-carve
                      /tmp/shunya.db                     shunya-cert (PIV)
```

Key design principles:
- **Privilege separation**: `shunyad` holds elevated access; `shunya-gui` is fully unprivileged.
- **Physical presence enforcement**: No wipe can be initiated without a human typing a random challenge.
- **Zero clipboard leakage**: Challenge codes are generated in-memory and never written to disk.
- **Cryptographic auditability**: Every wipe produces a hardware-signed JSON + PDF manifest.

### Directory Layout

```
shunyam/
├── Makefile                  # Unified build & install
├── proto/                    # Protobuf / gRPC definitions
│   └── shunya/v1/shunya.proto
├── go/                       # Go workspace
│   ├── cmd/shunyad/          # Daemon entrypoint + gRPC server
│   └── internal/
│       ├── db/               # SQLite job store
│       ├── job/              # State machine (Pending→Wiping→Verifying→Done)
│       ├── policy/           # Physical presence gate
│       └── tools/            # nvme-cli / shunya-engine adapters
├── crates/                   # Rust workspace
│   ├── shunya-engine/        # CLI wipe orchestrator
│   ├── shunya-gui/           # Slint native UI
│   ├── shunya-carve/         # Deep file-carving validator
│   ├── shunya-cert/          # SmartCard PIV certificate signer
│   ├── shunya-io/            # O_DIRECT I/O primitives
│   └── shunya-proto/         # Rust-side gRPC stubs (tonic)
└── iso/                      # Alpine Linux ISO builder
    ├── build.sh
    ├── mkimg.shunya.sh
    ├── aports/shunya/APKBUILD
    └── overlay/              # Init scripts, sysconfig
```

---

## Makefile Reference

| Target | Description |
|---|---|
| `make build` | Build all components into `./build/` |
| `make build-go` | Build `shunyad` only |
| `make build-rust` | Build `shunya-engine` + `shunya-gui` only |
| `make install` | Build + install to `~/.local/bin` |
| `make uninstall` | Remove installed binaries |
| `make run-daemon` | Build + start `shunyad` in foreground |
| `make run-gui` | Build + launch `shunya-gui` |
| `make clean` | Remove `./build/` and Rust `target/` |
| `make help` | Print target summary |

Override the install prefix:
```bash
make install PREFIX=/usr/local   # installs to /usr/local/bin
```

---

## Known Issues & Notes

- `ListDevices` on Linux requires `nvme-cli` to be installed (`sudo apt install nvme-cli`) for NVMe enumeration. Without it, the device list will be empty, but the daemon still starts and all other functions remain available.
- `shunya-engine generate-cert` requires a PCSC-compatible SmartCard reader and a PIV-provisioned token (e.g. YubiKey 5). Without hardware, the engine falls back to a software mock signature.
- The daemon logs to stderr. Redirect with `shunyad 2>/tmp/shunyad.log` for quiet background operation.
