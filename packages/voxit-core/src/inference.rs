//! Provider-routed transcription and rewrite pipeline.

use std::sync::mpsc::{Receiver, Sender};
#[cfg(target_os = "macos")] use std::{collections::BTreeMap, time::Instant};

#[cfg(target_os = "macos")]
use crate::providers::{self, InferenceProvider, RewriteRequest, TranscriptionRequest};
use crate::{
	ContextualVoiceRouter, FocusedAppContext, VoiceSessionPlan,
	providers::chatgpt::ChatGptProvider,
	realtime::{RealtimeError, RealtimeEvent, RealtimeSession, RealtimeSessionConfig},
};
use voxit_audio::AudioChunk;

/// Rewrite outcome status.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RewriteState {
	/// Rewrite was intentionally skipped before request.
	Skipped,
	/// Rewrite succeeded and output passed all safety checks.
	Applied,
	/// Rewrite was returned but rejected due to protected token mismatch.
	Rejected,
}

/// Guarded rewrite result payload.
#[derive(Clone, Debug)]
pub struct RewriteResult {
	/// Optional rewritten transcript when state is `Applied`.
	pub rewritten_transcript: Option<String>,
	/// Rewrite decision.
	pub state: RewriteState,
	/// Optional reason for skipped or rejected rewrite.
	pub reason: Option<String>,
}

/// Settings applied to a contextual rewrite pass.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RewriteSettings {
	/// Preserve numeric, date, and currency tokens.
	pub guard_protected_tokens: bool,
	/// Maximum accepted rewritten output length.
	pub max_output_chars: u32,
	/// Style preset supplied to prompt construction.
	pub style: String,
	/// Optional user glossary terms to preserve or prefer.
	pub glossary_terms: String,
}
impl Default for RewriteSettings {
	fn default() -> Self {
		Self {
			guard_protected_tokens: true,
			max_output_chars: 8_000,
			style: "clean".to_string(),
			glossary_terms: String::new(),
		}
	}
}

/// Background event sent to the UI thread.
#[derive(Debug)]
pub enum InferenceEvent {
	/// Pass2 transcription completed with raw transcript text.
	Pass2Completed {
		/// Pass2 duration in milliseconds.
		total_ms: u64,
		/// Raw transcript text.
		raw_transcript: String,
	},
	/// Pass3 rewrite completed (or rejected by guard).
	RewriteCompleted {
		/// Pass3 duration in milliseconds.
		total_ms: u64,
		/// Rewrite result.
		result: RewriteResult,
	},
	/// Pipeline failed with an error.
	Failed(String),
}

/// Start the configured realtime transcription provider.
#[cfg(target_os = "macos")]
pub fn start_realtime_session(
	config: RealtimeSessionConfig,
	chunk_rx: Receiver<AudioChunk>,
	event_tx: Sender<RealtimeEvent>,
) -> Result<RealtimeSession, RealtimeError> {
	let provider = default_provider().map_err(|err| RealtimeError::RuntimeError {
		reason: format!("ChatGPT OAuth provider unavailable: {err}"),
	})?;

	provider.start_realtime_session(config, chunk_rx, event_tx)
}

/// Realtime inference is unavailable on non-macOS placeholder builds.
#[cfg(not(target_os = "macos"))]
pub fn start_realtime_session(
	config: RealtimeSessionConfig,
	chunk_rx: Receiver<AudioChunk>,
	event_tx: Sender<RealtimeEvent>,
) -> Result<RealtimeSession, RealtimeError> {
	let _ = config;
	let _ = chunk_rx;
	let _ = event_tx;

	Err(RealtimeError::DependencyUnavailable {
		reason: "inference pipeline is only enabled on macOS builds".to_string(),
	})
}

/// Transcribes WAV bytes using the configured Pass2 provider.
#[cfg(target_os = "macos")]
pub fn transcribe_only(wav: &[u8], model: &str) -> Result<(String, u64), String> {
	let started = Instant::now();
	let provider = default_provider()?;
	let raw = provider.transcribe(TranscriptionRequest { wav, model })?;

	Ok((raw, started.elapsed().as_millis() as u64))
}

/// Inference pipeline is unavailable on non-macOS placeholder builds.
#[cfg(not(target_os = "macos"))]
pub fn transcribe_only(_wav: &[u8], _model: &str) -> Result<(String, u64), String> {
	Err("inference pipeline is only enabled on macOS builds.".to_string())
}

/// Rewrites transcript text with protected-token guard checks.
#[cfg(target_os = "macos")]
pub fn rewrite_only(text: &str, model: &str) -> Result<(RewriteResult, u64), String> {
	let plan = ContextualVoiceRouter.plan_for_context(&FocusedAppContext::new());

	rewrite_only_with_plan(text, model, &plan, &RewriteSettings::default())
}

/// Rewrites transcript text with the selected contextual voice plan.
#[cfg(target_os = "macos")]
pub fn rewrite_only_with_plan(
	text: &str,
	model: &str,
	plan: &VoiceSessionPlan,
	settings: &RewriteSettings,
) -> Result<(RewriteResult, u64), String> {
	if text.trim().is_empty() {
		return Ok((
			RewriteResult {
				rewritten_transcript: None,
				state: RewriteState::Skipped,
				reason: Some("empty transcript; rewrite skipped".to_string()),
			},
			0,
		));
	}

	let started = Instant::now();
	let result = rewrite_with_guard(text, model, plan, settings)?;

	Ok((result, started.elapsed().as_millis() as u64))
}

/// Inference pipeline is unavailable on non-macOS placeholder builds.
#[cfg(not(target_os = "macos"))]
pub fn rewrite_only(_text: &str, _model: &str) -> Result<(RewriteResult, u64), String> {
	Err("inference pipeline is only enabled on macOS builds.".to_string())
}

/// Inference pipeline is unavailable on non-macOS placeholder builds.
#[cfg(not(target_os = "macos"))]
pub fn rewrite_only_with_plan(
	_text: &str,
	_model: &str,
	_plan: &VoiceSessionPlan,
	_settings: &RewriteSettings,
) -> Result<(RewriteResult, u64), String> {
	Err("inference pipeline is only enabled on macOS builds.".to_string())
}

#[cfg(target_os = "macos")]
fn default_provider() -> Result<ChatGptProvider, String> {
	providers::chatgpt_oauth_provider()
}

#[cfg(target_os = "macos")]
fn rewrite_with_guard(
	text: &str,
	model: &str,
	plan: &VoiceSessionPlan,
	settings: &RewriteSettings,
) -> Result<RewriteResult, String> {
	let provider = default_provider()?;
	let mut instructions = plan.rewrite_instructions(&settings.style, settings.max_output_chars);
	if !settings.glossary_terms.trim().is_empty() {
		instructions.push_str("\nGlossary terms to preserve or prefer:\n");
		instructions.push_str(settings.glossary_terms.trim());
	}
	let rewritten =
		provider.rewrite(RewriteRequest { text, model, instructions: &instructions })?;

	if rewritten.chars().count() > settings.max_output_chars as usize {
		return Ok(RewriteResult {
			rewritten_transcript: None,
			state: RewriteState::Rejected,
			reason: Some(format!(
				"rewrite exceeded max output length (limit={} chars). Using ASR transcript for safety.",
				settings.max_output_chars
			)),
		});
	}
	if !settings.guard_protected_tokens {
		return Ok(RewriteResult {
			rewritten_transcript: Some(rewritten),
			state: RewriteState::Applied,
			reason: None,
		});
	}

	let baseline = protected_token_multiset(text);
	let candidate = protected_token_multiset(&rewritten);
	if baseline != candidate {
		return Ok(RewriteResult {
			rewritten_transcript: None,
			state: RewriteState::Rejected,
			reason: Some(
				"rewrite changed protected tokens (numbers, dates, or currency). Using ASR transcript for safety.".to_string(),
			),
		});
	}

	Ok(RewriteResult {
		rewritten_transcript: Some(rewritten),
		state: RewriteState::Applied,
		reason: None,
	})
}

#[cfg(target_os = "macos")]
fn protected_token_multiset(text: &str) -> BTreeMap<String, u32> {
	let mut items = BTreeMap::new();

	for token in text.split_whitespace() {
		let token = trim_token(token);

		if token.is_empty() {
			continue;
		}

		if let Some(normalized) = normalize_currency_token(token) {
			*items.entry(normalized).or_default() += 1;

			continue;
		}
		if let Some(normalized) = normalize_date_token(token) {
			*items.entry(normalized).or_default() += 1;

			continue;
		}
		if let Some(normalized) = normalize_numeric_token(token) {
			*items.entry(normalized).or_default() += 1;
		}
	}

	items
}

#[cfg(target_os = "macos")]
fn trim_token(raw: &str) -> &str {
	raw.trim_matches(|ch: char| {
		matches!(
			ch,
			'.' | ',' | ';' | ':' | '!' | '?' | '"' | '\'' | '(' | ')' | '[' | ']' | '{' | '}'
		)
	})
}

#[cfg(target_os = "macos")]
fn normalize_currency_token(token: &str) -> Option<String> {
	if let Some(without_symbol) = token.strip_prefix('$') {
		let value = normalize_numeric_token(without_symbol)?;

		return Some(format!("${value}"));
	}
	if let Some(without_symbol) = token.strip_prefix('€') {
		let value = normalize_numeric_token(without_symbol)?;

		return Some(format!("€{value}"));
	}
	if let Some(without_symbol) = token.strip_prefix('£') {
		let value = normalize_numeric_token(without_symbol)?;

		return Some(format!("£{value}"));
	}
	if let Some(without_symbol) = token.strip_prefix('¥') {
		let value = normalize_numeric_token(without_symbol)?;

		return Some(format!("¥{value}"));
	}

	None
}

#[cfg(target_os = "macos")]
fn normalize_date_token(token: &str) -> Option<String> {
	let parts: Vec<&str> = token.split(['/', '-']).collect();

	if parts.len() != 3 {
		return None;
	}

	let norm: Vec<_> = parts.iter().map(|part| part.trim()).collect();

	if !norm.iter().all(|part| !part.is_empty() && part.chars().all(|c| c.is_ascii_digit())) {
		return None;
	}

	let year = if norm[0].len() == 4 { norm[0] } else { norm[2] };
	let month = norm[1];
	let day = norm[2];

	Some(format!("date|{year}-{month}-{day}"))
}

#[cfg(target_os = "macos")]
fn normalize_numeric_token(token: &str) -> Option<String> {
	if token.is_empty() {
		return None;
	}

	let trimmed = token.trim_matches(|ch: char| ch == '$' || ch == '£' || ch == '€' || ch == '¥');

	if trimmed.is_empty() {
		return None;
	}

	let mut digits_seen = false;
	let mut dot_seen = false;
	let mut normalized = String::new();

	for ch in trimmed.chars() {
		if ch.is_ascii_digit() {
			digits_seen = true;

			normalized.push(ch);

			continue;
		}
		if ch == '.' {
			if dot_seen {
				return None;
			}

			dot_seen = true;

			normalized.push(ch);

			continue;
		}
		if ch != ',' {
			return None;
		}
	}

	if digits_seen { Some(normalized) } else { None }
}

#[cfg(test)]
#[cfg(target_os = "macos")]
mod tests {
	use crate::inference::{self};

	#[test]
	fn normalize_numeric_token_extracts_stable_forms() {
		assert_eq!(inference::normalize_numeric_token("12,345.60"), Some("12345.60".to_string()));
		assert_eq!(inference::normalize_numeric_token("abc"), None);
	}

	#[test]
	fn normalize_currency_token_parses_common_markers() {
		assert_eq!(inference::normalize_currency_token("$12.50"), Some("$12.50".to_string()));
		assert_eq!(inference::normalize_currency_token("€1,200"), Some("€1200".to_string()));
		assert_eq!(inference::normalize_currency_token("100"), None);
	}

	#[test]
	fn normalize_date_token_parses_common_patterns() {
		assert_eq!(
			inference::normalize_date_token("2026-02-28"),
			Some("date|2026-02-28".to_string())
		);
		assert_eq!(inference::normalize_date_token("02/28/26"), Some("date|26-28-26".to_string()));
		assert_eq!(inference::normalize_date_token("abc"), None);
	}

	#[test]
	fn rewrite_guard_flags_numeric_changes() {
		let raw =
			inference::protected_token_multiset("call me at 120 and send 25 dollars on 2026-02-28");
		let rewritten = inference::protected_token_multiset(
			"call me at one twenty and send 26 dollars on 2026-02-28",
		);

		assert_ne!(raw, rewritten);
	}
}
