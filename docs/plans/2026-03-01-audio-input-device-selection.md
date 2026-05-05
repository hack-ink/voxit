# Audio Input Device Selection Implementation Plan

## Goal

Deliver and document the implemented microphone picker behavior, aligned to current code paths and config contract.

## High-level execution steps

1. Confirm configuration parsing and serialization for audio keys
   - Ensure `audio.input_device_id` and `audio.input_device_name` are preserved in `AudioConfig`.
   - Keep defaults as:
     - `input_device_id = 0`
     - `input_device_name = ""`
   - Keep parse/serialize behavior unchanged except for explicit persistence of these keys.

2. Confirm audio module selection path
   - Keep `list_input_devices()` returning all input-capable devices (sorted for deterministic order).
   - Keep `resolve_input_device()` behavior:
     - `None` => default input.
     - explicit ID => selected if still input-capable.
     - explicit invalid/missing ID => system default with `fallback_to_default = true`.
   - Keep `start_recording_with_stream()` returning `InputDeviceSelection`.

3. Wire app startup and picker state sync
   - Refresh microphone list on startup and before user interaction via `refresh_input_devices()`.
   - Keep `sync_input_device_name()` behavior so persisted names are repaired to current label when possible.
   - Keep `selected_input_device_label()` as canonical display formatting.

4. Implement picker UI behavior
   - Keep `Refresh microphones` action tied to `refresh_input_devices()`.
   - Keep combo options:
     - `System default` mapped to `0`.
     - discovered device ids mapped to list entries.
   - On selection change:
     - write `config.audio.input_device_id`.
     - write `config.audio.input_device_name` for UI readability.
     - call `persist_config()`.

5. Apply startup-time fallback into recording flow
   - In `start_recording()`, pass optional configured id using `configured_input_device_id()`.
   - When `InputDeviceSelection` reports fallback:
     - prepend fallback notice in status text.
     - keep recorder + realtime session on fallback path.
   - Continue to proceed with Pass2/Pass3 flow once recorder starts.

6. Preserve diagnostics and constraints
   - Keep user-facing status updates for:
     - empty/failed refreshes,
     - fallback-to-default behavior,
     - non-macOS unsupported recording path.
   - Keep behavior aligned with non-breaking UI contract and existing restart/reload behavior.

## Validation scope (manual, no test rewrite)

- Update docs/plans only; no code edits in this slice.
- Verify the two key names are referenced as `audio.input_device_name` and `audio.input_device_id`.
