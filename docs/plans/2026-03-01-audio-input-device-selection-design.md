# Audio Input Device Selection Design

## Scope

Document the implemented behavior for choosing and persisting the microphone used for recording, including fallback and UX constraints.

## UX

- The runtime control panel shows a microphone section with:
  - A **Refresh microphones** button to re-enumerate available input devices.
  - A **Input device** combo box rendered from discovered devices and a **System default** option.
- Combo text conventions:
  - `System default` corresponds to no explicit device override.
  - Discovered item labels follow `name (id)`.
  - If the selected ID is no longer in the list, the fallback label uses `Device #<id>` or the persisted `audio.input_device_name`.
- Changing selection updates config immediately and persists it.
- Recording status should expose fallback when it happens:
  - e.g., `Selected microphone unavailable. Falling back to default: <name>.`

## Config contract

- Keys under `[audio]`:
  - `audio.input_device_id` (number, `0` => use system default).
  - `audio.input_device_name` (string, best-effort human-readable label).
- Default state:
  - `audio.input_device_id = 0`.
  - `audio.input_device_name = ""`.
- Persistence:
  - Both keys are serialized in config writes.
  - On load, missing/invalid keys fall back to defaults.
- Resolution rules:
  - If `audio.input_device_id == 0`, recording uses the platform default microphone.
  - If non-zero, app attempts that ID.

## Fallback and constraints

- If configured `audio.input_device_id` is invalid, disconnected, or lacks input scope at session start:
  - selection falls back to default input device.
  - recording proceeds with `fallback_to_default = true`.
  - status/logging reports the fallback.
- If the device enumeration call fails or returns empty:
  - combo still supports **System default** path.
  - no devices can be shown/selected from the list.
- Non-macOS paths currently do not support mic capture and are not in-scope for picker functionality.

## Acceptance criteria

- Picker always presents **System default** and any available input-capable device list.
- Selection persists and survives restart via `audio.input_device_name` + `audio.input_device_id`.
- Session start is deterministic when configured devices are unavailable.
- Fallback behavior is transparent in status/log output.
