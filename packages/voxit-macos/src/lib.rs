//! macOS target app capture and activation helpers.

#[cfg(not(target_os = "macos"))] use std::io::{self, Error, ErrorKind};
#[cfg(target_os = "macos")] use std::ptr;
use std::{ffi::c_void, mem, thread, time::Duration};
#[cfg(target_os = "macos")] use std::{
	io::Write as _,
	process::{Command, Stdio},
};

#[cfg(target_os = "macos")] use block2::RcBlock;
#[cfg(target_os = "macos")]
use objc2_app_kit::{NSApplicationActivationOptions, NSRunningApplication};
#[cfg(target_os = "macos")]
use objc2_av_foundation::{AVAuthorizationStatus, AVCaptureDevice, AVMediaTypeAudio};
use url::Url;

#[cfg(target_os = "macos")]
type CfDictionaryRef = *const c_void;
#[cfg(target_os = "macos")]
type CfTypeRef = *const c_void;

/// Captured frontmost application metadata.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct TargetApp {
	/// Process identifier reported by macOS.
	pub pid: Option<u32>,
	/// Bundle identifier (for example, `com.apple.Safari`).
	pub bundle_id: Option<String>,
	/// Frontmost application name.
	pub app_name: Option<String>,
	/// Focused window title when available.
	pub window_title: Option<String>,
	/// Browser or webview URL domain when available.
	pub url_domain: Option<String>,
	/// Focused accessibility element role when available.
	pub focused_element_role: Option<String>,
	/// Whether selected text was present when capture started.
	pub selected_text_present: bool,
}
impl TargetApp {
	/// Whether all fields are missing.
	pub fn is_empty(&self) -> bool {
		self.pid.is_none()
			&& self.bundle_id.is_none()
			&& self.app_name.is_none()
			&& self.window_title.is_none()
			&& self.url_domain.is_none()
			&& self.focused_element_role.is_none()
			&& !self.selected_text_present
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

	fn matches(&self, other: &Self) -> bool {
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

/// Permission-related privacy panes exposed in the onboarding checklist.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PermissionSettingsPane {
	/// Microphone privacy settings.
	Microphone,
	/// Accessibility privacy settings.
	Accessibility,
}
impl PermissionSettingsPane {
	/// Human-friendly label for status text.
	pub const fn display_name(self) -> &'static str {
		match self {
			Self::Microphone => "Microphone",
			Self::Accessibility => "Accessibility",
		}
	}
}

/// Current microphone authorization state reported by macOS.
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

/// Activate the current app and bring Voxit to the foreground.
#[cfg(target_os = "macos")]
pub fn activate_current_application() -> bool {
	let app = NSRunningApplication::currentApplication();
	#[allow(deprecated)]
	let activated = app.activateWithOptions(
		NSApplicationActivationOptions::ActivateAllWindows
			| NSApplicationActivationOptions::ActivateIgnoringOtherApps,
	);

	if !activated {
		tracing::warn!("failed to activate current application");
	}

	activated
}

/// Activation helper for non-macOS builds.
#[cfg(not(target_os = "macos"))]
pub fn activate_current_application() -> bool {
	false
}

/// Check whether a permission pane is currently granted.
#[cfg(target_os = "macos")]
pub fn permission_is_granted(pane: PermissionSettingsPane) -> bool {
	match pane {
		PermissionSettingsPane::Microphone => has_microphone_permission(),
		PermissionSettingsPane::Accessibility => has_accessibility_permission(),
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
		PermissionSettingsPane::Microphone => {
			matches!(request_microphone_permission(), MicrophonePermissionState::Granted)
		},
		PermissionSettingsPane::Accessibility => request_accessibility_permission(),
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
	let Some(audio_media_type) = media_type_audio else {
		tracing::warn!(
			"AVMediaTypeAudio symbol is unavailable; microphone permission state unknown"
		);

		return MicrophonePermissionState::Unknown;
	};
	let status = unsafe { AVCaptureDevice::authorizationStatusForMediaType(audio_media_type) };

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

	tracing::info!(?status, "microphone permission request invoked");

	if !matches!(status, MicrophonePermissionState::NotDetermined) {
		return status;
	}

	let callback = RcBlock::new(|_| {});
	let media_type_audio = unsafe { AVMediaTypeAudio };
	let Some(audio_media_type) = media_type_audio else {
		tracing::warn!(
			"AVMediaTypeAudio symbol is unavailable; skipping microphone prompt request"
		);

		return MicrophonePermissionState::Unknown;
	};

	unsafe {
		AVCaptureDevice::requestAccessForMediaType_completionHandler(audio_media_type, &callback);
	}

	// `requestAccessForMediaType:completionHandler:` completes asynchronously and invokes the
	// handler on an arbitrary queue. In practice this should copy/retain the completion block, but
	// to avoid any lifetime mismatch across FFI boundaries we intentionally keep the block alive
	// for the remainder of the process.
	mem::forget(callback);

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
	let _ = Error::new(ErrorKind::Unsupported, "frontmost capture is macOS-only");

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
			thread::sleep(delay);

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

/// Copy text to the clipboard and dispatch a paste gesture into the selected target.
#[cfg(target_os = "macos")]
pub fn paste_text(target: Option<&TargetApp>, text: &str, lock_target: bool) -> Result<(), String> {
	if text.is_empty() {
		return Err("nothing to paste".to_string());
	}
	if lock_target && let Some(target) = target {
		let _ = activate_target(target, 3, Duration::from_millis(80));
	}

	copy_to_clipboard(text)?;

	dispatch_command_v()
}

/// Paste helper for non-macOS builds.
#[cfg(not(target_os = "macos"))]
pub fn paste_text(
	_target: Option<&TargetApp>,
	_text: &str,
	_lock_target: bool,
) -> Result<(), String> {
	Err("paste is only supported on macOS in this build".to_string())
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
			set front_window_title to ""
			try
				set front_window_title to name of front window of front_process
			end try
			set focused_role to ""
			set selected_text_present to "false"
			try
				set focused_element to value of attribute "AXFocusedUIElement" of front_process
				try
					set focused_role to role of focused_element
				end try
				try
					set selected_text to value of attribute "AXSelectedText" of focused_element
					if selected_text is not missing value and selected_text is not "" then
						set selected_text_present to "true"
					end if
				end try
			end try
			return front_pid & "|" & front_bundle & "|" & front_name & "|" & front_window_title & "|" & focused_role & "|" & selected_text_present
		end tell"#;
	let output = execute_applescript_raw(script)?;
	let mut parts = output.splitn(6, '|');
	let pid = parts.next().and_then(parse_u32_trimmed);
	let bundle_id = parts.next().and_then(normalize_optional_string);
	let app_name = parts.next().and_then(normalize_optional_string);
	let window_title = parts.next().and_then(normalize_optional_string);
	let focused_element_role = parts.next().and_then(normalize_optional_string);
	let selected_text_present = parts.next().is_some_and(|raw| raw.trim() == "true");
	let url_domain = capture_url_domain(bundle_id.as_deref(), app_name.as_deref());

	Ok(TargetApp {
		pid,
		bundle_id,
		app_name,
		window_title,
		url_domain,
		focused_element_role,
		selected_text_present,
	})
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

#[cfg(target_os = "macos")]
fn copy_to_clipboard(text: &str) -> Result<(), String> {
	let mut child = Command::new("pbcopy")
		.stdin(Stdio::piped())
		.spawn()
		.map_err(|err| format!("spawn pbcopy failed: {err}"))?;
	let Some(stdin) = child.stdin.as_mut() else {
		return Err("pbcopy stdin unavailable".to_string());
	};

	stdin.write_all(text.as_bytes()).map_err(|err| format!("write pbcopy failed: {err}"))?;

	let status = child.wait().map_err(|err| format!("wait pbcopy failed: {err}"))?;

	if status.success() { Ok(()) } else { Err(format!("pbcopy failed with status {status}")) }
}

#[cfg(target_os = "macos")]
fn dispatch_command_v() -> Result<(), String> {
	execute_applescript_raw(
		r#"tell application "System Events" to keystroke "v" using command down"#,
	)
	.map(|_| ())
}

#[cfg(target_os = "macos")]
fn capture_url_domain(bundle_id: Option<&str>, app_name: Option<&str>) -> Option<String> {
	let script = browser_url_script(bundle_id, app_name)?;
	let url = execute_applescript_raw(&script).ok()?;

	parse_domain(&url)
}

#[cfg(target_os = "macos")]
fn browser_url_script(bundle_id: Option<&str>, app_name: Option<&str>) -> Option<String> {
	let identity = bundle_id.or(app_name)?.to_ascii_lowercase();

	if identity.contains("safari") {
		return Some(r#"tell application "Safari" to return URL of front document"#.to_string());
	}
	if identity.contains("chrome") {
		return Some(
			r#"tell application "Google Chrome" to return URL of active tab of front window"#
				.to_string(),
		);
	}
	if identity.contains("microsoft.edgemac") || identity.contains("microsoft edge") {
		return Some(
			r#"tell application "Microsoft Edge" to return URL of active tab of front window"#
				.to_string(),
		);
	}
	if identity.contains("company.thebrowser.browser") || identity.contains("arc") {
		return Some(
			r#"tell application "Arc" to return URL of active tab of front window"#.to_string(),
		);
	}

	None
}

fn parse_domain(raw_url: &str) -> Option<String> {
	let url = Url::parse(raw_url.trim()).ok()?;

	url.host_str().map(|domain| domain.trim_start_matches("www.").to_ascii_lowercase())
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
	tracing::info!("accessibility permission request invoked");

	let options = accessibility_request_options();

	if options.is_null() {
		tracing::warn!("accessibility request options unavailable");

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
		num_values: isize,
		key_callbacks: *const c_void,
		value_callbacks: *const c_void,
	) -> CfDictionaryRef;
	fn CFRelease(value: CfTypeRef);
	static kCFBooleanTrue: CfTypeRef;
}

#[cfg(target_os = "macos")]
#[link(name = "ApplicationServices", kind = "framework")]
unsafe extern "C" {
	fn AXIsProcessTrustedWithOptions(options: CfDictionaryRef) -> u8;
	static kAXTrustedCheckOptionPrompt: CfTypeRef;
}
