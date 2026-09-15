.PHONY: help all clean test build release lint lint-clippy lint-whitaker fmt check-fmt markdownlint spellcheck nixie typecheck python-test-deps publish-check powershell-wrapper-validate test-workflow-contracts FORCE

CRATE ?= ortho_config
CARGO ?= cargo
WHITAKER ?= whitaker
PUBLISH_CHECK_CARGO_REAL ?= $(shell command -v $(CARGO))
BUILD_JOBS ?=
CLIPPY_FLAGS ?= --all-targets --all-features -- -D warnings
MDLINT ?= markdownlint-cli2
# `make fmt` and `make check-fmt` call mdtablefix directly. `--git` selects the
# Markdown files Git tracks and `--include-untracked` adds the untracked files
# Git does not ignore, so a new document is formatted before it is staged.
# Both modes need mdtablefix 0.6.0 or later; CI pins the version at the
# install-mdtablefix step.
MDTABLEFIX ?= mdtablefix
MDTABLEFIX_SELECT = --git --include-untracked
MDTABLEFIX_RULES = --wrap --renumber --breaks --ellipsis --fences
NIXIE ?= nixie
UV_ENV = UV_CACHE_DIR=.uv-cache UV_TOOL_DIR=.uv-tools
# Single source of truth for the spelling gate. The builder pins the typos
# binary and owns the shared dictionary, so CI consumes it through the
# spellcheck target and the Makefile and CI cannot drift apart.
TYPOS_CONFIG_BUILDER_VERSION ?= v0.1.1
TYPOS_CONFIG_BUILDER = $(UV_ENV) $(UV) tool run --from \
	"git+https://github.com/leynos/typos-config-builder.git@$(TYPOS_CONFIG_BUILDER_VERSION)" \
	typos-config-builder
PUBLISH_CHECK_FLAGS ?=
PYTHON_VENV ?= scripts/.venv
UV ?= uv
PYTHON_VERSION ?= 3.13
PYTHON_DEPS_FILE ?= scripts/requirements-test.txt
PYTEST_FLAGS ?= --doctest-modules scripts/bump_version.py \
	scripts/release_archive.py scripts/release_archive_naming.py \
	scripts/typos_rollout_http.py \
	scripts/verify_release_archives.py scripts/tests -q
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

define BUILD_LIBRARY_COMMAND
$(CARGO) build $(BUILD_JOBS)                            \
  $(if $(findstring release,$(@)),--release)            \
  --lib
@# Copy artefacts only when the cargo output and make target differ.
src=target/$(if $(findstring release,$(@)),release,debug)/lib$(CRATE).rlib; \
if [ "$$src" != "$@" ]; then \
  install -Dm644 "$$src" "$@"; \
fi
endef

build: target/debug/lib$(CRATE).rlib ## Build debug library
release: target/release/lib$(CRATE).rlib ## Build release library

all: check-fmt typecheck lint test markdownlint nixie

clean: ## Remove build artefacts
	$(CARGO) clean
	rm -rf $(PYTHON_VENV)

test: python-test-deps ## Run tests with warnings treated as errors
	RUSTFLAGS="-D warnings" $(CARGO) test --all-targets --all-features $(BUILD_JOBS)
	$(PYTEST) $(PYTEST_FLAGS)

python-test-deps: ## Ensure Python test dependencies are provisioned
	$(PYTEST) --version > $(NULL_DEVICE)

test-workflow-contracts: ## Validate the mutation-testing caller contract
	$(UV) run --with 'pytest>=8' --with 'pyyaml>=6' --with 'hypothesis>=6' pytest tests/workflow_contracts -q

# will match target/debug/libmy_library.rlib and target/release/libmy_library.rlib
target/%/lib$(CRATE).rlib: FORCE ## Build library in debug or release
	$(BUILD_LIBRARY_COMMAND)

FORCE:

lint: lint-clippy lint-whitaker ## Run Clippy and the Whitaker Dylint suite with warnings denied

lint-clippy: ## Run rustdoc and Clippy with warnings denied
	RUSTDOCFLAGS="$(RUSTDOC_FLAGS)" $(CARGO) doc --workspace --no-deps
	$(CARGO) clippy $(CLIPPY_FLAGS)

lint-whitaker: ## Run the Whitaker Dylint suite with warnings denied
	RUSTFLAGS="-D warnings" $(WHITAKER) --all -- --all-targets --all-features

typecheck: ## Typecheck workspace (cargo check)
	RUSTFLAGS="-D warnings" $(CARGO) check --workspace --all-targets --all-features $(BUILD_JOBS)

fmt: ## Format Rust and Markdown sources
	$(CARGO) fmt --all
	$(MDTABLEFIX) --in-place $(MDTABLEFIX_SELECT) $(MDTABLEFIX_RULES)
	@unset FORCE_COLOR; $(MDLINT) --fix "**/*.md"

check-fmt: ## Verify formatting
	$(CARGO) fmt --all -- --check
	$(MDTABLEFIX) --check $(MDTABLEFIX_SELECT) $(MDTABLEFIX_RULES)

markdownlint: ## Lint Markdown files and enforce en-GB-oxendict spelling
	$(MDLINT) "**/*.md"
	$(MAKE) spellcheck

spellcheck: ## Enforce en-GB-oxendict (Oxford) spelling over Markdown prose
	$(TYPOS_CONFIG_BUILDER) gate --repository .

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
