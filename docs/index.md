# Documentation Index

Purpose: Route agents to the smallest correct repository surface for the current task.

Audience: All documentation in this repository is written for AI agents and LLM
workflows. The split below is by question type, not by human-versus-agent audience.

## Read order

- Read `README.md` first when you need the repository scope, platform target, or
  top-level runtime summary.
- Use `cargo make` whenever an equivalent repo task exists. When task details matter,
  inspect `Makefile.toml` directly.
- Read `docs/policy.md` for document contracts, placement rules, and naming rules.
- Read `Makefile.toml` when the task depends on repo task names or execution entrypoints.
- Then choose one primary lane:
  - `docs/spec/index.md` when the question is "what must be true?"
  - `docs/runbook/index.md` when the question is "which sequence should I execute?"
  - `docs/reference/index.md` when the question is "how is it currently organized or
    implemented?"
- Use `docs/plans/` only when a planning tool or execution workflow explicitly points to
  a saved plan artifact there.

## Routing matrix

- Need contracts, invariants, schemas, enums, state machines, or required behavior ->
  `docs/spec/`
- Need runbooks, onboarding, validation steps, troubleshooting, or operational sequences
  -> `docs/runbook/`
- Need current repository layout, ownership boundaries, or implementation surface maps ->
  `docs/reference/`
- Need repo task names or automation entrypoints -> `Makefile.toml`
- Need documentation placement or authoring rules -> `docs/policy.md`
- Need a planning-tool artifact or saved execution plan -> `docs/plans/`

## Retrieval rules

- Optimize for agent routing and execution, not narrative flow.
- Keep one authoritative document per topic. Link instead of copying.
- Keep runtime authority explicit: application and package crates plus `docs/spec/`
  outrank runbook, reference, and plan artifacts.
- Start each document with a short routing header that says what the document is for,
  when to read it, and what it does not cover.
- Keep links explicit and stable.
