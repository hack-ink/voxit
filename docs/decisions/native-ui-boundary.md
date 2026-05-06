# Native UI Boundary

Status: accepted

Date: 2026-05-05

Question: How should Voxit split product logic from platform UI after replacing the
legacy Rust UI shell with a macOS-first native interface?

Decision: Voxit uses Rust Core plus platform-native UI. Shared runtime contracts,
provider/auth/audio orchestration, transcript assembly, and platform-neutral UI model
types stay in Rust crates. macOS UI lives under `native/macos-host/` as a SwiftPM
package that links a small Rust C ABI from `packages/voxit-host-ffi/`.

Consequences:

- The app shell is macOS-only and implemented with Swift/SwiftUI.
- The legacy Rust UI crate and legacy Rust app bundling path are not app authority.
- Swift renders state copied from Rust-owned model snapshots instead of redefining core
  state machines.
- The host FFI starts narrow: ABI version, session lifetime, and snapshot copy-out.
  Auth, audio, transcription, and paste APIs should cross the boundary only after their
  Rust service APIs are intentionally promoted.
- The first auth route exposed to native UI is ChatGPT OAuth via device code.
