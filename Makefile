.PHONY: help all clean test build release lint lint-clippy lint-whitaker fmt check-fmt markdownlint spellcheck spelling-config spelling-phrase-check spelling-helper-test nixie typecheck python-test-deps publish-check powershell-wrapper-validate test-workflow-contracts FORCE

CRATE ?= ortho_config
CARGO ?= cargo
WHITAKER ?= whitaker
PUBLISH_CHECK_CARGO_REAL ?= $(shell command -v $(CARGO))
BUILD_JOBS ?=
CLIPPY_FLAGS ?= --all-targets --all-features -- -D warnings
MDLINT ?= markdownlint-cli2
NIXIE ?= nixie
# Single source of truth for the typos version; CI consumes it through the
# spellcheck target, so the Makefile and CI cannot drift apart.
TYPOS_VERSION ?= 1.48.0
RUFF_VERSION ?= 0.15.12
UV_ENV = UV_CACHE_DIR=.uv-cache UV_TOOL_DIR=.uv-tools
# Markdown file list shared by the spelling gate. markdownlint-cli2 does its
# own globbing via .markdownlint-cli2.jsonc; typos takes an explicit list.
MD_FILES_FIND ?= find . -type f -name '*.md' -not -path './target/*' -not -path './$(PYTHON_VENV)/*' -not -path './.git/*' -not -path './.memdb/*' -print0
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
SPELLING_PY_SRCS := \
	scripts/generate_typos_config.py scripts/typos_rollout_check.py \
	scripts/typos_rollout.py scripts/typos_rollout_cache.py \
	scripts/typos_rollout_http.py scripts/tests/test_generate_typos_config.py \
	scripts/tests/test_typos_rollout.py scripts/tests/test_typos_rollout_check.py \
	scripts/tests/test_typos_rollout_hardening.py \
	scripts/tests/test_typos_rollout_refresh.py scripts/tests/conftest.py \
	scripts/tests/typos_rollout_test_support.py
SPELLING_PY_TESTS := \
	scripts/tests/test_generate_typos_config.py scripts/tests/test_typos_rollout*.py
SPELLING_COVERAGE_ARGS := \
	--cov=generate_typos_config --cov=typos_rollout_check --cov=typos_rollout \
	--cov=typos_rollout_cache --cov=typos_rollout_http --cov-fail-under=90
SPELLING_HELPER_PYTEST = PYTHONPATH=scripts $(UV_ENV) $(UV) run --no-project \
	--python 3.13 --with pytest==9.1.1 --with pytest-cov==7.0.0 python -m pytest

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
	$(UV) run --with 'pytest>=8' --with 'pyyaml>=6' pytest tests/workflow_contracts -q

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
	mdformat-all

check-fmt: ## Verify formatting
	$(CARGO) fmt --all -- --check

markdownlint: ## Lint Markdown files and enforce en-GB-oxendict spelling
	$(MDLINT) "**/*.md"
	$(MAKE) spellcheck

spellcheck: spelling-phrase-check ## Enforce en-GB-oxendict (Oxford) spelling over Markdown prose
	@$(MD_FILES_FIND) | xargs -0 -r env $(UV_ENV) \
		$(UV) tool run typos@$(TYPOS_VERSION) --config typos.toml --force-exclude

spelling-phrase-check: spelling-config ## Reject prohibited spelling phrases
	@PYTHONPATH=scripts $(UV_ENV) $(UV) run --no-project --python 3.13 scripts/typos_rollout_check.py --repository .

spelling-config: spelling-helper-test ## Generate and verify the spelling configuration
	@$(UV_ENV) $(UV) run scripts/generate_typos_config.py
	@git ls-files --error-unmatch typos.toml >/dev/null
	@git diff --exit-code -- typos.toml

spelling-helper-test: ## Validate the shared spelling-policy integration
	@$(UV_ENV) $(UV) tool run ruff@$(RUFF_VERSION) format --isolated --target-version py313 --check $(SPELLING_PY_SRCS)
	@$(UV_ENV) $(UV) tool run ruff@$(RUFF_VERSION) check --isolated --target-version py313 $(SPELLING_PY_SRCS)
	@$(SPELLING_HELPER_PYTEST) $(SPELLING_PY_TESTS) -c /dev/null --rootdir=. -p no:cacheprovider $(SPELLING_COVERAGE_ARGS)

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
