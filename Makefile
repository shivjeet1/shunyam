.PHONY: all build build-go build-rust install uninstall clean run-daemon run-gui help

# ─── Install prefix ────────────────────────────────────────────────────────────
PREFIX  ?= $(HOME)/.local
BINDIR  ?= $(PREFIX)/bin
DESTDIR ?=

# ─── Output directory ──────────────────────────────────────────────────────────
BUILD_DIR = build

# ─── Binaries ──────────────────────────────────────────────────────────────────
SHUNYAD        = $(BUILD_DIR)/shunyad
SHUNYA_ENGINE  = $(BUILD_DIR)/shunya-engine
SHUNYA_GUI     = $(BUILD_DIR)/shunya-gui

# ─── Default target ────────────────────────────────────────────────────────────
all: build

# ─── Build ─────────────────────────────────────────────────────────────────────
build: build-go build-rust

build-go:
	@echo "==> Building Go daemon (shunyad)..."
	@mkdir -p $(BUILD_DIR)
	@cd go && go build -o ../$(SHUNYAD) ./cmd/shunyad
	@echo "    OK  $(SHUNYAD)"

build-rust:
	@echo "==> Building Rust workspace (shunya-engine, shunya-gui)..."
	@cd crates && cargo build --release
	@mkdir -p $(BUILD_DIR)
	@cp crates/target/release/shunya-engine $(SHUNYA_ENGINE) 2>/dev/null || true
	@cp crates/target/release/shunya-gui    $(SHUNYA_GUI)    2>/dev/null || true
	@echo "    OK  $(SHUNYA_ENGINE)"
	@echo "    OK  $(SHUNYA_GUI)"

# ─── Install ───────────────────────────────────────────────────────────────────
install: build
	@echo "==> Installing to $(DESTDIR)$(BINDIR)..."
	@install -d $(DESTDIR)$(BINDIR)
	@install -m 755 $(SHUNYAD) $(DESTDIR)$(BINDIR)/shunyad
	@[ -f "$(SHUNYA_ENGINE)" ] && install -m 755 $(SHUNYA_ENGINE) $(DESTDIR)$(BINDIR)/shunya-engine || true
	@[ -f "$(SHUNYA_GUI)"    ] && install -m 755 $(SHUNYA_GUI)    $(DESTDIR)$(BINDIR)/shunya-gui    || true
	@echo "    Installed shunyad, shunya-engine, shunya-gui → $(DESTDIR)$(BINDIR)"
	@echo ""
	@echo "    Make sure $(BINDIR) is in your PATH:"
	@echo "      export PATH=\"\$$PATH:$(BINDIR)\""

# ─── Uninstall ─────────────────────────────────────────────────────────────────
uninstall:
	@echo "==> Removing installed binaries from $(DESTDIR)$(BINDIR)..."
	@rm -f $(DESTDIR)$(BINDIR)/shunyad
	@rm -f $(DESTDIR)$(BINDIR)/shunya-engine
	@rm -f $(DESTDIR)$(BINDIR)/shunya-gui
	@echo "    Done."

# ─── Run helpers ───────────────────────────────────────────────────────────────
# Starts the daemon in the foreground (Ctrl-C to stop).
run-daemon: build-go
	@echo "==> Starting shunyad on 127.0.0.1:9090..."
	@$(SHUNYAD)

# Starts the GUI (can auto-start the daemon via pkexec if not running).
run-gui: build
	@echo "==> Launching shunya-gui..."
	@$(SHUNYA_GUI)

# ─── Clean ─────────────────────────────────────────────────────────────────────
clean:
	@echo "==> Cleaning build artifacts..."
	@rm -rf $(BUILD_DIR)
	@cd crates && cargo clean
	@echo "    Done."

# ─── Help ──────────────────────────────────────────────────────────────────────
help:
	@echo ""
	@echo "Shunya Secure Wipe — Makefile targets"
	@echo "───────────────────────────────────────"
	@echo "  make build        Build all components into ./build/"
	@echo "  make build-go     Build only the Go daemon (shunyad)"
	@echo "  make build-rust   Build only the Rust workspace"
	@echo "  make install      Build + install to $(BINDIR)"
	@echo "  make uninstall    Remove installed binaries"
	@echo "  make run-daemon   Build + start shunyad in foreground"
	@echo "  make run-gui      Build + launch shunya-gui"
	@echo "  make clean        Remove build/ and Rust target/"
	@echo "  make help         Show this message"
	@echo ""
	@echo "Override install prefix:  make install PREFIX=/usr/local"
	@echo ""
