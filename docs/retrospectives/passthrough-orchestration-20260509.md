# Forge passthrough orchestration retrospective - 2026-05-09

## Problem

The MCP entrypoint honored `AI_PASSTHROUGH=true` for simple AI tools, but dedicated orchestration tools such as `ai_code_generate` still forced real external engines. This contradicted the platform-dependent usage model where Forge should defer model execution to the host AionUI/Codex platform when passthrough is enabled.

## Fix

`OrchestratorConfig::from_env()` now reads `AI_PASSTHROUGH` and enables passthrough for orchestration tools when the value is `true` or `1`.

The globally installed Forge CLI at `D:\test\aionui\config\skills\aion-forge\bin\aion-cli.exe` was rebuilt and replaced from the release build, with the old binary preserved as a timestamped backup.

## Verification

- `cargo check -p aion-router`
- `cargo build --release -p aion-cli`
- Direct MCP subprocess check against the installed binary with `AI_PASSTHROUGH=true`
- Verified `text_summarize` returns host-LLM instructions
- Verified `ai_code_generate` returns a `type=passthrough` structured instruction instead of calling external engines

## Lesson

Do not treat passthrough as a partial MCP-only shortcut. Any Forge tool that can call an AI engine must share the same provider policy, otherwise the platform appears configured correctly while specific tools still fail through external backends.
