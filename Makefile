.PHONY: help all clean test build release lint lint-clippy lint-whitaker docstring-coverage fmt check-fmt markdownlint spellcheck spelling nixie typecheck python-test-deps publish-check powershell-wrapper-validate test-workflow-contracts FORCE

CRATE ?= ortho_config
CARGO ?= cargo
WHITAKER ?= whitaker
PUBLISH_CHECK_CARGO_REAL ?= $(shell command -v $(CARGO))
BUILD_JOBS ?=
# One lint invocation for every caller: the pinned-toolchain `build-test`
# job and the advisory `clippy-beta` job both run it through `lint-clippy`,
# so CI states its strictness once, here, rather than in two workflow steps
# that can drift apart.
CLIPPY_FLAGS ?= --workspace --all-targets --all-features -- -D warnings
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
# The CV-005 CodeScene contracts live in shared-actions and run from a full
# commit, so a fix is a pin bump. `.github/cv005.toml` holds this repository's
# only parameters.
CV005_CONTRACTS_REF ?= 2f58fab140cc8dfe69756aaa6a3bbb987c426023
CV005_CONTRACTS = $(UV_ENV) $(UV) tool run --python 3.13 \
	--from 'git+https://github.com/leynos/shared-actions@$(CV005_CONTRACTS_REF)\#subdirectory=packages/cv005-contracts' \
	cv005-contracts
# Single source of truth for the spelling gate. The builder pins the typos
# binary and owns the shared dictionary, so CI consumes it through the
# spellcheck target and the Makefile and CI cannot drift apart.
TYPOS_CONFIG_BUILDER_VERSION ?= v0.1.3
TYPOS_CONFIG_BUILDER = $(UV_ENV) $(UV) tool run --from \
	"git+https://github.com/leynos/typos-config-builder.git@$(TYPOS_CONFIG_BUILDER_VERSION)" \
	typos-config-builder
PUBLISH_CHECK_FLAGS ?=
PYTHON_VENV ?= scripts/.venv
UV ?= uv
PYTHON_VERSION ?= 3.13
PYTHON_DEPS_FILE ?= scripts/requirements-test.txt
GIT ?= git
PYTEST_FLAGS ?= --doctest-modules scripts/bump_version.py scripts/discard_build_trees.py \
	scripts/release_archive.py scripts/release_archive_naming.py \
	scripts/verify_release_archives.py scripts/tests -q
# Pinned rather than tracking the default branch: the publish step depends
# on the pre-flight skip, which v0.3.1 is the first release to carry
# (leynos/lading#261). An unpinned `uvx --from git+...` would also change
# what the release gate runs without anyone editing this repository.
LADING_REF ?= b771852411b428ff6fc779cc3a68b153a8f2439a
LADING ?= uvx --from git+https://github.com/leynos/lading@$(LADING_REF) lading
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

# --doctest-modules so the examples in the contract helpers are executed
# rather than displayed: they document how a Makefile recipe is parsed, and
# an example that has drifted from the parser is worse than none.
#
# Upper bounds as well as lower: this target bypasses
# scripts/requirements-test.txt and has no lockfile, so an unbounded
# requirement lets a future major release change collection or doctest
# behaviour with no edit to this repository. hypothesis arrived with the
# coverage-ceiling contract (#482) and is bounded for the same reason.
test-workflow-contracts: ## Validate the workflow caller contracts, including the shared CV-005 shape
	$(CV005_CONTRACTS) check --repository .
	$(UV) run --with 'pytest>=8,<10' --with 'pyyaml>=6,<7' \
		--with 'hypothesis>=6,<7' pytest \
		tests/workflow_contracts \
		--doctest-modules -q

# will match target/debug/libmy_library.rlib and target/release/libmy_library.rlib
target/%/lib$(CRATE).rlib: FORCE ## Build library in debug or release
	$(BUILD_LIBRARY_COMMAND)

FORCE:

lint: lint-clippy lint-whitaker docstring-coverage ## Run linters with warnings denied

lint-clippy: ## Run rustdoc and Clippy with warnings denied
	RUSTDOCFLAGS="$(RUSTDOC_FLAGS)" $(CARGO) doc --workspace --no-deps
	$(CARGO) clippy $(CLIPPY_FLAGS)

lint-whitaker: ## Run the Whitaker Dylint suite with warnings denied
	RUSTFLAGS="-D warnings" $(WHITAKER) --all -- --all-targets --all-features

# The inventory is discovered inside the recipe and handed over NUL-separated.
# `$(shell ...)` splits on whitespace and has no way to quote a path, so a file
# named `local helper.py` would reach interrogate as two arguments; `xargs -0`
# execs the command directly instead of re-parsing. `xargs` may still split a
# large inventory into batches.
#
# `--fail-under 100` alone is not enough: interrogate rounds the coverage
# percentage to the precision of the threshold before comparing it, so a run
# missing one docstring in 2000 reports `100.0%` and passes. The gate therefore
# also reads the summary's TOTAL row and fails on any `Miss`. That column is an
# exact count and does not depend on rounding. Every TOTAL row is summed, since
# a batched run reports one per batch and the first batches may include the
# missing item while the last one does not.
#
# Discovery writes to a file rather than straight into the pipe: a pipeline
# reports the status of its last command, so a failing `git ls-files` would
# otherwise let an empty batch pass the gate. `set -e` aborts the recipe on a
# discovery failure, and `--` stops a leading `-` in a filename being read as
# an option.
#
# An empty inventory is a pass, and `interrogate` is not invoked at all for it:
# that is what keeps the TOTAL check below from misreading "nothing was
# scanned" as "the summary is missing". The report is kept rather than streamed
# so the TOTAL row can be read after interrogate finishes, and a failing run
# still prints its output before the recipe exits. `xargs` runs from inside this
# shell so the workflow contracts can override `UV_RUN` without a recursive make.
docstring-coverage: ## Require complete docstrings for every Python source file
	set -eu; \
	inventory=$$(mktemp); \
	report=$$(mktemp); \
	trap 'rm -f "$$inventory" "$$report"' EXIT; \
	$(GIT) ls-files -z --cached --others --exclude-standard -- '*.py' '*.pyi' \
		> "$$inventory"; \
	if [ -s "$$inventory" ]; then \
		status=0; \
		xargs -0 $(UV_RUN) interrogate -v --fail-under 100 --ext pyi \
			--no-color -- < "$$inventory" > "$$report" || status=$$?; \
		cat "$$report"; \
		if [ "$$status" -ne 0 ]; then exit "$$status"; fi; \
		awk -F'|' '$$2 ~ /TOTAL/ { \
			column = $$4; \
			gsub(/ /, "", column); \
			missing += column; \
			found = 1; \
		} END { \
			if (!found) { \
				print "docstring-coverage: no TOTAL row in interrogate output" \
					> "/dev/stderr"; \
				exit 1; \
			} \
			if (missing != 0) { \
				printf "docstring-coverage: %s undocumented item(s)\n", \
					missing > "/dev/stderr"; \
				exit 1; \
			} \
		}' "$$report"; \
	fi

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

# The shared AGENTS.md spelling block tells agents to run `make spelling`.
spelling: spellcheck ## Alias for spellcheck

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
