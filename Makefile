.PHONY: help all clean test boundary build release lint lint-clippy lint-whitaker docstring-coverage fmt check-fmt markdownlint spellcheck spelling nixie typecheck msrv-build python-test-deps publish-check powershell-wrapper-validate test-workflow-contracts install-build-tools check-build-tools FORCE

CRATE ?= ortho_config
CARGO ?= cargo
WHITAKER ?= whitaker
PUBLISH_CHECK_CARGO_REAL ?= $(shell command -v $(CARGO))
BUILD_JOBS ?=
# The development build standard: the parallel `rustc` frontend and the `mold`
# linker are the defaults for development, test, lint and typecheck builds.
# `.cargo/config.toml` carries them, so a bare `cargo` invocation gets them
# too; this block restates them where a recipe assigns `RUSTFLAGS`.
#
# Why restate: Cargo selects a single `rustflags` source rather than merging
# them, and an assigned `RUSTFLAGS` outranks every `rustflags` table in the
# configuration. A recipe that assigned only `-D warnings` would therefore
# silently build without the standard and still report success.
# tests/workflow_contracts/build_standard_routing_test.py holds these variables
# equal to the configuration file for each host it names; change one without
# the other and that contract fails.
#
# Release and packaging builds, coverage, and the Whitaker suite are excluded
# and say so where they assign `RUSTFLAGS` below. `BUILD_STANDARD=0` turns the
# standard off for one invocation, for a toolchain that is not the pinned
# nightly (the advisory `clippy-beta` job): the parallel frontend is
# nightly-only, so a stable or beta compiler refuses it outright.
BUILD_STANDARD ?= 1
STANDARD_THREADS_FLAG ?= -Zthreads=8
STANDARD_MOLD_FLAG ?= -Clink-arg=-fuse-ld=mold
MOLD_VERSION_FILE ?= tools/mold/VERSION
MOLD_SHA256SUMS_FILE ?= tools/mold/SHA256SUMS
BUILD_TOOLS_PREFIX ?= $(HOME)/.local
# Exported rather than interpolated into the recipes. Make hands an exported
# variable to the child process directly, so a path containing a quote cannot
# break the command line the shell parses.
export MOLD_VERSION_FILE MOLD_SHA256SUMS_FILE BUILD_TOOLS_PREFIX
# `mold` ships for Linux only. macOS and Windows keep the platform linker,
# matching the `[target.*]` table in `.cargo/config.toml`. `OS` is set by
# Windows and by no other host. The Cranelift backend is not part of the
# standard (docs/developers-guide.md, "Cranelift qualification").
ifeq ($(OS),Windows_NT)
BUILD_HOST_OS ?= Windows_NT
else
BUILD_HOST_OS ?= $(shell uname -s)
endif
STANDARD_RUSTFLAGS = $(if $(filter 1,$(BUILD_STANDARD)),$(STANDARD_THREADS_FLAG)$(if $(filter Linux,$(BUILD_HOST_OS)), $(STANDARD_MOLD_FLAG)))
# Warnings denied, plus the standard. Every gate recipe that runs Cargo uses it.
GATE_RUSTFLAGS = RUSTFLAGS="-D warnings $(STANDARD_RUSTFLAGS)"
# A debug build keeps the caller's warning policy rather than imposing one.
DEBUG_RUSTFLAGS = RUSTFLAGS="$(STANDARD_RUSTFLAGS)"
# Release builds take none of the standard. Assigning `RUSTFLAGS` at all, even
# to the empty string, displaces the configuration file's `rustflags` tables,
# which is the whole mechanism.
RELEASE_RUSTFLAGS = RUSTFLAGS=""
# The MSRV is read from the manifest, as major.minor, so the build cannot drift
# from the promise. CI's matrix leg names the same major.minor and passes it in
# (MSRV=1.89), and a contract holds the two equal. `?=` is lazily expanded, so
# the shell only runs for the target that needs it.
MSRV ?= $(shell awk -F'"' '/^rust-version[[:space:]]*=/ { print $$2; exit }' Cargo.toml | cut -d. -f1,2)
# The build-tools install prefix leads PATH: `-fuse-ld=mold` resolves by PATH
# order, so the pinned release must outrank any distribution `mold`. Windows
# separates PATH entries with `;`, so the prefix is not prepended there.
ifneq ($(OS),Windows_NT)
export PATH := $(BUILD_TOOLS_PREFIX)/bin:$(PATH)
endif
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
$(if $(findstring release,$(@)),$(RELEASE_RUSTFLAGS),$(DEBUG_RUSTFLAGS)) \
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

test: check-build-tools python-test-deps ## Run tests with warnings treated as errors
	$(GATE_RUSTFLAGS) $(CARGO) test --all-targets --all-features $(BUILD_JOBS)
	$(PYTEST) $(PYTEST_FLAGS)

# The boundary contracts are the evidence that an application depending on the
# localization runtime alone never resolves the configuration facade, the derive
# macros, Figment, or Clap. They build and test foreign package roots under
# `target/localization-boundary`, so they cost minutes rather than seconds and
# are worth a target of their own: a boundary failure should be identifiable
# without re-running the whole workspace.
#
# This is a focused entry point, not a second copy of the claim. `make test`
# already runs it, because `--all-targets` reaches the declared `[[test]]
# boundary` target, and CI's coverage lane reaches it through nextest's default
# filter. `--offline` matches the fixtures: resolution that silently fetched
# from crates.io would prove something about the network instead of the shared
# package cache. The fixture target is deliberately kept between runs, so a
# warm run is not evidence of a cold download.
boundary: ## Prove the standalone runtime boundary against foreign consumer roots
	RUSTFLAGS="-D warnings" $(CARGO) test -p ortho_l10n --test boundary --offline $(BUILD_JOBS)

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

# Only the debug build uses the standard, so only it needs the standard's tools.
# A release build must not fail for want of a linker it will not use.
target/debug/lib$(CRATE).rlib: | check-build-tools

FORCE:

lint: lint-clippy lint-whitaker docstring-coverage ## Run linters with warnings denied

lint-clippy: check-build-tools ## Run rustdoc and Clippy with warnings denied
	$(GATE_RUSTFLAGS) RUSTDOCFLAGS="$(RUSTDOC_FLAGS)" $(CARGO) doc --workspace --no-deps
	$(GATE_RUSTFLAGS) $(CARGO) clippy $(CLIPPY_FLAGS)

# Whitaker runs on the toolchain its installer manages, not on this
# repository's, so it takes none of the development standard: the parallel
# frontend and `mold` would be applied to a different compiler, and `mold` is
# not a prerequisite it can rely on. The assignment is deliberately
# `-D warnings` alone, and deliberately not GATE_RUSTFLAGS.
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

typecheck: check-build-tools ## Typecheck workspace (cargo check)
	$(GATE_RUSTFLAGS) $(CARGO) check --workspace --all-targets --all-features $(BUILD_JOBS)

# Builds every target on the declared MSRV, and runs nothing: a build proves the
# code compiles and links for the oldest compiler the crates promise to support,
# and the `build-test` job has already established behaviour on the pinned
# toolchain. It is stronger than the estate's `cargo check --all-targets` cost
# rule and still no second test run (a deliberate choice, recorded in the
# developers' guide). The MSRV is a stable toolchain, which refuses the
# standard's nightly-only flags, so `RUSTFLAGS` is assigned to displace the
# configuration's `rustflags` tables rather than inherit them.
msrv-build: ## Build every target on the declared MSRV toolchain (no tests)
	RUSTFLAGS="-D warnings" $(CARGO) +$(MSRV) build --workspace --all-targets --all-features $(BUILD_JOBS)

install-build-tools: ## Install the pinned mold linker and the pinned toolchain
	@scripts/install-build-tools.sh

check-build-tools: ## Check the mold linker and toolchain prerequisites
ifeq ($(BUILD_STANDARD),1)
	@scripts/check-build-tools.sh
else
	@echo 'build-tools: BUILD_STANDARD=$(BUILD_STANDARD); the build standard is off, nothing to check'
endif

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
