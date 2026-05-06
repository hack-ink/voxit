//! Platform-neutral UI model shared by native hosts.
//!
//! The first native-host slice keeps UI state deliberately small. Runtime services
//! still live in the existing Rust crates, while platform hosts render this model and
//! call back into focused core services as those APIs are promoted behind the host
//! boundary.

use crate::config::Config;

/// Platform family that owns the visible application shell.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PlatformHost {
	/// Native macOS host.
	MacOS,
	/// Unsupported or test-only host.
	Unsupported,
}

/// Authentication method exposed by the first native host.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum AuthMethod {
	/// ChatGPT OAuth device-code authorization.
	ChatGptDeviceCode,
}

/// Authentication state shown by native UI.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum AuthSurfaceState {
	/// The host has not completed an auth status read yet.
	Checking,
	/// No usable ChatGPT session is present.
	SignedOut,
	/// A usable ChatGPT session is present.
	SignedIn,
	/// An auth flow is currently in progress.
	Busy,
}

/// Dictation state shown by native UI.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DictationSurfaceState {
	/// No active recording is in progress.
	Idle,
	/// Microphone capture and pass-1 streaming are active.
	Listening,
	/// Pass-2 finalization is running.
	Finalizing,
	/// Pass-3 rewrite is running.
	Rewriting,
	/// The latest dictation cycle has completed.
	Done,
}

/// Native hotkey behavior shown by UI.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum HotkeySurfaceMode {
	/// Press once to start and press again to stop.
	Toggle,
	/// Hold to record and release to stop.
	Hold,
}
impl HotkeySurfaceMode {
	fn from_config_value(value: &str) -> Self {
		match value {
			"hold" => Self::Hold,
			_ => Self::Toggle,
		}
	}
}

/// Minimal platform-neutral snapshot rendered by native hosts.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct NativeHostSnapshot {
	/// Visible platform shell.
	pub platform: PlatformHost,
	/// Auth method for the first native UI.
	pub auth_method: AuthMethod,
	/// Current auth status.
	pub auth_state: AuthSurfaceState,
	/// Current dictation status.
	pub dictation_state: DictationSurfaceState,
	/// Current hotkey mode.
	pub hotkey_mode: HotkeySurfaceMode,
	/// Suggested panel width.
	pub panel_width_px: u32,
	/// Suggested panel height.
	pub panel_height_px: u32,
	/// True when pass-3 rewrite is enabled by config.
	pub rewrite_enabled: bool,
}
impl NativeHostSnapshot {
	/// Builds the first native-host snapshot from persistent config defaults.
	pub fn initial(platform: PlatformHost, config: &Config) -> Self {
		Self {
			platform,
			auth_method: AuthMethod::ChatGptDeviceCode,
			auth_state: AuthSurfaceState::Checking,
			dictation_state: DictationSurfaceState::Idle,
			hotkey_mode: HotkeySurfaceMode::from_config_value(&config.hotkey.mode),
			panel_width_px: config.ui.panel_width_px,
			panel_height_px: config.ui.panel_height_px,
			rewrite_enabled: config.rewrite.enabled,
		}
	}
}

#[cfg(test)]
mod tests {
	use crate::{
		config::Config,
		ui_model::{HotkeySurfaceMode, NativeHostSnapshot, PlatformHost},
	};

	#[test]
	fn initial_snapshot_uses_configured_dimensions_and_rewrite_flag() {
		let mut config = Config::default();

		config.ui.panel_width_px = 512;
		config.ui.panel_height_px = 320;
		config.rewrite.enabled = false;

		let snapshot = NativeHostSnapshot::initial(PlatformHost::MacOS, &config);

		assert_eq!(snapshot.panel_width_px, 512);
		assert_eq!(snapshot.panel_height_px, 320);
		assert!(!snapshot.rewrite_enabled);
	}

	#[test]
	fn initial_snapshot_maps_hotkey_mode() {
		let mut config = Config::default();

		config.hotkey.mode = "hold".to_string();

		let snapshot = NativeHostSnapshot::initial(PlatformHost::MacOS, &config);

		assert_eq!(snapshot.hotkey_mode, HotkeySurfaceMode::Hold);
	}
}
