.PHONY: all build install clean test

# Default prefix for installation
PREFIX ?= $(HOME)/.local
BINDIR ?= $(PREFIX)/bin
DESTDIR ?=

# Output directories
BUILD_DIR = ../build

all: build

build:
	@echo "==> Building Go Daemon (shunyad)..."
	@mkdir -p $(BUILD_DIR)
	@go build -o $(BUILD_DIR)/shunyad ./cmd/shunyad
	@echo "==> Go Daemon built successfully at $(BUILD_DIR)/shunyad."

install: build
	@echo "==> Installing shunyad to $(DESTDIR)$(BINDIR)..."
	@install -d $(DESTDIR)$(BINDIR)
	@install -m 755 $(BUILD_DIR)/shunyad $(DESTDIR)$(BINDIR)/shunyad
	@echo "==> Installation complete."

test:
	@echo "==> Running Go tests..."
	@go test -v ./...

clean:
	@echo "==> Cleaning Go build artifacts..."
	@rm -f $(BUILD_DIR)/shunyad
