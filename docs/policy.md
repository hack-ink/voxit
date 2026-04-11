# Documentation Policy

Purpose: Define the repository-wide documentation taxonomy, naming rules, and placement
rules for durable agent-facing content.

Audience: All documentation under `docs/` is written for AI agents and LLM workflows.
The split below is by question type, not by reader type.

## Primary taxonomy

| Lane | Location | Answers | Holds |
| --- | --- | --- | --- |
| Spec | `docs/spec/` | What must be true? | Contracts, schemas, invariants, required behavior |
| Runbook | `docs/runbook/` | Which sequence should I execute? | Operational procedures, onboarding steps, validation flows, recovery steps |
| Reference | `docs/reference/` | How is it currently organized or implemented? | Repository layout, surface maps, current implementation boundaries |
| Decisions | `docs/decisions/` | Why is it shaped this way? | Durable rationale, tradeoffs, and consequences |

## Placement rules

- If a document defines correctness, it belongs in `docs/spec/`.
- If a document defines operator actions, it belongs in `docs/runbook/`.
- If a document describes current structure, ownership, or implementation boundaries, it
  belongs in `docs/reference/`.
- If a document records durable rationale or tradeoffs, it belongs in
  `docs/decisions/`.
- If a document drifts across lanes, split it instead of stretching one file to answer
  several question types.
- Do not duplicate authoritative content across lanes. Link to the source of truth.
- Do not add `docs/plans/` back. Transient planning artifacts are not part of the
  durable docs tree in this repository.

## Naming rules

- Directory names express document lane.
- File names express stable topic.
- Use lowercase kebab-case for document file names.
- Keep primary-lane file names short and topic-first.
- Do not encode temporary versions such as `v1`, `draft2`, or dates into primary-lane
  file names.
- Do not repeat the directory class in the file name when the topic is already clear.
  Prefer `runtime.md` under `docs/spec/` over `runtime-spec.md`.
- Prefer names like `runtime.md`, `first-run.md`, and `repository-layout.md`.
- Keep `index.md` reserved for lane routers.

## Document headers

Every primary-lane document should start with a short routing header.

Spec header:

- `Purpose`
- `Status: normative`
- `Read this when`
- `Not this document`
- `Defines`

Runbook header:

- `Goal`
- `Read this when`
- `Inputs` or `Preconditions`
- `Depends on`
- `Outputs` or `Verification`

Reference header:

- `Purpose`
- `Read this when`
- `Not this document`
- `Covers`

Decision header:

- `Status`
- `Date`
- `Question`
- `Decision`
- `Consequences`

## Canonical entry points

- Unified router: `docs/index.md`
- Normative router: `docs/spec/index.md`
- Procedural router: `docs/runbook/index.md`
- Current-state router: `docs/reference/index.md`
- Rationale router: `docs/decisions/index.md`
- Repo task and automation entrypoints: `Makefile.toml`

## Update workflow

- Behavior or schema change: update the relevant spec.
- Procedure change: update the relevant runbook.
- Structural or ownership change: update the relevant reference doc.
- Tradeoff or rationale change: update the relevant decision doc.
- If a document starts carrying normative content from another lane, move that content
  into the authoritative lane and link to it.
