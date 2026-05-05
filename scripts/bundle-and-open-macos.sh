#!/usr/bin/env bash
set -euo pipefail

repo_root="$(git rev-parse --show-toplevel)"
cd "$repo_root"

cargo bundle --release -p voxit

app_path="target/release/bundle/osx/Voxit.app"

if [ "${VOXIT_ALLOW_MULTI_INSTANCE:-0}" != "1" ]; then
  existing_pids="$(pgrep -x voxit || true)"
  if [ -n "$existing_pids" ]; then
    echo "Voxit is already running (pids: $(echo "$existing_pids" | tr '\n' ' '))."
    echo "Quit it first to avoid launching multiple menu bar instances (this script uses: open -n)."
    echo "To override, rerun with: VOXIT_ALLOW_MULTI_INSTANCE=1"
    exit 2
  fi
fi

# If the bundle has Gatekeeper attributes, Launch Services may block launch via `open` even for local builds.
# Only strip attributes when they are actually present.
if xattr -p com.apple.quarantine "$app_path" >/dev/null 2>&1; then
  echo "Detected Gatekeeper quarantine on $app_path; removing for local dev launch..."
  xattr -dr com.apple.quarantine "$app_path"
fi

# `com.apple.provenance` can trigger remote Gatekeeper assessment, which rejects ad-hoc signed local bundles.
if xattr -p com.apple.provenance "$app_path" >/dev/null 2>&1; then
  echo "Detected com.apple.provenance on $app_path; removing for local dev launch..."
  xattr -dr com.apple.provenance "$app_path"
  if xattr -p com.apple.provenance "$app_path" >/dev/null 2>&1; then
    echo "Note: com.apple.provenance is still present after removal attempt."
  fi
fi

# `cargo bundle` produces a minimal `.app` with an ad-hoc-signed Mach-O binary.
# Without a bundle signature, macOS Launch Services may refuse to launch it with:
# "code has no resources but signature indicates they must be present".
# Re-signing the bundle ad-hoc keeps local dev runs launchable via `open`.
codesign_identity="${VOXIT_CODESIGN_IDENTITY:--}"
echo "Signing Voxit.app with identity: $codesign_identity"
codesign --force --deep --sign "$codesign_identity" "$app_path"

# Some macOS builds attach `com.apple.provenance` during signing; strip it again to avoid
# triggering remote Gatekeeper assessment that kills the process shortly after launch.
if xattr -p com.apple.provenance "$app_path" >/dev/null 2>&1; then
  echo "Detected com.apple.provenance after codesign; removing for local dev launch..."
  xattr -dr com.apple.provenance "$app_path"
  if xattr -p com.apple.provenance "$app_path" >/dev/null 2>&1; then
    echo "Note: com.apple.provenance is still present after removal attempt."
  fi
fi

open -n "$app_path"

new_pid=""
for _ in {1..8}; do
  sleep 0.25
  pid_now="$(pgrep -x voxit || true)"
  if [ -n "$pid_now" ]; then
    new_pid="$pid_now"
    break
  fi
done

if [ -z "$new_pid" ]; then
  echo "Voxit process did not appear after open."
else
  # Verify it stays running for a short window (common failure mode: killed after remote assessment).
  alive=1
  for _ in {1..24}; do
    sleep 0.25
    if ! kill -0 "$new_pid" >/dev/null 2>&1; then
      alive=0
      break
    fi
  done

  if [ "$alive" -eq 1 ]; then
    echo "Voxit launched (pid=$new_pid)."
    echo "Note: Voxit is a menu bar app (LSUIElement=1). If no window appears, click the tray icon or press the hotkey."
    echo "Config: ~/Library/Application Support/hack.ink.voxit/config.toml (ui.start_hidden / hotkey.chord)"
    exit 0
  fi
fi

echo "Voxit did not stay running after launch via open (likely Gatekeeper/assessment kill)."
echo "Signature check:"
codesign -vvv --deep --strict "$app_path" 2>&1 | head -n 20 || true
echo "spctl assessment (expected reject for local ad-hoc builds):"
spctl --assess --type execute --verbose=4 "$app_path" 2>&1 | head -n 20 || true
echo "Falling back to direct exec (bypasses LaunchServices):"
nohup "$app_path/Contents/MacOS/voxit" >/dev/null 2>&1 &
sleep 0.5
pid_fallback="$(pgrep -x voxit || true)"
if [ -n "$pid_fallback" ]; then
  echo "Voxit launched via direct exec (pid=$pid_fallback)."
else
  echo "Direct exec failed too. Recent crash reports:"
  ls -lt "$HOME/Library/Logs/DiagnosticReports" 2>/dev/null | rg -i "voxit|Voxit" | head -n 5 || true
  exit 1
fi
