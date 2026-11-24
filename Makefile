# YhHTTPServer — build automation
#
# Run `make` for the native release build, `make help` for everything else.

CARGO    ?= cargo
BIN       = yhtps
TARGETDIR = target

.PHONY: help all build release debug musl arm64 windows macos test smoke fmt \
        clippy check clean install uninstall dist pure

help: ## show this help
	@grep -E '^[a-zA-Z_-]+:.*?## .*$$' $(MAKEFILE_LIST) \
		| awk 'BEGIN{FS=":.*?## "}{printf "  \033[36m%-12s\033[0m %s\n", $$1, $$2}'

all: release ## alias for `release`

build: debug ## alias for `debug`

debug: ## debug build (fast compile)
	$(CARGO) build

release: ## optimised native build
	$(CARGO) build --release
	@echo
	@echo "binary: $(TARGETDIR)/release/$(BIN)"

pure: ## release build without TLS (pure Rust, no C deps)
	$(CARGO) build --release --no-default-features

dist: ## build + package the full release matrix into dist/
	bash scripts/release-matrix.sh dist
	@echo && ls -la dist

musl: ## static x86_64 Linux binary (no glibc)
	rustup target add x86_64-unknown-linux-musl
	$(CARGO) build --release --target x86_64-unknown-linux-musl --no-default-features
	@ls -la $(TARGETDIR)/x86_64-unknown-linux-musl/release/$(BIN)

arm64: ## Linux ARM64 (aarch64)
	rustup target add aarch64-unknown-linux-gnu
	CARGO_TARGET_AARCH64_UNKNOWN_LINUX_GNU_LINKER=aarch64-linux-gnu-gcc \
		$(CARGO) build --release --target aarch64-unknown-linux-gnu --no-default-features

windows: ## Windows x86_64 (MinGW)
	rustup target add x86_64-pc-windows-gnu
	$(CARGO) build --release --target x86_64-pc-windows-gnu --no-default-features
	@ls -la $(TARGETDIR)/x86_64-pc-windows-gnu/release/$(BIN).exe

macos: ## macOS universal binary (must be run on macOS)
	rustup target add x86_64-apple-darwin aarch64-apple-darwin
	$(CARGO) build --release --target x86_64-apple-darwin
	$(CARGO) build --release --target aarch64-apple-darwin
	lipo -create \
		$(TARGETDIR)/x86_64-apple-darwin/release/$(BIN) \
		$(TARGETDIR)/aarch64-apple-darwin/release/$(BIN) \
		-output $(TARGETDIR)/$(BIN)-universal

test: ## run the test suite
	$(CARGO) test

smoke: release ## end-to-end HTTP smoke test against the built binary
	bash scripts/smoke.sh

fmt: ## format the source
	$(CARGO) fmt

clippy: ## run clippy with warnings as errors
	$(CARGO) clippy --all-targets -- -D warnings

check: ## fast type check
	$(CARGO) check --all-targets

install: release ## install to /usr/local/bin (override PREFIX=...)
	install -d "$(DESTDIR)$(PREFIX)/bin"
	install -m 0755 $(TARGETDIR)/release/$(BIN) "$(DESTDIR)$(PREFIX)/bin/$(BIN)"
	-install -d "$(DESTDIR)$(PREFIX)/share/man/man1"
	-install -m 0644 docs/$(BIN).1 "$(DESTDIR)$(PREFIX)/share/man/man1/$(BIN).1"
	@echo "installed $(DESTDIR)$(PREFIX)/bin/$(BIN)"

uninstall: ## remove the installed binary
	rm -f "$(DESTDIR)$(PREFIX)/bin/$(BIN)"

clean: ## remove build artefacts
	$(CARGO) clean

PREFIX ?= /usr/local
