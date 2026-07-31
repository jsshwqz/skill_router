# Error Knowledge and Prevention Design

## Goal

Turn execution failures into durable, actionable knowledge so Forge can prevent the same failure, apply a verified mitigation, and detect regressions.

## Data Model

Each failure receives a deterministic fingerprint derived from capability, error class, normalized error text, and canonical execution context. Records persist locally with backward-compatible defaults.

The lifecycle is observed, reproduced, fixed, verified, and regressed. A verified recurrence becomes regressed and increases priority.

Fix evidence binds a commit, regression test, optional pull request, and optional safe mitigation to one fingerprint. Replay metadata tracks observed and verified versions.

## Execution Gate

Before execution, Forge compares capability and canonical context with durable records:

- unresolved matching records block accidental repetition;
- verified records require their known mitigation evidence;
- explicit fingerprint replay permits controlled reproduction;
- unrelated successful executions cannot resolve a record.

## Integration

The learner owns persistence and lifecycle transitions. The executor invokes the prevention gate and records failures automatically. The `error_knowledge` capability exposes controlled lifecycle operations to the autonomous agent.

## Safety

The feature performs no network calls or production mutation. Automatic mitigation is evidence-gated and must match the stored verified mitigation exactly.
