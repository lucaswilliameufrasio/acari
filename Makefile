.PHONY: all setup test fmt lint check run release clean help

CARGO := cargo

all: check test

setup:
	@echo "==> Verifying Rust toolchain..."
	@command -v rustup >/dev/null 2>&1 || { echo "error: rustup not found. Install from https://rustup.rs/"; exit 1; }
	@rustup toolchain list | grep -q stable || rustup toolchain install stable
	@echo "==> Installing rustfmt and clippy..."
	rustup component add rustfmt clippy 2>/dev/null; true
	@echo "==> Installing cargo-nextest..."
	@if command -v cargo-nextest >/dev/null 2>&1; then \
		echo "  cargo-nextest already installed"; \
	else \
		if command -v cargo-binstall >/dev/null 2>&1; then \
			cargo binstall -y cargo-nextest; \
		else \
			cargo install cargo-nextest --locked; \
		fi; \
	fi
	@echo "==> Fetching dependencies..."
	$(CARGO) fetch
	@echo "==> Building to validate toolchain..."
	$(CARGO) build
	@echo ""
	@echo "Setup complete."

test:
	cargo nextest run --all-targets --all-features

fmt:
	cargo fmt --all --check

lint:
	cargo clippy --all-targets --all-features -- -D warnings

check:
	cargo check --all-targets --all-features

run:
	$(CARGO) run $(ARGS)

release:
	$(CARGO) build --release $(ARGS)

clean:
	$(CARGO) clean

help:
	@echo "Usage: make <target>"
	@echo ""
	@echo "Targets:"
	@echo "  setup   Install required tools and prepare environment"
	@echo "  test    Run all tests with nextest"
	@echo "  fmt     Check code formatting"
	@echo "  lint    Run clippy lints"
	@echo "  check   Check compilation (no tests)"
	@echo "  run     Run the application (ARGS=... for extra flags)"
	@echo "  release Build release binary"
	@echo "  clean   Remove build artifacts"
	@echo "  help    Show this help"
