# Runbook Index

Purpose: Route agents to procedural documents that tell them which sequence to execute.

Question this index answers: "which sequence should I execute?"

## Use this index when

- You need a runbook, how-to, migration sequence, validation flow, troubleshooting path,
  or maintenance procedure.
- You already know the relevant spec and need the operational steps.
- You need explicit prerequisites, commands, checkpoints, or verification.

## Do not use this index when

- You need the authoritative contract, schema, or invariant.
- You need current repository layout or implementation boundaries.
- You need durable design rationale rather than operator steps.

## What belongs in `docs/runbook/`

- Task-oriented operator procedures.
- Validation and inspection sequences.
- Rollout, rollback, and recovery flows.
- Bounded recipes that depend on a governing spec.

## Current runbooks

- [`first-run-onboarding.md`](./first-run-onboarding.md) for first sign-in, permission
  grant, and paste-path verification on macOS.
