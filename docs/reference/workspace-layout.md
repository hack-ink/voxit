# Workspace Layout

Purpose: Describe the current top-level repository surfaces and which concerns each one
owns.

Read this when: You need to know where the app entrypoint, shared packages, repo task
definitions, or documentation topics currently live.

Not this document: The normative runtime contract or the onboarding sequence.

Covers: The workspace surface map, ownership boundaries, and the role of `apps/`,
`packages/`, `docs/`, `scripts/`, and repository root policy files.

## Top-level surfaces

- `apps/voxit/` holds the application crate and packaging-facing entrypoint for the
  macOS app.
- `packages/voxit-core/` holds the shared runtime logic, auth, OpenAI integration, and
  dictation pipeline code.
- `packages/voxit-audio/` holds audio-capture specific functionality.
- `packages/voxit-macos/` holds macOS-specific integration surfaces.
- `docs/spec/` holds normative runtime and behavior contracts.
- `docs/runbook/` holds operator procedures such as onboarding and validation flows.
- `docs/reference/` holds current repository and implementation surface maps.
- `docs/plans/` holds saved plan artifacts rather than governing repository policy.
- `Makefile.toml` holds repo-native task names for lint, test, format, and checks.
- `scripts/` holds repository helper scripts such as local macOS packaging helpers.
- `.github/workflows/` holds CI and release automation.

## Boundary notes

- Runtime authority stays in the application and package crates plus the governing specs
  under `docs/spec/`.
- `docs/runbook/` and `docs/reference/` must not override runtime or configuration
  authority.
- `Makefile.toml` is the source of truth for named repository tasks.
- `docs/plans/` can capture design or execution artifacts, but those files do not become
  policy until their conclusions are promoted into spec, runbook, or reference docs.
