#!/usr/bin/env bash
set -euo pipefail

MODE="${1:-run}"
APP_NAME="Voxit"
EXECUTABLE_NAME="VoxitNativeHost"
BUNDLE_ID="ink.hack.voxit"
MIN_SYSTEM_VERSION="14.0"
DEFAULT_SIGN_IDENTITY="x@acg.box"

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
PACKAGE_DIR="$ROOT_DIR/native/macos-host"
COMMON_ROOT="$(cd "$(git -C "$ROOT_DIR" rev-parse --git-common-dir)/.." && pwd)"
STAGE_DIR="${VOXIT_NATIVE_HOST_STAGE_DIR:-$COMMON_ROOT/target/voxit-native-host}"
APP_BUNDLE="$STAGE_DIR/$APP_NAME.app"
APP_CONTENTS="$APP_BUNDLE/Contents"
APP_MACOS="$APP_CONTENTS/MacOS"
APP_RESOURCES="$APP_CONTENTS/Resources"
APP_BINARY="$APP_MACOS/$EXECUTABLE_NAME"
INFO_PLIST="$APP_CONTENTS/Info.plist"
APP_ICON_SOURCE="$ROOT_DIR/assets/app-icon/generated/app-icon.icns"
APP_ICON_NAME="AppIcon.icns"
STATUS_ICON_SOURCE="$ROOT_DIR/assets/tray-icon/generated/tray-icon-template.png"
STATUS_ICON_NAME="StatusBarIcon.png"

RUST_PROFILE="${VOXIT_NATIVE_HOST_RUST_PROFILE:-debug}"
SWIFT_CONFIGURATION="${VOXIT_NATIVE_HOST_SWIFT_CONFIGURATION:-debug}"
RESOLVED_SIGN_IDENTITY=""

RUST_BUILD_ARGS=(-p voxit-host-ffi)
if [[ "${VOXIT_NATIVE_HOST_LOCKED:-0}" == "1" ]]; then
	RUST_BUILD_ARGS+=(--locked)
fi
if [[ "$RUST_PROFILE" == "debug" ]]; then
	RUST_LIB_DIR="$ROOT_DIR/target/debug"
else
	RUST_BUILD_ARGS+=(--profile "$RUST_PROFILE")
	RUST_LIB_DIR="$ROOT_DIR/target/$RUST_PROFILE"
fi

SWIFT_BUILD_FLAGS=()
if [[ "$SWIFT_CONFIGURATION" == "release" ]]; then
	SWIFT_BUILD_FLAGS=(-c release)
fi

APP_VERSION="$(sed -n '/^\[workspace.package\]/,/^\[/s/^version *= *"\(.*\)"/\1/p' "$ROOT_DIR/Cargo.toml" | head -n 1)"
APP_VERSION="${APP_VERSION:-0.1.0}"

write_info_plist() {
	local icon_plist_entry=""
	if [[ -f "$APP_RESOURCES/$APP_ICON_NAME" ]]; then
		icon_plist_entry="$(cat <<PLIST
  <key>CFBundleIconFile</key>
  <string>${APP_ICON_NAME%.icns}</string>
PLIST
)"
	fi

	mkdir -p "$APP_CONTENTS"
	cat >"$INFO_PLIST" <<PLIST
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
  <key>CFBundleExecutable</key>
  <string>$EXECUTABLE_NAME</string>
  <key>CFBundleIdentifier</key>
  <string>$BUNDLE_ID</string>
  <key>CFBundleName</key>
  <string>$APP_NAME</string>
  <key>CFBundleDisplayName</key>
  <string>$APP_NAME</string>
  <key>CFBundlePackageType</key>
  <string>APPL</string>
  <key>CFBundleShortVersionString</key>
  <string>$APP_VERSION</string>
  <key>CFBundleVersion</key>
  <string>$APP_VERSION</string>
  <key>LSApplicationCategoryType</key>
  <string>public.app-category.productivity</string>
  <key>LSMinimumSystemVersion</key>
  <string>$MIN_SYSTEM_VERSION</string>
  <key>LSUIElement</key>
  <true/>
  <key>NSHighResolutionCapable</key>
  <true/>
  <key>NSMicrophoneUsageDescription</key>
  <string>Voxit needs microphone access to transcribe your speech.</string>
  <key>NSPrincipalClass</key>
  <string>NSApplication</string>
$icon_plist_entry
</dict>
</plist>
PLIST
}

resolve_signing_identity() {
	local requested_identity identity_list identity

	requested_identity="${VOXIT_NATIVE_HOST_SIGN_IDENTITY:-$DEFAULT_SIGN_IDENTITY}"
	if [[ "$requested_identity" == "-" ]]; then
		RESOLVED_SIGN_IDENTITY="-"
		return 0
	fi

	identity_list="$(security find-identity -v -p codesigning 2>/dev/null || true)"
	if [[ -n "$requested_identity" ]]; then
		while IFS= read -r line; do
			identity="${line#*\"}"
			identity="${identity%%\"*}"
			if [[ -n "$identity" && "$identity" == *"$requested_identity"* ]]; then
				RESOLVED_SIGN_IDENTITY="$identity"
				return 0
			fi
		done <<<"$identity_list"
	fi

	while IFS= read -r line; do
		identity="${line#*\"}"
		identity="${identity%%\"*}"
		if [[ -n "$identity" && "$identity" == Apple\ Development:* ]]; then
			RESOLVED_SIGN_IDENTITY="$identity"
			return 0
		fi
	done <<<"$identity_list"

	return 1
}

sign_app_bundle() {
	local build_root="$1"
	local entitlements_path requested_identity
	requested_identity="${VOXIT_NATIVE_HOST_SIGN_IDENTITY:-$DEFAULT_SIGN_IDENTITY}"
	entitlements_path="$build_root/$EXECUTABLE_NAME-entitlement.plist"

	if resolve_signing_identity; then
		if [[ -f "$entitlements_path" ]]; then
			codesign \
				--force \
				--deep \
				--options runtime \
				--sign "$RESOLVED_SIGN_IDENTITY" \
				--entitlements "$entitlements_path" \
				"$APP_BUNDLE"
		else
			codesign \
				--force \
				--deep \
				--options runtime \
				--sign "$RESOLVED_SIGN_IDENTITY" \
				"$APP_BUNDLE"
		fi
		return
	fi

	echo "error: no valid macOS codesigning identity matching \"$requested_identity\" was found." >&2
	echo "error: import the real signing certificate or set VOXIT_NATIVE_HOST_SIGN_IDENTITY to a valid identity." >&2
	echo "error: Voxit native host staging requires a stable codesigning identity." >&2
	exit 1
}

stage_app_bundle() {
	local build_root build_binary

	MACOSX_DEPLOYMENT_TARGET="$MIN_SYSTEM_VERSION" cargo build "${RUST_BUILD_ARGS[@]}"

	build_root="$(
		VOXIT_HOST_FFI_LIB_DIR="$RUST_LIB_DIR" \
			swift build --package-path "$PACKAGE_DIR" "${SWIFT_BUILD_FLAGS[@]}" --show-bin-path
	)"
	build_binary="$build_root/$EXECUTABLE_NAME"

	rm -f "$build_binary"
	VOXIT_HOST_FFI_LIB_DIR="$RUST_LIB_DIR" \
		swift build --package-path "$PACKAGE_DIR" "${SWIFT_BUILD_FLAGS[@]}" --product "$EXECUTABLE_NAME"

	if [[ ! -x "$build_binary" ]]; then
		echo "error: Swift build did not produce $build_binary" >&2
		exit 1
	fi

	rm -rf "$APP_BUNDLE"
	mkdir -p "$APP_MACOS" "$APP_RESOURCES"
	cp "$build_binary" "$APP_BINARY"
	chmod +x "$APP_BINARY"
	if [[ -f "$APP_ICON_SOURCE" ]]; then
		cp "$APP_ICON_SOURCE" "$APP_RESOURCES/$APP_ICON_NAME"
	fi
	if [[ -f "$STATUS_ICON_SOURCE" ]]; then
		cp "$STATUS_ICON_SOURCE" "$APP_RESOURCES/$STATUS_ICON_NAME"
	fi
	write_info_plist
	plutil -lint "$INFO_PLIST" >/dev/null
	sign_app_bundle "$build_root"
}

terminate_running_host() {
	pkill -x "$EXECUTABLE_NAME" >/dev/null 2>&1 || true
}

open_app() {
	/usr/bin/open "$APP_BUNDLE"
}

case "$MODE" in
	stage|--stage)
		stage_app_bundle
		;;
	run)
		terminate_running_host
		stage_app_bundle
		open_app
		;;
	verify|--verify)
		terminate_running_host
		stage_app_bundle
		open_app
		sleep 1
		pgrep -x "$EXECUTABLE_NAME" >/dev/null
		;;
	--debug|debug)
		stage_app_bundle
		lldb -- "$APP_BINARY"
		;;
	*)
		echo "usage: $0 [run|stage|--verify|--debug]" >&2
		exit 2
		;;
esac
