# Stateful Aion Forge ACP Agent Design

## Goal

Turn `aion-forge acp` into a stateful AionUI Agent that can describe and execute the live Forge builtin registry, retain conversation context, and honor per-session model selection.

This extends the unified CLI design and supersedes its model-selection non-goal and older guidance that treated ACP only as a passive compatibility adapter.

## Current Problem

The adapter creates a session ID without storing state. Every `session/prompt` calls `ai_task` once with a generic assistant instruction, so the model receives neither Forge identity nor its capability catalog and no planner executes tools.

Model selection is not persisted. A prompt without a model receives a fixed default, while an unmatched requested model silently falls back to other endpoints. AionUI can therefore display a model that does not control the actual request. Bootstrap prompts such as Assistant Rules and `[Skill: ...]` are also answered independently instead of being retained as session instructions.

## Architecture

Use official Rust ACP protocol types where available, preserve the verified stdin/stdout process boundary, and add a narrow compatibility layer for AionUI versions that still send legacy model fields.

Separate Rust modules own transport, session state, model configuration, planning, and tool execution. The unified CLI continues to dispatch `aion-forge acp` in-process. Protocol messages use stdout exclusively and tracing diagnostics use stderr.

The provider-neutral planner returns a typed action envelope, so the tool loop does not depend on provider-native function calling.

## Session State

The server stores sessions in memory by ACP session ID. A session contains its working directory, selected model, modes and configuration, injected instructions, bounded conversation and tool history, and active cancellation state.

`session/new` validates the working directory, selects an enabled model, stores the session, and returns configuration options. Unknown session IDs produce protocol errors.

Explicit AionUI bootstrap envelopes are stored as instructions without invoking the model. Ordinary content remains a user turn, including normal discussion about skills.

## Model Configuration

The advertised catalog comes only from enabled `candidate_ai_endpoints()` entries, deduplicated by model ID. Static unavailable entries are removed.

The server exposes a stable ACP select configuration option and handles `session/set_config_option`. Compatibility handling accepts models supplied through session creation, prompt parameters, or a legacy model-selection request. Every path updates the same session field.

Before an AI request, the selected model must resolve to one exact enabled endpoint. Unknown or unavailable models return a visible error listing valid choices. They never silently fall back to DeepSeek or another provider.

An explicit `auto` option is the only mode allowed to use configured endpoint priority and fallback.

## Identity and Capabilities

The system instruction is generated from `BuiltinRegistry` at request time. It identifies Aion Forge and includes exact callable builtin names and public descriptions. Internal AI entries that would recurse into the planner are excluded.

Identity and capability questions are answered from this generated catalog; no hard-coded count or copied list is maintained. AionUI-injected skills supplement rather than replace native capabilities.

## Agent Loop

For each ordinary prompt:

1. Build instructions from Forge identity, registry metadata, session instructions, history, and the user turn.
2. Ask the selected model for either a final response or one registered tool call.
3. Parse and validate the typed action, tool name, and JSON object arguments.
4. Emit an ACP tool-call update and execute through `BuiltinRegistry`.
5. Emit completion or failure, append the normalized observation, and continue planning.
6. Stream the final assistant response and return the stop reason.

The initial limit is six calls per prompt. One malformed action receives one repair attempt. Repeated identical failures, cancellation, or limit exhaustion produce a visible diagnostic response. Recursive AI entry points and unregistered tools are rejected.

## ACP Behavior

Assistant content uses `session/update` agent-message chunks. Tools use standard tool-call and tool-call-update events. Each accepted user prompt produces visible assistant content or a visible protocol error; empty visible turns are not valid outcomes.

Cancellation stops new tool calls and prevents late results from entering history. Provider keys, headers, and environment values never appear in events or logs.

## Provider Boundary

Existing `ai_task` fallback remains unchanged for non-ACP callers. The ACP engine validates an exact endpoint before calling it, preventing ordinary ACP requests from entering fallback behavior.

Planner and executor traits allow deterministic tests without network access. Production uses existing configured endpoints and the existing builtin registry.

## Compatibility

`aion-forge acp` remains canonical. `aion-forge-cli acp` and the standalone ACP binary may delegate to the same library during the compatibility period. MCP and direct-tool commands remain unchanged.

After deployment, development Agent records may be refreshed so AionUI does not retain stale handshake or model metadata.

## Test Strategy

Use red-green-refactor for each boundary.

Unit tests cover session lifecycle, bootstrap ingestion, live capability catalogs, model options and persistence, invalid models, action parsing and repair, loop limits, repeated calls, and cancellation.

Protocol tests cover initialization, session creation, stable and legacy model updates, visible assistant chunks, tool lifecycle events, stop reasons, unknown sessions, and JSON-only stdout.

Integration tests use fake planners and executors to prove tool execution, result consumption, final responses, and multi-turn history. AionUI verification confirms identity, capabilities, visible tools, retained context, two real model selections, and visible invalid-model failure without DeepSeek fallback.

## Success Criteria

1. The ACP entry point is a stateful Agent rather than a one-shot chat adapter.
2. Identity and capabilities derive from the live registry.
3. Tool-requiring prompts execute real builtins with visible lifecycle events.
4. Bootstrap rules and skills survive into later turns.
5. Only enabled models plus `auto` are advertised.
6. Model selection persists and controls the actual endpoint.
7. Invalid models and provider failures never cause implicit cross-provider fallback.
8. Focused tests, formatting, linting, and release builds pass.
9. The deployed executable passes real AionUI conversation and model-switch tests.

## Non-Goals

- Changing AionUI Agent discovery internals.
- Replacing Forge MCP or direct-tool interfaces.
- Adding credentials or enabling unconfigured providers.
- Persisting conversations across process restarts.
- Refactoring unrelated router builtins.
