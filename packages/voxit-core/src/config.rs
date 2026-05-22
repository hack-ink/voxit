//! Persistent app configuration for Voxit.

use std::{
	fs::{self, File},
	io::{Read as _, Write as _},
	path::PathBuf,
};

use directories::ProjectDirs;

/// Frontend UI settings.
#[derive(Clone, Debug)]
pub struct UiConfig {
	/// Show panel minimized by default.
	pub start_hidden: bool,
	/// Suggested panel width.
	pub panel_width_px: u32,
	/// Suggested panel height.
	pub panel_height_px: u32,
}
impl Default for UiConfig {
	fn default() -> Self {
		Self { start_hidden: true, panel_width_px: 420, panel_height_px: 260 }
	}
}

/// Hotkey behavior.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct HotkeyConfig {
	/// Hotkey combination string, e.g. `ctrl+shift+space`.
	pub chord: String,
	/// `toggle` or `hold`.
	pub mode: String,
}
impl Default for HotkeyConfig {
	fn default() -> Self {
		Self { chord: "ctrl+shift+space".to_string(), mode: "toggle".to_string() }
	}
}

/// Microphone capture options.
#[derive(Clone, Debug)]
pub struct AudioConfig {
	/// `voice_processing` or `cpal`.
	pub backend: String,
	/// Captured device sample rate in Hz.
	pub input_sample_rate_hz: u32,
	/// Human-readable selected input microphone name.
	pub input_device_name: String,
	/// Selected input microphone CoreAudio id, `0` for default.
	pub input_device_id: u32,
	/// Target sample rate used by Pass1 and model APIs.
	pub realtime_target_rate_hz: u32,
}
impl Default for AudioConfig {
	fn default() -> Self {
		Self {
			backend: "voice_processing".to_string(),
			input_sample_rate_hz: 48_000,
			input_device_name: String::new(),
			input_device_id: 0,
			realtime_target_rate_hz: 24_000,
		}
	}
}

/// OpenAI API model settings.
#[derive(Clone, Debug)]
pub struct OpenAiConfig {
	/// Base API URL.
	pub api_base_url: String,
	/// Realtime pass model.
	pub realtime_model: String,
	/// Finalize pass model.
	pub finalize_model: String,
	/// Rewrite pass model.
	pub rewrite_model: String,
	/// Language code.
	pub language: String,
	/// Realtime nested options.
	pub realtime: OpenAiRealtimeConfig,
}
impl Default for OpenAiConfig {
	fn default() -> Self {
		Self {
			api_base_url: "https://api.openai.com/v1".to_string(),
			realtime_model: "gpt-realtime-2".to_string(),
			finalize_model: "gpt-4o-transcribe".to_string(),
			rewrite_model: "gpt-5.2-mini".to_string(),
			language: "en".to_string(),
			realtime: OpenAiRealtimeConfig::default(),
		}
	}
}

/// OpenAI realtime options.
#[derive(Clone, Debug)]
pub struct OpenAiRealtimeConfig {
	/// Optional noise reduction profile.
	pub noise_reduction: String,
	/// Input-audio transcription model used for realtime Pass1 transcript events.
	pub transcription_model: String,
}
impl Default for OpenAiRealtimeConfig {
	fn default() -> Self {
		Self {
			noise_reduction: "near_field".to_string(),
			transcription_model: "gpt-4o-mini-transcribe".to_string(),
		}
	}
}

/// Rewrite behavior.
#[derive(Clone, Debug)]
pub struct RewriteConfig {
	/// Enable rewrite pipeline.
	pub enabled: bool,
	/// Auto-run rewrite once finalize returns.
	pub auto: bool,
	/// Keep numeric/date/currency tokens unchanged.
	pub guard_numbers: bool,
	/// Rewrite output character limit.
	pub max_output_chars: u32,
	/// Rewrite style preset.
	pub style: String,
}
impl Default for RewriteConfig {
	fn default() -> Self {
		Self {
			enabled: true,
			auto: true,
			guard_numbers: true,
			max_output_chars: 8_000,
			style: "clean".to_string(),
		}
	}
}

/// Paste behavior.
#[derive(Clone, Debug)]
pub struct PasteConfig {
	/// Keep and paste into frontmost target app.
	pub lock_frontmost_app: bool,
	/// Clipboard+CMD+V currently.
	pub method: String,
}
impl Default for PasteConfig {
	fn default() -> Self {
		Self { lock_frontmost_app: true, method: "clipboard_cmd_v".to_string() }
	}
}

/// Root app configuration.
#[derive(Clone, Debug, Default)]
pub struct Config {
	/// UI config.
	pub ui: UiConfig,
	/// Hotkey config.
	pub hotkey: HotkeyConfig,
	/// Audio config.
	pub audio: AudioConfig,
	/// OpenAI config.
	pub openai: OpenAiConfig,
	/// Rewrite config.
	pub rewrite: RewriteConfig,
	/// Paste config.
	pub paste: PasteConfig,
}
impl Config {
	/// Returns default config path at `<Application Support>/voxit/config.toml`.
	pub fn config_path() -> Result<PathBuf, String> {
		let dirs = ProjectDirs::from("", "hack.ink", "voxit")
			.ok_or_else(|| "failed to resolve project dirs".to_string())?;

		Ok(dirs.config_dir().join("config.toml"))
	}

	/// Loads config from `config.toml`, or returns defaults when file is missing.
	pub fn load() -> Result<Self, String> {
		let path = Self::config_path()?;

		if !path.exists() {
			return Ok(Self::default());
		}

		let mut raw = String::new();

		File::open(&path)
			.map_err(|err| format!("failed to open {}: {err}", path.display()))?
			.read_to_string(&mut raw)
			.map_err(|err| format!("failed to read {}: {err}", path.display()))?;

		parse_toml(raw).ok_or_else(|| "invalid config.toml".to_string())
	}

	/// Writes current config to `config.toml`.
	pub fn save(&self) -> Result<(), String> {
		let path = Self::config_path()?;

		if let Some(parent) = path.parent() {
			fs::create_dir_all(parent).map_err(|err| {
				format!("failed to create config directory {}: {err}", parent.display())
			})?;
		}

		let content = serialize_toml(self);
		let mut file = File::create(&path)
			.map_err(|err| format!("failed to write config file {}: {err}", path.display()))?;

		file.write_all(content.as_bytes())
			.map_err(|err| format!("failed to write config {}: {err}", path.display()))?;

		Ok(())
	}
}

#[derive(Default)]
struct ParsedValue {
	bool: Option<bool>,
	u32: Option<u32>,
	str: Option<String>,
}

fn parse_toml(raw: String) -> Option<Config> {
	let mut config = Config::default();
	let mut section: Vec<String> = Vec::new();

	for line in raw.lines() {
		let line = line.trim();

		if line.is_empty() || line.starts_with('#') {
			continue;
		}

		if let Some(name) = line.strip_prefix('[').and_then(|s| s.strip_suffix(']')) {
			section = name.split('.').map(str::to_string).collect();

			continue;
		}

		let Some((raw_key, raw_value)) = line.split_once('=') else {
			continue;
		};
		let key = raw_key.trim();
		let value = parse_value(raw_value.trim());

		apply_config_value(&mut config, section.as_slice(), key, &value);
	}

	Some(config)
}

fn apply_config_value(config: &mut Config, section: &[String], key: &str, value: &ParsedValue) {
	if apply_ui_config(config, section, key, value) {
		return;
	}
	if apply_hotkey_config(config, section, key, value) {
		return;
	}
	if apply_audio_config(config, section, key, value) {
		return;
	}
	if apply_openai_config(config, section, key, value) {
		return;
	}
	if apply_rewrite_config(config, section, key, value) {
		return;
	}

	let _ = apply_paste_config(config, section, key, value);
}

fn apply_ui_config(
	config: &mut Config,
	section: &[String],
	key: &str,
	value: &ParsedValue,
) -> bool {
	let [section] = section else {
		return false;
	};

	if section.as_str() != "ui" {
		return false;
	}

	match key {
		"start_hidden" =>
			if let Some(v) = value.bool {
				config.ui.start_hidden = v;
			},
		"panel_width_px" =>
			if let Some(v) = value.u32 {
				config.ui.panel_width_px = v;
			},
		"panel_height_px" =>
			if let Some(v) = value.u32 {
				config.ui.panel_height_px = v;
			},
		_ => return false,
	}

	true
}

fn apply_hotkey_config(
	config: &mut Config,
	section: &[String],
	key: &str,
	value: &ParsedValue,
) -> bool {
	let [section] = section else {
		return false;
	};

	if section.as_str() != "hotkey" {
		return false;
	}

	match key {
		"chord" =>
			if let Some(v) = value.str.clone() {
				config.hotkey.chord = v;
			},
		"mode" => {
			if let Some(v) = value.str.as_deref()
				&& (v == "toggle" || v == "hold")
			{
				config.hotkey.mode = v.to_string();
			}
		},
		_ => return false,
	}

	true
}

fn apply_audio_config(
	config: &mut Config,
	section: &[String],
	key: &str,
	value: &ParsedValue,
) -> bool {
	let [section] = section else {
		return false;
	};

	if section.as_str() != "audio" {
		return false;
	}

	match key {
		"backend" => {
			if let Some(v) = value.str.as_deref()
				&& (v == "voice_processing" || v == "cpal")
			{
				config.audio.backend = v.to_string();
			}
		},
		"input_sample_rate_hz" =>
			if let Some(v) = value.u32 {
				config.audio.input_sample_rate_hz = v;
			},
		"input_device_name" =>
			if let Some(v) = value.str.clone() {
				config.audio.input_device_name = v;
			},
		"input_device_id" =>
			if let Some(v) = value.u32 {
				config.audio.input_device_id = v;
			},
		"realtime_target_rate_hz" =>
			if let Some(v) = value.u32 {
				config.audio.realtime_target_rate_hz = v;
			},
		_ => return false,
	}

	true
}

fn apply_openai_config(
	config: &mut Config,
	section: &[String],
	key: &str,
	value: &ParsedValue,
) -> bool {
	match (section, key) {
		([section], "api_base_url") if section == "openai" =>
			if let Some(v) = value.str.clone() {
				config.openai.api_base_url = v;
			},
		([section], "realtime_model") if section == "openai" =>
			if let Some(v) = value.str.clone() {
				config.openai.realtime_model = v;
			},
		([section], "finalize_model") if section == "openai" =>
			if let Some(v) = value.str.clone() {
				config.openai.finalize_model = v;
			},
		([section], "rewrite_model") if section == "openai" =>
			if let Some(v) = value.str.clone() {
				config.openai.rewrite_model = v;
			},
		([section], "language") if section == "openai" =>
			if let Some(v) = value.str.clone() {
				config.openai.language = v;
			},
		([openai_section, realtime_section], "noise_reduction")
			if openai_section == "openai" && realtime_section == "realtime" =>
		{
			if let Some(v) = value.str.as_deref()
				&& (v == "near_field" || v == "far_field" || v == "off")
			{
				config.openai.realtime.noise_reduction = v.to_string();
			}
		},
		([openai_section, realtime_section], "transcription_model")
			if openai_section == "openai" && realtime_section == "realtime" =>
		{
			if let Some(v) = value.str.clone() {
				config.openai.realtime.transcription_model = v;
			}
		},
		_ => return false,
	}

	true
}

fn apply_rewrite_config(
	config: &mut Config,
	section: &[String],
	key: &str,
	value: &ParsedValue,
) -> bool {
	let [section] = section else {
		return false;
	};

	if section.as_str() != "rewrite" {
		return false;
	}

	match key {
		"enabled" =>
			if let Some(v) = value.bool {
				config.rewrite.enabled = v;
			},
		"auto" =>
			if let Some(v) = value.bool {
				config.rewrite.auto = v;
			},
		"guard_numbers" =>
			if let Some(v) = value.bool {
				config.rewrite.guard_numbers = v;
			},
		"max_output_chars" =>
			if let Some(v) = value.u32 {
				config.rewrite.max_output_chars = v;
			},
		"style" =>
			if let Some(v) = value.str.clone() {
				config.rewrite.style = v;
			},
		_ => return false,
	}

	true
}

fn apply_paste_config(
	config: &mut Config,
	section: &[String],
	key: &str,
	value: &ParsedValue,
) -> bool {
	let [section] = section else {
		return false;
	};

	if section.as_str() != "paste" {
		return false;
	}

	match key {
		"lock_frontmost_app" =>
			if let Some(v) = value.bool {
				config.paste.lock_frontmost_app = v;
			},
		"method" =>
			if let Some(v) = value.str.clone() {
				config.paste.method = v;
			},
		_ => return false,
	}

	true
}

fn parse_value(raw: &str) -> ParsedValue {
	let text = raw.trim();

	if text == "true" {
		return ParsedValue { bool: Some(true), ..Default::default() };
	}
	if text == "false" {
		return ParsedValue { bool: Some(false), ..Default::default() };
	}

	if let Ok(v) = text.parse::<u32>() {
		return ParsedValue { u32: Some(v), ..Default::default() };
	}
	if let Some(raw) = text.strip_prefix('"').and_then(|v| v.strip_suffix('"')) {
		return ParsedValue { str: Some(raw.to_string()), ..Default::default() };
	}
	if let Some(raw) = text.strip_prefix('\'').and_then(|v| v.strip_suffix('\'')) {
		return ParsedValue { str: Some(raw.to_string()), ..Default::default() };
	}

	ParsedValue { str: Some(text.to_string()), ..Default::default() }
}

fn serialize_toml(config: &Config) -> String {
	let mut output = String::new();

	output.push_str("[ui]\n");
	output.push_str(&format!("start_hidden = {}\n", config.ui.start_hidden));
	output.push_str(&format!("panel_width_px = {}\n", config.ui.panel_width_px));
	output.push_str(&format!("panel_height_px = {}\n\n", config.ui.panel_height_px));
	output.push_str("[hotkey]\n");
	output.push_str(&format!("chord = \"{}\"\n", config.hotkey.chord));
	output.push_str(&format!("mode = \"{}\"\n\n", config.hotkey.mode));
	output.push_str("[audio]\n");
	output.push_str(&format!("backend = \"{}\"\n", config.audio.backend));
	output.push_str(&format!("input_sample_rate_hz = {}\n", config.audio.input_sample_rate_hz));
	output.push_str(&format!("input_device_name = \"{}\"\n", config.audio.input_device_name));
	output.push_str(&format!("input_device_id = {}\n", config.audio.input_device_id));
	output.push_str(&format!(
		"realtime_target_rate_hz = {}\n\n",
		config.audio.realtime_target_rate_hz
	));
	output.push_str("[openai]\n");
	output.push_str(&format!("api_base_url = \"{}\"\n", config.openai.api_base_url));
	output.push_str(&format!("realtime_model = \"{}\"\n", config.openai.realtime_model));
	output.push_str(&format!("finalize_model = \"{}\"\n", config.openai.finalize_model));
	output.push_str(&format!("rewrite_model = \"{}\"\n", config.openai.rewrite_model));
	output.push_str(&format!("language = \"{}\"\n\n", config.openai.language));
	output.push_str("[openai.realtime]\n");
	output.push_str(&format!("noise_reduction = \"{}\"\n", config.openai.realtime.noise_reduction));
	output.push_str(&format!(
		"transcription_model = \"{}\"\n\n",
		config.openai.realtime.transcription_model
	));
	output.push_str("[rewrite]\n");
	output.push_str(&format!("enabled = {}\n", config.rewrite.enabled));
	output.push_str(&format!("auto = {}\n", config.rewrite.auto));
	output.push_str(&format!("guard_numbers = {}\n", config.rewrite.guard_numbers));
	output.push_str(&format!("max_output_chars = {}\n", config.rewrite.max_output_chars));
	output.push_str(&format!("style = \"{}\"\n\n", config.rewrite.style));
	output.push_str("[paste]\n");
	output.push_str(&format!("lock_frontmost_app = {}\n", config.paste.lock_frontmost_app));
	output.push_str(&format!("method = \"{}\"\n", config.paste.method));

	output
}

#[cfg(test)]
mod tests {
	use crate::config::{self, Config};

	#[test]
	fn parse_default_like_content() {
		let parsed = config::parse_toml(
			r#"
[ui]
start_hidden = false
panel_width_px = 500
panel_height_px = 300

[hotkey]
chord = "cmd+shift+space"
mode = "hold"

	[audio]
	backend = "cpal"
	input_sample_rate_hz = 48000
	input_device_name = "USB Mic"
	input_device_id = 123
	realtime_target_rate_hz = 24000

[openai]
api_base_url = "https://api.openai.com/v1"
realtime_model = "gpt-realtime-2"
finalize_model = "gpt-4o-transcribe"
rewrite_model = "gpt-5.2-mini"
language = "en"

[openai.realtime]
noise_reduction = "near_field"
transcription_model = "gpt-4o-mini-transcribe"

[rewrite]
enabled = false
auto = true
guard_numbers = true
max_output_chars = 9000
style = "clean"

[paste]
lock_frontmost_app = true
method = "clipboard_cmd_v"
"#
			.to_string(),
		)
		.expect("valid sample");

		assert!(!parsed.ui.start_hidden);
		assert_eq!(parsed.hotkey.mode, "hold");
		assert_eq!(parsed.audio.backend, "cpal");
		assert_eq!(parsed.audio.input_device_name, "USB Mic");
		assert_eq!(parsed.audio.input_device_id, 123);
		assert_eq!(parsed.openai.realtime.noise_reduction, "near_field");
		assert_eq!(parsed.openai.realtime.transcription_model, "gpt-4o-mini-transcribe");
	}

	#[test]
	fn serialize_produces_toml() {
		let config = Config::default();
		let raw = config::serialize_toml(&config);
		let parsed = config::parse_toml(raw).expect("should parse serialized value");

		assert_eq!(parsed.ui.panel_width_px, 420);
		assert_eq!(parsed.paste.method, "clipboard_cmd_v");
		assert_eq!(parsed.audio.input_device_id, 0);
		assert!(parsed.audio.input_device_name.is_empty());
		assert_eq!(parsed.openai.realtime_model, "gpt-realtime-2");
	}
}
