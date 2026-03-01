//! Persistent app configuration for Voxit.

use std::{
	fs::{self, File},
	io::{Read, Write},
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

/// Hotkey behavior.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct HotkeyConfig {
	/// Hotkey combination string, e.g. `ctrl+shift+space`.
	pub chord: String,
	/// `toggle` or `hold`.
	pub mode: String,
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

/// OpenAI realtime options.
#[derive(Clone, Debug)]
pub struct OpenAiRealtimeConfig {
	/// Optional noise reduction profile.
	pub noise_reduction: String,
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

/// Paste behavior.
#[derive(Clone, Debug)]
pub struct PasteConfig {
	/// Keep and paste into frontmost target app.
	pub lock_frontmost_app: bool,
	/// Clipboard+CMD+V currently.
	pub method: String,
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

impl Default for UiConfig {
	fn default() -> Self {
		Self { start_hidden: true, panel_width_px: 420, panel_height_px: 260 }
	}
}

impl Default for HotkeyConfig {
	fn default() -> Self {
		Self { chord: "ctrl+shift+space".to_string(), mode: "toggle".to_string() }
	}
}

impl Default for AudioConfig {
	fn default() -> Self {
		Self {
			backend: "voice_processing".to_string(),
			input_sample_rate_hz: 48000,
			input_device_name: String::new(),
			input_device_id: 0,
			realtime_target_rate_hz: 24000,
		}
	}
}

impl Default for OpenAiRealtimeConfig {
	fn default() -> Self {
		Self { noise_reduction: "near_field".to_string() }
	}
}

impl Default for OpenAiConfig {
	fn default() -> Self {
		Self {
			api_base_url: "https://api.openai.com/v1".to_string(),
			realtime_model: "gpt-4o-mini-transcribe".to_string(),
			finalize_model: "gpt-4o-transcribe".to_string(),
			rewrite_model: "gpt-5.2-mini".to_string(),
			language: "en".to_string(),
			realtime: OpenAiRealtimeConfig::default(),
		}
	}
}

impl Default for RewriteConfig {
	fn default() -> Self {
		Self {
			enabled: true,
			auto: true,
			guard_numbers: true,
			max_output_chars: 8000,
			style: "clean".to_string(),
		}
	}
}

impl Default for PasteConfig {
	fn default() -> Self {
		Self { lock_frontmost_app: true, method: "clipboard_cmd_v".to_string() }
	}
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

		let mut file = File::create(&path)
			.map_err(|err| format!("failed to write config file {}: {err}", path.display()))?;
		let content = serialize_toml(self);
		file.write_all(content.as_bytes())
			.map_err(|err| format!("failed to write config {}: {err}", path.display()))?;

		Ok(())
	}
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

		match (section.as_slice(), key) {
			([section], "start_hidden") if *section == "ui" =>
				if let Some(v) = value.bool {
					config.ui.start_hidden = v;
				},
			([section], "panel_width_px") if *section == "ui" =>
				if let Some(v) = value.u32 {
					config.ui.panel_width_px = v;
				},
			([section], "panel_height_px") if *section == "ui" =>
				if let Some(v) = value.u32 {
					config.ui.panel_height_px = v;
				},
			([section], "chord") if *section == "hotkey" =>
				if let Some(v) = value.str {
					config.hotkey.chord = v;
				},
			([section], "mode") if *section == "hotkey" => {
				if let Some(v) = value.str
					&& (v == "toggle" || v == "hold")
				{
					config.hotkey.mode = v;
				}
			},
			([section], "backend") if *section == "audio" => {
				if let Some(v) = value.str
					&& (v == "voice_processing" || v == "cpal")
				{
					config.audio.backend = v;
				}
			},
			([section], "input_sample_rate_hz") if *section == "audio" => {
				if let Some(v) = value.u32 {
					config.audio.input_sample_rate_hz = v;
				}
			},
			([section], "input_device_name") if *section == "audio" =>
				if let Some(v) = value.str {
					config.audio.input_device_name = v;
				},
			([section], "input_device_id") if *section == "audio" =>
				if let Some(v) = value.u32 {
					config.audio.input_device_id = v;
				},
			([section], "realtime_target_rate_hz") if *section == "audio" => {
				if let Some(v) = value.u32 {
					config.audio.realtime_target_rate_hz = v;
				}
			},
			([section], "api_base_url") if *section == "openai" =>
				if let Some(v) = value.str {
					config.openai.api_base_url = v;
				},
			([section], "realtime_model") if *section == "openai" =>
				if let Some(v) = value.str {
					config.openai.realtime_model = v;
				},
			([section], "finalize_model") if *section == "openai" =>
				if let Some(v) = value.str {
					config.openai.finalize_model = v;
				},
			([section], "rewrite_model") if *section == "openai" =>
				if let Some(v) = value.str {
					config.openai.rewrite_model = v;
				},
			([section], "language") if *section == "openai" =>
				if let Some(v) = value.str {
					config.openai.language = v;
				},
			([openai_section, realtime_section], "noise_reduction")
				if openai_section == "openai" && realtime_section == "realtime" =>
			{
				if let Some(v) = value.str
					&& (v == "near_field" || v == "far_field" || v == "off")
				{
					config.openai.realtime.noise_reduction = v;
				}
			},
			([section], "enabled") if *section == "rewrite" =>
				if let Some(v) = value.bool {
					config.rewrite.enabled = v;
				},
			([section], "auto") if *section == "rewrite" =>
				if let Some(v) = value.bool {
					config.rewrite.auto = v;
				},
			([section], "guard_numbers") if *section == "rewrite" =>
				if let Some(v) = value.bool {
					config.rewrite.guard_numbers = v;
				},
			([section], "max_output_chars") if *section == "rewrite" =>
				if let Some(v) = value.u32 {
					config.rewrite.max_output_chars = v;
				},
			([section], "style") if *section == "rewrite" =>
				if let Some(v) = value.str {
					config.rewrite.style = v;
				},
			([section], "lock_frontmost_app") if *section == "paste" => {
				if let Some(v) = value.bool {
					config.paste.lock_frontmost_app = v;
				}
			},
			([section], "method") if *section == "paste" =>
				if let Some(v) = value.str {
					config.paste.method = v;
				},
			_ => {},
		}
	}

	Some(config)
}

#[derive(Default)]
struct ParsedValue {
	bool: Option<bool>,
	u32: Option<u32>,
	str: Option<String>,
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
	output
		.push_str(&format!("noise_reduction = \"{}\"\n\n", config.openai.realtime.noise_reduction));

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
	use super::*;

	#[test]
	fn parse_default_like_content() {
		let parsed = parse_toml(
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
realtime_model = "gpt-4o-mini-transcribe"
finalize_model = "gpt-4o-transcribe"
rewrite_model = "gpt-5.2-mini"
language = "en"

[openai.realtime]
noise_reduction = "near_field"

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
	}

	#[test]
	fn serialize_produces_toml() {
		let config = Config::default();
		let raw = serialize_toml(&config);
		let parsed = parse_toml(raw).expect("should parse serialized value");
		assert_eq!(parsed.ui.panel_width_px, 420);
		assert_eq!(parsed.paste.method, "clipboard_cmd_v");
		assert_eq!(parsed.audio.input_device_id, 0);
		assert!(parsed.audio.input_device_name.is_empty());
	}
}
