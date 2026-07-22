# Aion Forge Entrypoint Separation Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Restore `aion-forge-cli` as the only Aion Forge CLI/MCP entrypoint, isolate ACP, and preserve the unrelated AionUI `aion-cli` as a sibling project.

**Architecture:** `aion-forge-cli` owns direct builtin execution, the 2026-07-17 CLI flags, MCP stdio, and supported setup. `aion-forge-acp` owns only ACP. The coordinator moves the unrelated `aion-cli` to `D:/test/aionui/aion-cli`, owns workspace integration, and removes every active Forge dependency on that name after end-to-end verification.

**Tech Stack:** Rust 2021, Tokio, Clap 4, Serde JSON, Aion Router, MCP JSON-RPC over stdio, Cargo workspace.

---

## File ownership for parallel execution

| Owner | Exclusive paths |
|---|---|
| Forge worker A | `aion-forge-cli/**` |
| Forge worker B | `aion-forge-acp/**` |
| Forge worker C | `.github/workflows/release.yml`, `Dockerfile`, `README.md`, `CHANGELOG.md`, `HANDOFF.md`, `docs/guide/**`, `scripts/build/**`, `scripts/install/**`, `safety-manifest.json`, `skill.json`, `.mcp.json*`, `scripts/assistants/**` |
| Coordinator | root `Cargo.toml`, `Cargo.lock`, `aion-cli/**`, `aion-cli-gen/**`, external directories, deployed executables, AionUI runtime configuration, final integration |

Workers must not edit files outside their exclusive paths. The coordinator resolves all cross-cutting references after worker delivery.

### Task 1: Preserve the unrelated AionUI CLI before Forge changes

**Files:**
- Source: `aion-cli/**`
- Create outside Forge: `D:/test/aionui/aion-cli/**`
- Evidence: `D:/Temp/AionForgeEntrypointMigration-20260722/`

- [ ] **Step 1: Capture the source inventory**

Use a read-only recursive inventory of `D:/test/aionui/forge/aion-cli`, excluding build outputs and `.git.bak` object files only when computing content equivalence. Record relative path, byte length, and SHA-256 under the D-drive evidence directory.

- [ ] **Step 2: Copy without deleting the source**

Create `D:/test/aionui/aion-cli` and copy the complete directory. Do not overwrite an existing non-identical target; if one exists, stop and compare it first.

- [ ] **Step 3: Verify preservation**

Generate the same inventory for the target. Expected result: identical relative files, lengths, and SHA-256 values. Verify `Cargo.toml`, `src/main.rs`, `src/mcp.rs`, `src/adapter_gen.rs`, and `.git.bak` are present.

- [ ] **Step 4: Record the migration checkpoint**

Do not remove `aion-cli` from Forge yet. The source remains intact until Tasks 2–7 pass.

### Task 2: Create the Forge CLI identity contract — worker A

**Files:**
- Create: `aion-forge-cli/Cargo.toml`
- Create: `aion-forge-cli/src/lib.rs`
- Create: `aion-forge-cli/src/cli.rs`
- Create: `aion-forge-cli/tests/cli_contract.rs`

- [ ] **Step 1: Add test scaffolding without production implementation**

Create the package manifest with package and binary name `aion-forge-cli`, version `0.7.0`, edition `2021`, and the existing workspace dependencies required by the former Forge CLI. Create `src/lib.rs` containing only `pub mod cli;`. Add this contract test before creating `src/cli.rs`:

```rust
use aion_forge_cli::{Cli, Commands};
use clap::Parser;

#[test]
fn preserves_the_july_17_direct_tool_flags() {
    let cli = Cli::try_parse_from([
        "aion-forge-cli",
        "--tool",
        "echo",
        "--params",
        r#"{"text":"hello"}"#,
        "--quiet",
    ])
    .expect("the Forge CLI flags must parse");

    assert_eq!(cli.tool.as_deref(), Some("echo"));
    assert_eq!(cli.params.as_deref(), Some(r#"{"text":"hello"}"#));
    assert!(cli.quiet);
}

#[test]
fn exposes_forge_mcp_and_setup_but_not_acp() {
    let mcp = Cli::try_parse_from(["aion-forge-cli", "mcp-server"])
        .expect("mcp-server must parse");
    assert!(matches!(mcp.command, Some(Commands::McpServer)));

    let setup = Cli::try_parse_from(["aion-forge-cli", "setup", "--dry-run"])
        .expect("setup must parse");
    assert!(matches!(
        setup.command,
        Some(Commands::Setup { dry_run: true })
    ));

    assert!(Cli::try_parse_from(["aion-forge-cli", "acp"]).is_err());
}
```

- [ ] **Step 2: Run RED**

Run `cargo test --manifest-path aion-forge-cli/Cargo.toml --test cli_contract`. Expected: compilation fails because `src/cli.rs`, `Cli`, and `Commands` do not exist.

- [ ] **Step 3: Implement the minimal parser**

Create public `Cli` and `Commands` types in `src/cli.rs`. Preserve the flags `--tool`, `--params`, `--list`, and `--quiet`. Define only `McpServer` and `Setup { dry_run: bool }` subcommands. Use Clap's `Parser` and `Subcommand` derives and the command identity `aion-forge-cli`.

- [ ] **Step 4: Run GREEN**

Run the same targeted test. Expected: both tests pass.

- [ ] **Step 5: Commit worker A's identity slice**

Commit only `aion-forge-cli/Cargo.toml`, `src/lib.rs`, `src/cli.rs`, and `tests/cli_contract.rs` with message `feat(forge-cli): restore Forge command identity`.

### Task 3: Restore direct Forge execution — worker A

**Files:**
- Create: `aion-forge-cli/src/direct.rs`
- Create: `aion-forge-cli/tests/direct_execution.rs`
- Modify: `aion-forge-cli/src/lib.rs`

- [ ] **Step 1: Write the failing direct execution tests**

```rust
use aion_forge_cli::direct::execute_tool;
use serde_json::json;

#[tokio::test]
async fn executes_a_local_forge_builtin() {
    let result = execute_tool("echo", json!({ "text": "hello" }))
        .await
        .expect("echo must execute");

    assert_eq!(result["capability"], "echo");
    assert_eq!(result["echo"], "hello");
}

#[tokio::test]
async fn rejects_an_unknown_tool() {
    let error = execute_tool("not_a_forge_tool", json!({}))
        .await
        .expect_err("unknown tools must fail");

    assert!(error.to_string().contains("not_a_forge_tool"));
}
```

- [ ] **Step 2: Run RED**

Run `cargo test --manifest-path aion-forge-cli/Cargo.toml --test direct_execution`. Expected: compilation fails because `direct` and `execute_tool` are absent.

- [ ] **Step 3: Implement the minimum shared direct executor**

Move the direct builtin execution behavior from the 2026-07-17 Forge CLI/current ACP main into `direct.rs`. The public function signature is:

```rust
pub async fn execute_tool(
    tool_name: &str,
    params: serde_json::Value,
) -> anyhow::Result<serde_json::Value>
```

It must obtain the builtin from `BuiltinRegistry::default_registry()`, construct the existing default-deny `SkillDefinition`, execute it with `ExecutionContext`, and return an error containing the requested name when absent. Input sanitization remains a CLI-boundary concern and is not duplicated in this function.

- [ ] **Step 4: Run GREEN**

Run the targeted test. Expected: two tests pass without network access.

- [ ] **Step 5: Commit worker A's direct execution slice**

Commit message: `feat(forge-cli): restore direct builtin execution`.

### Task 4: Move MCP ownership into Forge CLI — worker A

**Files:**
- Create: `aion-forge-cli/src/mcp.rs`
- Create: `aion-forge-cli/src/main.rs`
- Create: `aion-forge-cli/tests/mcp_stdio.rs`
- Modify: `aion-forge-cli/src/lib.rs`

- [ ] **Step 1: Write the failing MCP process test**

```rust
use std::io::Write;
use std::process::{Command, Stdio};

#[test]
fn serves_forge_identity_and_tools_over_stdio() {
    let mut child = Command::new(env!("CARGO_BIN_EXE_aion-forge-cli"))
        .arg("mcp-server")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("Forge MCP must start");

    let mut stdin = child.stdin.take().expect("stdin must be piped");
    writeln!(
        stdin,
        r#"{"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"2025-03-26","capabilities":{},"clientInfo":{"name":"test","version":"1"}}}"#
    )
    .expect("initialize must be written");
    writeln!(
        stdin,
        r#"{"jsonrpc":"2.0","id":2,"method":"tools/list","params":{}}"#
    )
    .expect("tools/list must be written");
    drop(stdin);

    let output = child.wait_with_output().expect("Forge MCP must exit on EOF");
    assert!(output.status.success());

    let stdout = String::from_utf8(output.stdout).expect("stdout must be UTF-8");
    let responses: Vec<serde_json::Value> = stdout
        .lines()
        .map(|line| serde_json::from_str(line).expect("stdout must contain only JSON-RPC"))
        .collect();

    assert_eq!(responses[0]["result"]["serverInfo"]["name"], "aion-forge");
    assert_eq!(responses[1]["result"]["tools"].as_array().unwrap().len(), 75);
}
```

- [ ] **Step 2: Run RED**

Run `cargo test --manifest-path aion-forge-cli/Cargo.toml --test mcp_stdio`. Expected: the binary target or `mcp-server` implementation is missing.

- [ ] **Step 3: Move the existing MCP implementation**

Copy `aion-cli/src/mcp.rs` into the worker-owned `aion-forge-cli/src/mcp.rs`, preserving protocol behavior. Keep server name `aion-forge`, stderr logging, notification silence, `tools/list`, `tools/call`, async task polling, and MCP-mode timeout behavior. Remove all textual claims that this belongs to `aion-cli` or Claude specifically.

- [ ] **Step 4: Build the Forge main entrypoint**

Create `src/main.rs` that loads the same environment precedence, parses `Cli`, sends all tracing to stderr in MCP mode, dispatches `mcp-server` to `mcp::run_mcp_server`, dispatches direct flags through `direct::execute_tool`, prints `--list` from `CapabilityRegistry`, and delegates `setup` to the setup module added in Task 5. Do not include ACP code or an `acp` command.

- [ ] **Step 5: Run GREEN and regression tests**

Run the MCP process test, CLI contract test, and direct execution test. Expected: all pass; the MCP test parses every stdout line as JSON.

- [ ] **Step 6: Commit worker A's MCP slice**

Commit message: `feat(forge-cli): own the Forge MCP server`.

### Task 5: Implement supported Forge setup — worker A

**Files:**
- Create: `aion-forge-cli/src/setup.rs`
- Create: `aion-forge-cli/tests/setup_contract.rs`
- Modify: `aion-forge-cli/src/lib.rs`
- Modify: `aion-forge-cli/src/main.rs`

- [ ] **Step 1: Write the failing setup contract**

```rust
use aion_forge_cli::setup::mcp_transport;

#[test]
fn setup_targets_the_forge_binary_and_mcp_mode() {
    let transport = mcp_transport("D:/test/aionui/forge/aion-forge-cli.exe");

    assert_eq!(
        transport["command"],
        "D:/test/aionui/forge/aion-forge-cli.exe"
    );
    assert_eq!(transport["args"][0], "mcp-server");
    assert!(!transport.to_string().contains("aion-cli.exe"));
}
```

- [ ] **Step 2: Run RED**

Run `cargo test --manifest-path aion-forge-cli/Cargo.toml --test setup_contract`. Expected: `setup` is missing.

- [ ] **Step 3: Implement configuration generation**

Implement `mcp_transport` as a pure Rust function returning a stdio transport value with the supplied Forge executable and one `mcp-server` argument. Implement `setup --dry-run` to print the redacted intended configuration. Non-dry-run setup may call only the supported AionUI helper context; if that context is absent, return `CONFIG_ENV_MISSING` and do not inspect or write the backend database.

- [ ] **Step 4: Run GREEN**

Run all worker A tests. Expected: setup contract and earlier contracts pass.

- [ ] **Step 5: Commit worker A's setup slice**

Commit message: `feat(forge-cli): generate supported MCP setup`.

### Task 6: Make ACP protocol-only — worker B

**Files:**
- Modify: `aion-forge-acp/src/main.rs`
- Modify: `aion-forge-acp/Cargo.toml`
- Create: `aion-forge-acp/tests/cli_isolation.rs`
- Preserve: `aion-forge-acp/src/acp.rs`

- [ ] **Step 1: Add RED isolation tests**

Expose a small parser from a library module or test the binary process. Assert that `acp` parses and each of `--tool`, `--params`, `--list`, `setup`, and `mcp-server` fails. Assert the help text describes only the ACP adapter.

- [ ] **Step 2: Run RED**

Run `cargo test -p aion-forge-acp --test cli_isolation`. Expected: current `--tool`, `--params`, and `--list` remain accepted, so the isolation test fails for the intended reason.

- [ ] **Step 3: Remove non-ACP responsibilities**

Delete direct builtin execution and list handling from ACP main. Remove dependencies used only by that behavior, including `glitch-filter` when no longer referenced. Keep ACP stdout/stderr protocol separation and the existing `acp.rs` behavior.

- [ ] **Step 4: Run GREEN and ACP regression tests**

Run the isolation test and all `aion-forge-acp` tests. Expected: all pass.

- [ ] **Step 5: Commit worker B's slice**

Commit message: `refactor(acp): keep the adapter protocol-only`.

### Task 7: Update active product surfaces — worker C

**Files:** Use only worker C's ownership list.

- [ ] **Step 1: Establish the failing reference audit**

Search active files for Forge references to the exact executable/package identity `aion-cli` or `aion-cli.exe`, excluding `docs/archive/**`, the approved design/plan migration history, and statements that explicitly identify the external AionUI CLI. Expected before edits: active release, Docker, install, MCP, skill, and guide files contain forbidden Forge references.

- [ ] **Step 2: Update build and release surfaces**

Change release matrix artifact names, build package selection, Docker copies, package scripts, install verification, safety manifest entries, and generated adapter entrypoints to `aion-forge-cli`. Keep `aion-forge-acp` optional and do not add `aion-cli` compatibility artifacts.

- [ ] **Step 3: Update active docs and skill surfaces**

Update the active overview, installation guide, handoff, README, assistant prompt, `.mcp.json` examples, and root skill metadata to describe `aion-forge-cli.exe mcp-server`. State that `aion-cli` belongs to the separate AionUI Agent project. Do not rewrite archived historical evidence.

- [ ] **Step 4: Re-run the reference audit**

Expected: no active Forge surface uses `aion-cli` as a Forge executable or package. Historical or explicit external-product references are the only remaining matches.

- [ ] **Step 5: Commit worker C's slice**

Commit message: `docs: point Forge surfaces to its own CLI`.

### Task 8: Integrate workspace ownership and remove the duplicate — coordinator

**Files:**
- Modify: `Cargo.toml`
- Modify: `Cargo.lock`
- Move: `aion-cli-gen/**` to `aion-forge-cli-gen/**`
- Remove from Forge after preservation: `aion-cli/**`
- Integrate worker branches/changes

- [ ] **Step 1: Verify all worker deliveries and RED/GREEN evidence**

Inspect each diff, rerun each targeted test, and reject changes outside assigned ownership.

- [ ] **Step 2: Integrate workspace members**

Add `aion-forge-cli`, retain `aion-forge-acp`, remove `aion-cli`, and rename `aion-cli-gen` to `aion-forge-cli-gen` in the root workspace. Update the renamed generator's package identity and Rust documentation without changing behavior. Regenerate `Cargo.lock` through Cargo, not manual editing.

- [ ] **Step 3: Prove the external CLI copy is intact**

Repeat Task 1's hash comparison immediately before removal. If any mismatch exists, stop. After equality is confirmed, remove `aion-cli/**` only from the Forge repository; the sibling project remains untouched.

- [ ] **Step 4: Run integration checks**

Run `cargo check --workspace --locked`, all new targeted tests, and the active reference audit. Expected: no package named `aion-cli` is built by Forge and no active Forge config launches it.

- [ ] **Step 5: Commit the integration slice**

Commit message: `refactor: separate Forge from the AionUI CLI`.

### Task 9: Build, deploy, and verify AionUI runtime — coordinator

**Files:**
- Build: `target/release/aion-forge-cli.exe`
- Deploy: `D:/test/aionui/forge/aion-forge-cli.exe`
- Update: `D:/test/aionui/config/skills/aion-forge/**`
- Backup only: `D:/Temp/AionForgeEntrypointMigration-20260722/**`

- [ ] **Step 1: Build the release artifact**

Run the locked release build for `aion-forge-cli` and `aion-forge-acp`. Record exit code, artifact sizes, and SHA-256 values.

- [ ] **Step 2: Verify the new artifact before deployment**

Run `--help`, `--list`, direct `echo`, MCP `initialize`, MCP `tools/list`, and MCP local `tools/call`. Expected: Forge identity, 75 tools, valid JSON-only stdout, and no ACP command.

- [ ] **Step 3: Update AionUI through supported configuration**

Back up the current D-drive Forge skill package. Update its skill metadata and executable to `aion-forge-cli.exe`. Use the official AionUI config helper to update and read back the persisted MCP transport. If the helper reports `CONFIG_ENV_MISSING`, do not edit the database; record the blocker and keep the old runtime operational until an authorized helper context is available.

- [ ] **Step 4: Restart and inspect the actual process**

Restart AionUI, wait for `aioncore`, trigger Forge MCP lazily, and verify the child executable path and SHA-256 are the new Forge artifact. No running Forge process may resolve to an `aion-cli.exe` path.

- [ ] **Step 5: Run a real routed model call**

Call `code_generate` through the AionUI-connected Forge MCP and confirm a non-placeholder result, Provider metadata, and usage accounting.

### Task 10: Final verification and closeout — coordinator

**Files:**
- Record only verified changes and decisions through Forge self-evolution tools.

- [ ] **Step 1: Run the complete test suite**

Run `cargo test --workspace --no-fail-fast --locked`. Expected: zero failures across unit, integration, and documentation tests.

- [ ] **Step 2: Run formatting and build verification**

Run workspace formatting check, workspace check, and locked release build. Report pre-existing unrelated formatting failures separately; do not modify unrelated files.

- [ ] **Step 3: Verify requirements line by line**

Confirm the six completion criteria from the approved design: Forge owns its CLI/MCP; ACP is protocol-only; external AionUI CLI is intact; active references are clean; AionUI launches the new binary; real MCP execution succeeds.

- [ ] **Step 4: Inspect Git state**

Review `git status`, changed files, commit list, and diff statistics. Do not push without explicit authorization.

- [ ] **Step 5: Record the session**

Call Forge `record_change` for every modified file group, record the architecture decision, generate `session_report`, and store the concise report as a Decision memory.

