# Documentation Policy

Purpose: Define the repository-wide documentation taxonomy, naming rules, and placement
rules for durable agent-facing content.

Audience: All documentation under `docs/` is written for AI agents and LLM workflows.
The split below is by question type, not by reader type.

## Primary taxonomy

This repository standardizes on three primary documentation lanes:

| Lane | Location | Answers | Holds |
| --- | --- | --- | --- |
| Spec | `docs/spec/` | What must be true? | Contracts, schemas, invariants, required behavior |
| Runbook | `docs/runbook/` | Which sequence should I execute? | Operational procedures, onboarding steps, validation flows, recovery steps |
| Reference | `docs/reference/` | How is it currently organized or implemented? | Repository layout, surface maps, current implementation boundaries |

## Artifact lanes

- `docs/plans/` is allowed for plan artifacts that are explicitly produced or consumed by
  a planning workflow.
- `docs/plans/` is not a primary documentation lane and is not authoritative for runtime
  behavior, repository policy, or operator procedures.

## Placement rules

- If a document defines correctness, it belongs in `docs/spec/`.
- If a document defines operator actions, it belongs in `docs/runbook/`.
- If a document describes current structure, ownership, or implementation boundaries, it
  belongs in `docs/reference/`.
- Do not duplicate authoritative content across lanes. Link to the source of truth.

## Naming rules

- Directory names express document type.
- File names express stable topic.
- Use lowercase kebab-case for document file names.
- Do not encode temporary versions such as `v0`, `v1`, or `draft2` into stable file
  names.
- Do not repeat the directory class in the file name when the topic is already clear.
  Prefer `runtime.md` under `docs/spec/` over `runtime-spec.md`.

## Document headers

Every document should start with a short routing header.

Spec header:

- `Purpose`
- `Status: normative`
- `Read this when`
- `Not this document`
- `Defines`

Runbook header:

- `Goal`
- `Read this when`
- `Preconditions` or `Inputs`
- `Depends on`
- `Verification` or `Outputs`

Reference header:

- `Purpose`
- `Read this when`
- `Not this document`
- `Covers`

## Canonical entry points

- Unified router: `docs/index.md`
- Normative router: `docs/spec/index.md`
- Procedural router: `docs/runbook/index.md`
- Current-state router: `docs/reference/index.md`
- Repo task and automation entrypoints: `Makefile.toml`

## Update workflow

- Behavior or schema change: update the relevant spec.
- Procedure change: update the relevant runbook.
- Structural or ownership change: update the relevant reference doc.
- If a document drifts across lanes, split it instead of stretching one document to do
  several jobs.
