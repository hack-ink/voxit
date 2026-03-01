//! macOS target app capture and activation helpers.

use std::{ffi::c_void, thread::sleep, time::Duration};

#[cfg(target_os = "macos")] use std::process::Command;

#[cfg(not(target_os = "macos"))] use std::io;
#[cfg(target_os = "macos")] use std::ptr;

#[cfg(target_os = "macos")] use block2::RcBlock;
#[cfg(target_os = "macos")]
use objc2_av_foundation::{AVAuthorizationStatus, AVCaptureDevice, AVMediaTypeAudio};
#[cfg(target_os = "macos")] use objc2_foundation::NSString;

#[cfg(target_os = "macos")]
type CfBoolean = u8;
#[cfg(target_os = "macos")]
type CfDictionaryRef = *const c_void;
#[cfg(target_os = "macos")]
type CfTypeRef = *const c_void;
#[cfg(target_os = "macos")]
type CfIndex = isize;

/// Captured frontmost application metadata.
#[derive(Debug, Clone, Eq, PartialEq, Default)]
pub struct TargetApp {
	/// Process identifier reported by macOS.
	pub pid: Option<u32>,
	/// Bundle identifier (for example, `com.apple.Safari`).
	pub bundle_id: Option<String>,
	/// Frontmost application name.
	pub app_name: Option<String>,
}

/// Permission-related privacy panes exposed in the onboarding checklist.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PermissionSettingsPane {
	/// Microphone privacy settings.
	Microphone,
	/// Accessibility privacy settings.
	Accessibility,
	/// Input monitoring privacy settings.
	InputMonitoring,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum MicrophonePermissionState {
	/// Permission already granted.
	Granted,
	/// Permission has not yet been requested.
	NotDetermined,
	/// Permission denied explicitly by user.
	Denied,
	/// Permission denied by system policy.
	Restricted,
	/// Permission prompt was requested.
	Prompted,
	/// Platform or status could not be queried.
	Unknown,
}

impl PermissionSettingsPane {
	/// Human-friendly label for status text.
	pub const fn display_name(self) -> &'static str {
		match self {
			Self::Microphone => "Microphone",
			Self::Accessibility => "Accessibility",
			Self::InputMonitoring => "Input Monitoring",
		}
	}
}

/// Check whether a permission pane is currently granted.
#[cfg(target_os = "macos")]
pub fn permission_is_granted(pane: PermissionSettingsPane) -> bool {
	match pane {
		PermissionSettingsPane::Microphone => has_microphone_permission(),
		PermissionSettingsPane::Accessibility => has_accessibility_permission(),
		PermissionSettingsPane::InputMonitoring => has_input_monitoring_permission(),
	}
}

/// Non-macOS fallback for permission checks.
#[cfg(not(target_os = "macos"))]
pub fn permission_is_granted(_pane: PermissionSettingsPane) -> bool {
	false
}

/// Request a permission for the selected pane.
#[cfg(target_os = "macos")]
pub fn request_permission(pane: PermissionSettingsPane) -> bool {
	match pane {
		PermissionSettingsPane::Microphone =>
			matches!(request_microphone_permission(), MicrophonePermissionState::Granted),
		PermissionSettingsPane::Accessibility => request_accessibility_permission(),
		PermissionSettingsPane::InputMonitoring => request_input_monitoring_permission(),
	}
}

/// Non-macOS fallback for permission requests.
#[cfg(not(target_os = "macos"))]
pub fn request_permission(_pane: PermissionSettingsPane) -> bool {
	false
}

/// Returns the current microphone authorization state.
#[cfg(target_os = "macos")]
pub fn microphone_permission_state() -> MicrophonePermissionState {
	let media_type_audio = unsafe { AVMediaTypeAudio };
	let status = if let Some(audio_media_type) = media_type_audio {
		unsafe { AVCaptureDevice::authorizationStatusForMediaType(audio_media_type) }
	} else {
		let fallback_media_type = NSString::from_str("soun");
		unsafe { AVCaptureDevice::authorizationStatusForMediaType(fallback_media_type.as_ref()) }
	};

	match status {
		AVAuthorizationStatus::Authorized => MicrophonePermissionState::Granted,
		AVAuthorizationStatus::NotDetermined => MicrophonePermissionState::NotDetermined,
		AVAuthorizationStatus::Denied => MicrophonePermissionState::Denied,
		AVAuthorizationStatus::Restricted => MicrophonePermissionState::Restricted,
		_ => MicrophonePermissionState::Unknown,
	}
}

/// Returns the current microphone authorization state on unsupported platforms.
#[cfg(not(target_os = "macos"))]
pub fn microphone_permission_state() -> MicrophonePermissionState {
	MicrophonePermissionState::Unknown
}

/// Requests microphone authorization prompt.
///
/// Returns the permission status observed before and after scheduling the prompt.
#[cfg(target_os = "macos")]
pub fn request_microphone_permission() -> MicrophonePermissionState {
	let status = microphone_permission_state();
	if !matches!(status, MicrophonePermissionState::NotDetermined) {
		return status;
	}

	let callback = RcBlock::new(|_| {});
	let media_type_audio = unsafe { AVMediaTypeAudio };
	if let Some(audio_media_type) = media_type_audio {
		unsafe {
			AVCaptureDevice::requestAccessForMediaType_completionHandler(
				audio_media_type,
				&callback,
			);
		}
	} else {
		let fallback_media_type = NSString::from_str("soun");
		unsafe {
			AVCaptureDevice::requestAccessForMediaType_completionHandler(
				fallback_media_type.as_ref(),
				&callback,
			);
		}
	}

	MicrophonePermissionState::Prompted
}

/// Returns microphone authorization prompt status on unsupported platforms.
#[cfg(not(target_os = "macos"))]
pub fn request_microphone_permission() -> MicrophonePermissionState {
	MicrophonePermissionState::Unknown
}

/// Capture the frontmost app from macOS Accessibility APIs.
///
/// Returns `None` when the helper cannot access `System Events` or no app is
/// frontmost.
///
/// Required permissions:
/// - Accessibility (for querying `System Events`)
/// - Automation permission for this app to drive AppleScript
#[cfg(target_os = "macos")]
pub fn capture_frontmost_app() -> Option<TargetApp> {
	match capture_frontmost_app_impl() {
		Ok(result) => Some(result),
		Err(err) => {
			tracing::debug!(?err, "failed to capture frontmost app");
			None
		},
	}
}

/// Capture the frontmost app from non-macOS builds.
#[cfg(not(target_os = "macos"))]
pub fn capture_frontmost_app() -> Option<TargetApp> {
	let _ = io::Error::new(io::ErrorKind::Unsupported, "frontmost capture is macOS-only");
	None
}

/// Activate the captured target app and verify it becomes frontmost.
///
/// Returns `true` when an activation attempt succeeded.
#[cfg(target_os = "macos")]
pub fn activate_target(target: &TargetApp, attempts: u32, base_delay: Duration) -> bool {
	let Some(script) = activation_script(target) else {
		return false;
	};
	let mut delay = if base_delay.is_zero() { Duration::from_millis(80) } else { base_delay };

	for attempt_no in 1..=attempts {
		let target_log = target.log_id();
		match execute_applescript_raw(&script) {
			Ok(_) => tracing::debug!(?attempt_no, target=%target_log, "activated target app"),
			Err(err) => {
				tracing::warn!(?attempt_no, ?err, target=%target_log, "activation command failed")
			},
		}

		if capture_frontmost_app().is_some_and(|front| front.matches(target)) {
			return true;
		}

		if attempt_no < attempts {
			sleep(delay);
			delay = delay.saturating_mul(2).min(Duration::from_millis(500));
		}
	}

	false
}

/// Activate helper for non-macOS builds.
#[cfg(not(target_os = "macos"))]
pub fn activate_target(_target: &TargetApp, _attempts: u32, _base_delay: Duration) -> bool {
	false
}

impl TargetApp {
	/// Whether all fields are missing.
	pub fn is_empty(&self) -> bool {
		self.pid.is_none() && self.bundle_id.is_none() && self.app_name.is_none()
	}

	fn log_id(&self) -> String {
		if let Some(bundle_id) = self.bundle_id.as_ref().filter(|value| !value.is_empty()) {
			return bundle_id.clone();
		}

		if let Some(app_name) = self.app_name.as_ref().filter(|value| !value.is_empty()) {
			return app_name.clone();
		}

		"unknown".to_string()
	}

	fn matches(&self, other: &TargetApp) -> bool {
		if self.is_empty() || other.is_empty() {
			return false;
		}

		if let (Some(pid), Some(other_pid)) = (self.pid, other.pid)
			&& pid == other_pid
		{
			return true;
		}

		if let (Some(bundle), Some(other_bundle)) =
			(self.bundle_id.as_deref(), other.bundle_id.as_deref())
			&& !bundle.is_empty()
			&& bundle == other_bundle
		{
			return true;
		}

		if let (Some(name), Some(other_name)) =
			(self.app_name.as_deref(), other.app_name.as_deref())
		{
			return !name.is_empty() && name == other_name;
		}

		false
	}
}

#[cfg(target_os = "macos")]
fn capture_frontmost_app_impl() -> Result<TargetApp, String> {
	let script = r#"tell application "System Events"
		set front_process to first application process whose frontmost is true
		set front_pid to unix id of front_process
		set front_name to name of front_process
		try
			set front_bundle to bundle identifier of front_process
		on error
			set front_bundle to ""
		end try
		return front_pid & "|" & front_bundle & "|" & front_name
	end tell"#;

	let output = execute_applescript_raw(script)?;
	let mut parts = output.splitn(3, '|');
	let pid = parts.next().and_then(parse_u32_trimmed);
	let bundle_id = parts.next().and_then(normalize_optional_string);
	let app_name = parts.next().and_then(normalize_optional_string);

	Ok(TargetApp { pid, bundle_id, app_name })
}

#[cfg(not(target_os = "macos"))]
fn capture_frontmost_app_impl() -> Result<TargetApp, String> {
	Err("frontmost capture is not available on non-macOS".to_string())
}

#[cfg(target_os = "macos")]
fn activation_script(target: &TargetApp) -> Option<String> {
	if let Some(bundle_id) = target.bundle_id.as_deref().filter(|value| !value.is_empty()) {
		return Some(activation_script_for_bundle_id(bundle_id));
	}

	target.app_name.as_deref().filter(|value| !value.is_empty()).map(activation_script_for_app_name)
}

#[cfg(not(target_os = "macos"))]
fn activation_script(_target: &TargetApp) -> Option<String> {
	None
}

#[cfg(target_os = "macos")]
fn execute_applescript_raw(script: &str) -> Result<String, String> {
	let output = Command::new("osascript")
		.arg("-e")
		.arg(script)
		.output()
		.map_err(|err| format!("spawn osascript failed: {err}"))?;

	if !output.status.success() {
		let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
		return Err(format!("osascript failed: {stderr}"));
	}

	let stdout = String::from_utf8_lossy(&output.stdout).to_string();
	Ok(stdout.trim().to_string())
}

#[cfg(not(target_os = "macos"))]
fn execute_applescript_raw(_script: &str) -> Result<String, String> {
	Err("activation is not available on non-macOS".to_string())
}

#[cfg(target_os = "macos")]
fn activation_script_for_bundle_id(bundle_id: &str) -> String {
	let escaped = escape_applescript_string(bundle_id);
	format!(r#"tell application id "{escaped}" to activate"#)
}

#[cfg(target_os = "macos")]
fn activation_script_for_app_name(app_name: &str) -> String {
	let escaped = escape_applescript_string(app_name);
	format!(r#"tell application "{escaped}" to activate"#)
}

#[cfg(target_os = "macos")]
fn escape_applescript_string(input: &str) -> String {
	input.replace('\\', "\\\\").replace('\"', "\\\"")
}

#[cfg(target_os = "macos")]
fn parse_u32_trimmed(raw: &str) -> Option<u32> {
	let value = raw.trim();
	if value.is_empty() { None } else { value.parse::<u32>().ok() }
}

#[cfg(target_os = "macos")]
fn normalize_optional_string(raw: &str) -> Option<String> {
	let trimmed = raw.trim();
	if trimmed.is_empty() { None } else { Some(trimmed.to_string()) }
}

#[cfg(target_os = "macos")]
fn has_accessibility_permission() -> bool {
	let trusted = unsafe { AXIsProcessTrustedWithOptions(ptr::null()) };

	trusted != 0
}

#[cfg(target_os = "macos")]
fn request_accessibility_permission() -> bool {
	let options = accessibility_request_options();

	if options.is_null() {
		return false;
	}

	let requested = unsafe { AXIsProcessTrustedWithOptions(options) };
	let allowed = requested != 0;

	unsafe {
		CFRelease(options as CfTypeRef);
	}

	allowed
}

#[cfg(target_os = "macos")]
fn has_input_monitoring_permission() -> bool {
	unsafe { CGPreflightListenEventAccess() != 0 }
}

#[cfg(target_os = "macos")]
fn request_input_monitoring_permission() -> bool {
	let requested = unsafe { CGRequestListenEventAccess() != 0 };

	requested || has_input_monitoring_permission()
}

#[cfg(target_os = "macos")]
fn accessibility_request_options() -> CfDictionaryRef {
	let key: CfTypeRef = unsafe { kAXTrustedCheckOptionPrompt };
	let value: CfTypeRef = unsafe { kCFBooleanTrue };

	if key.is_null() || value.is_null() {
		return ptr::null();
	}

	let mut keys = [key];
	let mut values = [value];

	unsafe {
		CFDictionaryCreate(
			ptr::null(),
			keys.as_mut_ptr() as *const *const c_void,
			values.as_mut_ptr() as *const *const c_void,
			1,
			ptr::null(),
			ptr::null(),
		)
	}
}

/// Returns whether AVFoundation reports microphone authorization as granted.
#[cfg(target_os = "macos")]
fn has_microphone_permission() -> bool {
	matches!(microphone_permission_state(), MicrophonePermissionState::Granted)
}

#[cfg(target_os = "macos")]
#[link(name = "CoreFoundation", kind = "framework")]
unsafe extern "C" {
	fn CFDictionaryCreate(
		allocator: *const c_void,
		keys: *const *const c_void,
		values: *const *const c_void,
		num_values: CfIndex,
		key_callbacks: *const c_void,
		value_callbacks: *const c_void,
	) -> CfDictionaryRef;
	fn CFRelease(value: CfTypeRef);
	static kCFBooleanTrue: CfTypeRef;
}

#[cfg(target_os = "macos")]
#[link(name = "ApplicationServices", kind = "framework")]
unsafe extern "C" {
	fn AXIsProcessTrustedWithOptions(options: CfDictionaryRef) -> CfBoolean;
	fn CGPreflightListenEventAccess() -> CfBoolean;
	fn CGRequestListenEventAccess() -> CfBoolean;
	static kAXTrustedCheckOptionPrompt: CfTypeRef;
}
