.PHONY: help all clean test build release lint fmt check-fmt markdownlint nixie check python-test-deps

CRATE ?= ortho-config
CARGO ?= cargo
BUILD_JOBS ?=
CLIPPY_FLAGS ?= --all-targets --all-features -- -D warnings
MDLINT ?= markdownlint
NIXIE ?= nixie
PYTHON_VENV ?= scripts/.venv
UV ?= uv
PYTHON_VERSION ?= 3.13
PYTHON_DEPS_FILE ?= scripts/requirements-test.txt
PYTEST_FLAGS ?= --doctest-modules scripts/bump_version.py scripts/tests -q
ifeq ($(OS),Windows_NT)
NULL_DEVICE ?= NUL
else
NULL_DEVICE ?= /dev/null
endif
UV_RUN := $(UV) run --python $(PYTHON_VERSION) --with-requirements $(PYTHON_DEPS_FILE)
PYTEST ?= $(UV_RUN) --module pytest

build: target/debug/lib$(CRATE) ## Build debug binary
release: target/release/lib$(CRATE) ## Build release binary

all: release ## Default target builds release binary

clean: ## Remove build artifacts
	$(CARGO) clean
	rm -rf $(PYTHON_VENV)

test: python-test-deps ## Run tests with warnings treated as errors
	RUSTFLAGS="-D warnings" $(CARGO) test --all-targets --all-features $(BUILD_JOBS)
	$(PYTEST) $(PYTEST_FLAGS)

python-test-deps: ## Ensure Python test dependencies are provisioned
	$(PYTEST) --version > $(NULL_DEVICE)

# will match target/debug/libmy_library.rlib and target/release/libmy_library.rlib
target/%/lib$(CRATE).rlib: ## Build library in debug or release
	$(CARGO) build $(BUILD_JOBS)                            \
	  $(if $(findstring release,$(@)),--release)            \
	  --lib
	@# copy the .rlib into your own target tree
	install -Dm644                                           \
	  target/$(if $(findstring release,$(@)),release,debug)/lib$(CRATE).rlib \
	  $@

lint: ## Run Clippy with warnings denied
	$(CARGO) clippy $(CLIPPY_FLAGS)

fmt: ## Format Rust and Markdown sources
	$(CARGO) fmt --all
	mdformat-all

check-fmt: ## Verify formatting
	$(CARGO) fmt --all -- --check

check: check-fmt lint test markdownlint nixie

markdownlint: ## Lint Markdown files
	find . -type f -name '*.md' -not -path './target/*' -print0 | xargs -0 -- $(MDLINT)

nixie:
	# CI currently requires --no-sandbox; remove once nixie supports
	# environment variable control for this option
	nixie --no-sandbox

help: ## Show available targets
	@grep -E '^[a-zA-Z_-]+:.*?##' $(MAKEFILE_LIST) | \
	awk 'BEGIN {FS=":"; printf "Available targets:\n"} {printf "  %-20s %s\n", $$1, $$2}'
