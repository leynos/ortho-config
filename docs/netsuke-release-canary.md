# Netsuke v0.1.0 release-admission canary

This branch runs a selected mixed Rust, Python, Markdown, and generated
configuration slice through the exact Netsuke candidate. Linux runs the full
lint action, while the normal `windows-latest` leg runs formatting, Clippy,
mixed tests, and the PowerShell wrapper validation.

The explicit empty `targets: []` remains because v0.1.0 requires the top-level
key even when the selected canary graph consists entirely of actions.

The Netsukefile intentionally uses direct commands for the selected gates. It
does not turn existing Python generators or PowerShell tooling into embedded
shell scripts: those focused helpers remain their own maintained boundaries.
The Makefile remains for the broader contributor and publication workflow.
