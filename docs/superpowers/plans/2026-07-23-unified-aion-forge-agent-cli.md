# Unified Aion Forge Agent CLI Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Ship `aion-forge` as the standard PATH-discoverable ACP/CLI entry point while retaining `aion-forge-cli` as a compatible alias.

**Architecture:** Convert the ACP adapter into a reusable Rust library and call it in-process from the unified CLI. Build two CLI binary names from one shared entry function, then update release artifacts, installers, MCP setup defaults, documentation, and AionUI verification.

**Tech Stack:** Rust 2021, Tokio, Clap, JSON-RPC over stdio, Cargo integration tests, GitHub Actions.

---

### Task 1: Expose ACP as reusable Rust library

**Files:**
- Create: `aion-forge-acp/src/lib.rs`
- Modify: `aion-forge-acp/src/main.rs`
- Test: `aion-forge-acp/tests/cli_isolation.rs`

- [ ] **Step 1: Write the failing library contract**

Add a compile-time integration test importing `aion_forge_acp::run_acp_server` and keep the existing JSON-RPC initialize/shutdown test.

```rust
use aion_forge_acp::run_acp_server;

#[test]
fn acp_server_is_exposed_as_library_api() {
    let _ = run_acp_server;
}
```

- [ ] **Step 2: Verify the new contract fails**

Run: `rtk cargo test -p aion-forge-acp --test cli_isolation acp_server_is_exposed_as_library_api`
Expected: FAIL because package `aion-forge-acp` has no library target.

- [ ] **Step 3: Add the minimal library boundary**

Create `src/lib.rs` with a public documented export and make the binary delegate to it:

```rust
//! Reusable ACP protocol entry point for Aion Forge.

mod acp;

/// Run the ACP JSON-RPC server over stdin and stdout.
pub use acp::run_acp_server;
```

In `src/main.rs`, remove `mod acp;` and call `aion_forge_acp::run_acp_server().await`. Preserve stderr tracing and existing environment initialization.

- [ ] **Step 4: Run focused ACP tests**

Run: `rtk cargo test -p aion-forge-acp --test cli_isolation`
Expected: all ACP isolation and handshake tests PASS.

- [ ] **Step 5: Commit**

Run: `rtk git add aion-forge-acp && rtk git commit -m "refactor(acp): expose reusable protocol server"`

### Task 2: Add the canonical unified CLI and compatibility alias

**Files:**
- Modify: `aion-forge-cli/Cargo.toml`
- Modify: `aion-forge-cli/src/cli.rs`
- Modify: `aion-forge-cli/src/lib.rs`
- Modify: `aion-forge-cli/src/main.rs`
- Create: `aion-forge-cli/src/bin/aion-forge-cli.rs`
- Test: `aion-forge-cli/tests/cli_contract.rs`
- Test: `aion-forge-cli/tests/acp_contract.rs`

- [ ] **Step 1: Write failing parser and binary contracts**

Change the parser contract to require `Commands::Acp`, use `aion-forge` as the canonical invocation, and add a process test using `env!("CARGO_BIN_EXE_aion-forge")`.

```rust
let acp = Cli::try_parse_from(["aion-forge", "acp"]).expect("acp should parse");
assert!(matches!(acp.command, Some(Commands::Acp)));
```

Add an ACP process test that sends initialize and shutdown JSON lines and asserts every stdout line parses as JSON and the process exits successfully.

- [ ] **Step 2: Verify the contracts fail**

Run: `rtk cargo test -p aion-forge-cli --test cli_contract --test acp_contract`
Expected: FAIL because `Acp` and `CARGO_BIN_EXE_aion-forge` do not exist.

- [ ] **Step 3: Implement one shared Rust entry function**

Add `aion-forge-acp = { path = "../aion-forge-acp" }`. Define explicit Cargo binaries named `aion-forge` and `aion-forge-cli`. Add `Commands::Acp`; dispatch it with:

```rust
Some(cli::Commands::Acp) => {
    aion_forge_acp::run_acp_server().await?;
    Ok(None)
}
```

Move executable initialization and result rendering into a documented `aion_forge_cli::main_entry()` function. Both binary files contain only a Tokio main that calls this shared function. Keep all tracing on stderr.

- [ ] **Step 4: Run unified CLI contracts**

Run: `rtk cargo test -p aion-forge-cli --test cli_contract --test acp_contract --test mcp_contract --test direct_contract`
Expected: all selected tests PASS and ACP/MCP stdout contains JSON only.

- [ ] **Step 5: Commit**

Run: `rtk git add aion-forge-cli && rtk git commit -m "feat(cli): unify Forge ACP and CLI entrypoints"`

### Task 3: Make generated MCP configuration canonical

**Files:**
- Modify: `aion-forge-cli/src/setup.rs`
- Modify: `aion-forge-cli/tests/setup_contract.rs`

- [ ] **Step 1: Write the failing canonical setup contract**

Update the dry-run and helper-input tests to pass `D:\\tools\\aion-forge.exe` and assert command `aion-forge.exe` with argument `mcp-server`. Add a test ensuring a legacy registered server is updated rather than duplicated.

- [ ] **Step 2: Verify it fails**

Run: `rtk cargo test -p aion-forge-cli --test setup_contract`
Expected: FAIL while setup still preserves the compatibility executable path.

- [ ] **Step 3: Resolve the canonical executable path**

Add a small function that replaces the current compatibility filename with the platform-specific canonical sibling before building AionUI MCP input. Keep `setup --dry-run` deterministic and preserve environment redaction.

- [ ] **Step 4: Run setup contracts**

Run: `rtk cargo test -p aion-forge-cli --test setup_contract`
Expected: all setup contracts PASS.

- [ ] **Step 5: Commit**

Run: `rtk git add aion-forge-cli/src/setup.rs aion-forge-cli/tests/setup_contract.rs && rtk git commit -m "fix(setup): register canonical Forge command"`

### Task 4: Publish both command names from the same build

**Files:**
- Modify: `.github/workflows/release.yml`
- Modify: `scripts/build/package.sh`
- Modify: `scripts/build/package-local.sh`
- Modify: `scripts/build/update-checksums.sh`
- Modify: `safety-manifest.json`
- Test: `aion-forge-cli/tests/release_contract.rs`

- [ ] **Step 1: Write failing release text contracts**

Add a Rust integration test that reads the workflow, package scripts, and safety manifest using `include_str!` and asserts canonical artifacts exist for Windows, Linux, macOS x86_64, and macOS aarch64, while compatibility artifacts remain present.

- [ ] **Step 2: Verify it fails**

Run: `rtk cargo test -p aion-forge-cli --test release_contract`
Expected: FAIL because release metadata only names `aion-forge-cli`.

- [ ] **Step 3: Update release and package mappings**

Build `-p aion-forge-cli --bin aion-forge --bin aion-forge-cli`. Publish canonical `aion-forge-*` artifacts and compatibility `aion-forge-cli-*` artifacts from the corresponding Cargo binaries. Package both executable names and add checksum entries for both binary keys.

- [ ] **Step 4: Run release contracts**

Run: `rtk cargo test -p aion-forge-cli --test release_contract`
Expected: PASS for all platform artifact and manifest assertions.

- [ ] **Step 5: Commit**

Run: `rtk git add .github/workflows/release.yml scripts/build safety-manifest.json aion-forge-cli/tests/release_contract.rs && rtk git commit -m "build: publish canonical Forge agent CLI"`

### Task 5: Install the canonical command and preserve the alias

**Files:**
- Modify: `scripts/install/install.ps1`
- Modify: `scripts/install/install.sh`
- Modify: `README.md`
- Modify: `docs/guide/AIONUI_INSTALL.md`
- Test: `aion-forge-cli/tests/install_contract.rs`

- [ ] **Step 1: Write failing installer contracts**

Add Rust text-contract tests asserting both installers download or install `aion-forge`, retain `aion-forge-cli`, verify canonical `--version`, and show `aion-forge acp` plus `aion-forge mcp-server` examples.

- [ ] **Step 2: Verify it fails**

Run: `rtk cargo test -p aion-forge-cli --test install_contract`
Expected: FAIL because installers only install and verify `aion-forge-cli`.

- [ ] **Step 3: Update installers and documentation**

Install the canonical artifact as `aion-forge` or `aion-forge.exe`, install the compatibility artifact beside it, verify both canonical help and version behavior, and use canonical commands in all new examples. Keep existing user PATH handling unchanged.

- [ ] **Step 4: Run installer contracts**

Run: `rtk cargo test -p aion-forge-cli --test install_contract`
Expected: PASS.

- [ ] **Step 5: Commit**

Run: `rtk git add scripts/install README.md docs/guide/AIONUI_INSTALL.md aion-forge-cli/tests/install_contract.rs && rtk git commit -m "fix(install): install discoverable Forge agent"`

### Task 6: Build, install locally, and verify AionUI discovery

**Files:**
- Modify only if a failing contract identifies a production defect.

- [ ] **Step 1: Run Forge code quality tools**

Use Forge `code_lint` on all changed Rust files and Forge `code_test` for the focused package tests. Resolve only findings tied to this design.

- [ ] **Step 2: Run Rust verification**

Run: `rtk cargo fmt --all -- --check`
Expected: exit 0.

Run: `rtk cargo clippy -p aion-forge-cli -p aion-forge-acp --all-targets -- -D warnings`
Expected: exit 0 with no warnings.

Run: `rtk cargo test -p aion-forge-cli -p aion-forge-acp`
Expected: all tests PASS.

- [ ] **Step 3: Build release binaries**

Run: `rtk cargo build --release -p aion-forge-cli --bin aion-forge --bin aion-forge-cli`
Expected: `target/release/aion-forge.exe` and `target/release/aion-forge-cli.exe` exist on Windows.

- [ ] **Step 4: Install into the existing user PATH directory**

Use the project installer or copy the verified release artifacts to the installer-managed Aion Forge bin directory, preserving both names. Do not create an AionUI command override.

- [ ] **Step 5: Verify CLI and protocol behavior**

Run canonical `--version` and `--help` with a bounded timeout. Send ACP initialize/shutdown messages and assert JSON-only stdout. Repeat the existing MCP initialization contract.

- [ ] **Step 6: Verify AionUI catalog state**

Use the supported AionUI helper `config agents list` and confirm the built-in `Aion Forge` entry reports `installed: true` with `has_command_override: false`. Then use the platform connection test and confirm online status.

- [ ] **Step 7: Record session and commit any verification-driven fix**

Call Forge `record_change`, `record_decision`, `session_report`, and `memory_remember`. If verification required production changes, commit them with a focused message.
