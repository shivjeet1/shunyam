.PHONY: all build build-go build-rust install clean

# Default prefix for installation
PREFIX ?= /usr/local
BINDIR ?= $(PREFIX)/bin

# Output directories
BUILD_DIR = build

all: build

build: build-go build-rust

build-go:
	@echo "==> Building Go Daemon (shunyad)..."
	@mkdir -p $(BUILD_DIR)
	@cd go && go build -o ../$(BUILD_DIR)/shunyad ./cmd/shunyad
	@echo "==> Go Daemon built successfully at $(BUILD_DIR)/shunyad."

build-rust:
	@echo "==> Building Rust Workspace (shunya-engine, shunya-gui)..."
	@cd crates && cargo build --release
	@mkdir -p $(BUILD_DIR)
	@cp crates/target/release/shunya-engine $(BUILD_DIR)/ || true
	@cp crates/target/release/shunya-gui $(BUILD_DIR)/ || true
	@echo "==> Rust binaries built successfully."

install: build
	@echo "==> Installing Shunya platform to $(BINDIR)..."
	@install -d $(DESTDIR)$(BINDIR)
	@install -m 755 $(BUILD_DIR)/shunyad $(DESTDIR)$(BINDIR)/shunyad
	@if [ -f "$(BUILD_DIR)/shunya-engine" ]; then \
		install -m 755 $(BUILD_DIR)/shunya-engine $(DESTDIR)$(BINDIR)/shunya-engine; \
	fi
	@if [ -f "$(BUILD_DIR)/shunya-gui" ]; then \
		install -m 755 $(BUILD_DIR)/shunya-gui $(DESTDIR)$(BINDIR)/shunya-gui; \
	fi
	@echo "==> Installation complete."

clean:
	@echo "==> Cleaning build artifacts..."
	@rm -rf $(BUILD_DIR)
	@cd crates && cargo clean
	@echo "==> Clean complete."
