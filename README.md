# Shunya Secure Wipe Platform

Shunya is an enterprise-grade secure wipe platform designed to cryptographically and physically sanitize storage media (NVMe, SATA, USB). It enforces NIST 800-88 compliance workflows, verifies every purge via deep structural file-carving heuristics, and outputs a hardware-signed cryptographic PDF certificate of destruction. It also includes an Advanced File Recovery utility powered by PhotoRec for recovering fragmented files with validation and deep carving capabilities.

---

## Components

| Binary | Language | Role |
|---|---|---|
| `shunyad` | Go | Privileged background daemon — device probing, wiping via `nvme`/`hdparm`/`shred`, SQLite job tracking, Policy Gate, gRPC server on `127.0.0.1:9090` |
| `shunya-engine` | Rust | Certificate engine — SmartCard PIV certificate signing, hardware-signed JSON/PDF manifest generation |
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

Simply launch the GUI. The **"Start Daemon"** button will spawn the daemon automatically with root privileges via `pkexec` (Polkit) if it is not already running:

```bash
shunya-gui
```

**Option B — Manual**

```bash
# Terminal 1: start the privileged daemon (requires sudo)
sudo shunyad

# Terminal 2: launch the GUI (connects automatically once daemon is live)
shunya-gui
```

**Option C — Make helpers**

```bash
# Start daemon in foreground (Ctrl-C to stop)
sudo make run-daemon

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
            ┌──────────────────┴──────────────────┐
            ▼                                     ▼
      Wipe/Sanitize                         File Recovery
            │                                     │
            ▼                                     ▼
      Select Device                         Select Device
      Authorize (Challenge)                 Select Recovery Profile
      Execute Wipe                          Execute Scan
            │                                     │
            ├──── Wiping                          ├──── Scanning
            ├──── Verifying                       └──── Done
            └──── Done ── Generate Cert
```

### Physical Presence Challenge

Before any destructive wipe is authorized, the daemon issues a one-time 4-byte hex challenge string (e.g. `WIPE-A3F9`). The operator must type it verbatim into the GUI. This prevents remote or automated execution of wipes.

---

## Architecture

```
shunya-gui  ──gRPC──▶  shunyad (Go)  ──execs──▶ nvme-cli / hdparm / shred
   (Slint)            127.0.0.1:9090                (Secure Wipe)
                           │
                           ├── SQLite DB (/tmp/shunya.db)
                           │
                           └──subprocess──▶ shunya-engine (Rust)
                                               (Cert Generation)
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
│       ├── wipe/             # Wipe execution via nvme/hdparm/shred
│       └── tools/            # lsblk adapters & helper execution
├── crates/                   # Rust workspace
│   ├── shunya-engine/        # CLI tool for certificate signing
│   ├── shunya-gui/           # Slint native UI
│   ├── shunya-carve/         # Deep file-carving validator
│   ├── shunya-cert/          # SmartCard PIV certificate signer
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


