.PHONY: help all clean test build release lint fmt check-fmt markdownlint nixie typecheck python-test-deps publish-check powershell-wrapper-validate FORCE

CRATE ?= ortho_config
CARGO ?= cargo
PUBLISH_CHECK_CARGO_REAL ?= $(shell command -v $(CARGO))
BUILD_JOBS ?=
CLIPPY_FLAGS ?= --all-targets --all-features -- -D warnings
MDLINT ?= markdownlint-cli2
NIXIE ?= nixie
PUBLISH_CHECK_FLAGS ?=
PYTHON_VENV ?= scripts/.venv
UV ?= uv
PYTHON_VERSION ?= 3.13
PYTHON_DEPS_FILE ?= scripts/requirements-test.txt
PYTEST_FLAGS ?= --doctest-modules scripts/bump_version.py scripts/tests -q
LADING ?= uvx --from git+https://github.com/leynos/lading lading
POWERSHELL ?= pwsh
ifeq ($(OS),Windows_NT)
NULL_DEVICE ?= NUL
else
NULL_DEVICE ?= /dev/null
endif
UV_RUN := $(UV) run --python $(PYTHON_VERSION) --with-requirements $(PYTHON_DEPS_FILE)
PYTEST ?= $(UV_RUN) --module pytest
RUSTDOC_FLAGS ?= -D warnings

build: target/debug/lib$(CRATE).rlib ## Build debug library
release: target/release/lib$(CRATE).rlib ## Build release library

all: check-fmt typecheck lint test markdownlint nixie

clean: ## Remove build artifacts
	$(CARGO) clean
	rm -rf $(PYTHON_VENV)

test: python-test-deps ## Run tests with warnings treated as errors
	RUSTFLAGS="-D warnings" $(CARGO) test --all-targets --all-features $(BUILD_JOBS)
	$(PYTEST) $(PYTEST_FLAGS)

python-test-deps: ## Ensure Python test dependencies are provisioned
	$(PYTEST) --version > $(NULL_DEVICE)

# will match target/debug/libmy_library.rlib and target/release/libmy_library.rlib
target/%/lib$(CRATE).rlib: FORCE ## Build library in debug or release
	$(CARGO) build $(BUILD_JOBS)                            \
	  $(if $(findstring release,$(@)),--release)            \
	  --lib
	@# Copy artefacts only when the cargo output and make target differ.
	src=target/$(if $(findstring release,$(@)),release,debug)/lib$(CRATE).rlib; \
	if [ "$$src" != "$@" ]; then \
	  install -Dm644 "$$src" "$@"; \
	fi

FORCE:

lint: ## Run Clippy with warnings denied
	RUSTDOCFLAGS="$(RUSTDOC_FLAGS)" $(CARGO) doc --workspace --no-deps
	$(CARGO) clippy $(CLIPPY_FLAGS)

typecheck: ## Typecheck workspace (cargo check)
	RUSTFLAGS="-D warnings" $(CARGO) check --workspace --all-targets --all-features $(BUILD_JOBS)

fmt: ## Format Rust and Markdown sources
	$(CARGO) fmt --all
	mdformat-all

check-fmt: ## Verify formatting
	$(CARGO) fmt --all -- --check

markdownlint: ## Lint Markdown files
	$(MDLINT) "**/*.md"

nixie:
	# CI currently requires --no-sandbox; remove once nixie supports
	# environment variable control for this option
	$(NIXIE) --no-sandbox

powershell-wrapper-validate: ## Validate PowerShell wrapper output (Windows only)
ifeq ($(OS),Windows_NT)
	$(POWERSHELL) -File scripts/validate_powershell_wrapper.ps1
else
	@echo "Skipping PowerShell wrapper validation (not Windows)."
endif

publish-check: ## Run Lading publish pre-flight checks
	$(LADING) publish $(PUBLISH_CHECK_FLAGS) --workspace-root $(CURDIR)

help: ## Show available targets
	@grep -E '^[a-zA-Z_-]+:.*?##' $(MAKEFILE_LIST) | \
	awk 'BEGIN {FS=":"; printf "Available targets:\n"} {printf "  %-20s %s\n", $$1, $$2}'
