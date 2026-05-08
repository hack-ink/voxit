# Repository Layout

Purpose: Describe the current top-level repository surfaces and which concerns each one
owns.

Read this when: You need to know where the app entrypoint, shared packages, repo task
definitions, or documentation topics currently live.

Not this document: The normative runtime contract, the first-run operator sequence, or
the design rationale behind specific product choices.

Covers: The repository surface map, ownership boundaries, and the role of
`native/macos-host/`, `packages/`, `docs/`, `scripts/`, and repository root policy
files.

## Top-level surfaces

- `native/macos-host/` holds the SwiftPM native macOS host. It owns platform UI
  composition, the menu bar extra, the Voxit control-center window, the Settings
  window, and links Rust through the host FFI static library.
- `packages/voxit-core/` holds the shared runtime logic, auth, OpenAI integration, and
  dictation pipeline code. Platform-neutral UI model types and contextual voice
  planning contracts also live here so hosts do not invent divergent state names,
  profile routing, or output policies.
- `packages/voxit-audio/` holds audio-capture specific functionality.
- `packages/voxit-host-ffi/` holds the thin C ABI consumed by `native/macos-host/`.
- `packages/voxit-macos/` holds macOS-specific integration surfaces.
- `docs/spec/` holds normative runtime and behavior contracts.
- `docs/runbook/` holds operator procedures such as onboarding and validation flows.
- `docs/reference/` holds current repository and implementation surface maps.
- `docs/decisions/` holds durable rationale and tradeoffs behind current design choices.
- `Makefile.toml` holds repo-native task names for lint, test, format, and checks.
- `scripts/` holds repository helper scripts such as Swift native-host staging and
  local launch helpers.
- `.github/workflows/` holds CI and release automation.

## Boundary notes

- Runtime authority stays in the application and package crates plus the governing specs
  under `docs/spec/`.
- Native UI code may depend on `packages/voxit-host-ffi/`, but it must not duplicate
  provider/auth/audio runtime policy or contextual voice routing already owned by Rust
  core crates.
- `docs/runbook/`, `docs/reference/`, and `docs/decisions/` must not override runtime
  or configuration authority.
- `Makefile.toml` is the source of truth for named repository tasks.
- Decision docs explain why the system is shaped a certain way; the spec still defines
  what must be true at runtime.
