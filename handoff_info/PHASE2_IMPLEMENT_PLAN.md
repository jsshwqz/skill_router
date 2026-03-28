# Implementation Plan - Event Migration (Phase 2.1)

This task focuses on upgrading the automation kernel's audit capability by migrating from a simple `error_history` to a structured `AutomationEvent` stream. This will enable better diagnostics, cross-agent state handoff, and UI visualization.

## Proposed Changes

### [Component] aion-router (Automation Kernel)

#### [MODIFY] [state.rs](file:///d:/test/aionui/skill/%E6%96%B0%E5%BB%BA%E6%96%87%E4%BB%B6%E5%A4%B9/aion_forge/aion-router/src/automation/state.rs)
- Add `AutomationEvent` enum to capture various lifecycle events:
  - `TaskStarted`, `StepStarted`, `StepExecuted`, `StepVerified`, `SideEffectOccurred`, `RecoveryDecision`, `UserAcknowledgment`, `ErrorOccurred`, `TaskCompleted`, `TaskFailed`.
- Add `EventEntry` struct with `timestamp_ms`.
- Add `event_stream: Vec<EventEntry>` to `AutomationState`.
- Mark `error_history` as deprecated (but keep for compatibility or remove if clean-slate is preferred).

#### [MODIFY] [loop_engine.rs](file:///d:/test/aionui/skill/%E6%96%B0%E5%BB%BA%E6%96%87%E4%BB%B6%E5%A4%B9/aion_forge/aion-router/src/automation/loop_engine.rs)
- Inject event logging at:
  - Loop start (TaskStarted)
  - Execution start (StepStarted)
  - After `executor.execute` (StepExecuted, SideEffectOccurred)
  - After `verifier.verify` (StepVerified)
  - In `RecoveryEngine` decision point (RecoveryDecision)
  - At High-risk confirm gate (UserAcknowledgment)
  - On error (ErrorOccurred)
  - Loop exit (TaskCompleted/TaskFailed)

## Verification Plan

### Automated Tests
- Run `cargo test --test cpevr_test`
- Add a new test case `test_event_stream_recording` in `cpevr_test.rs` to verify that specific events (like `StepStarted`, `StepVerified`) are captured with correct IDs and sequence.

### Manual Verification
- None required as this is a kernel-level structural change verified by integration tests.
