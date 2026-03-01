//! OpenAI transcription and optional rewrite client.

#[cfg(target_os = "macos")] use std::collections::BTreeMap;
#[cfg(target_os = "macos")] use std::time::{Duration, Instant};

#[cfg(target_os = "macos")] use reqwest::blocking::{Client, multipart};
#[cfg(target_os = "macos")] use serde_json::Value;

/// Rewrite outcome status.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RewriteState {
	/// Rewrite was intentionally skipped before request.
	Skipped,
	/// Rewrite succeeded and output passed all safety checks.
	Applied,
	/// Rewrite was returned but rejected due to protected token mismatch.
	Rejected,
}

/// Guarded rewrite result payload.
#[derive(Debug, Clone)]
pub struct RewriteResult {
	/// Optional rewritten transcript when state is `Applied`.
	pub rewritten_transcript: Option<String>,
	/// Rewrite decision.
	pub state: RewriteState,
	/// Optional reason for skipped or rejected rewrite.
	pub reason: Option<String>,
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

/// Transcribes WAV bytes using the configured Pass2 model.
#[cfg(target_os = "macos")]
pub fn transcribe_only(wav: &[u8], model: &str) -> Result<(String, u64), String> {
	let started = Instant::now();
	let raw = transcribe(wav, model)?;
	Ok((raw, started.elapsed().as_millis() as u64))
}

/// OpenAI pipeline is unavailable on non-macOS placeholder builds.
#[cfg(not(target_os = "macos"))]
pub fn transcribe_only(_wav: &[u8], _model: &str) -> Result<(String, u64), String> {
	Err("OpenAI pipeline is only enabled on macOS builds.".to_string())
}

/// Rewrites transcript text with protected-token guard checks.
#[cfg(target_os = "macos")]
pub fn rewrite_only(text: &str, model: &str) -> Result<(RewriteResult, u64), String> {
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
	let result = rewrite_with_guard(text, model)?;
	Ok((result, started.elapsed().as_millis() as u64))
}

/// Rewrite pipeline is unavailable on non-macOS placeholder builds.
#[cfg(not(target_os = "macos"))]
pub fn rewrite_only(_text: &str, _model: &str) -> Result<(RewriteResult, u64), String> {
	Err("OpenAI pipeline is only enabled on macOS builds.".to_string())
}

#[cfg(target_os = "macos")]
fn transcribe(wav: &[u8], model: &str) -> Result<String, String> {
	let body =
		post_multipart("https://api.openai.com/v1/audio/transcriptions", wav, "audio.wav", model)?;

	extract_json_value(&body, &["/text", "/output_text"])
		.or_else(|| extract_json_output_array_value(&body))
		.ok_or_else(|| "transcription response has no usable text".to_string())
}

#[cfg(target_os = "macos")]
fn rewrite(text: &str, model: &str) -> Result<String, String> {
	let prompt = "Rewrite the transcript for punctuation and readability. Keep the meaning, numbers, and names intact.";
	let body = serde_json::json!({
		"model": model,
		"input": format!("Transcript: {text}"),
		"instructions": prompt,
		"temperature": 0.2,
	});

	let body = post_json("https://api.openai.com/v1/responses", body)?;

	extract_json_value(&body, &["/output_text", "/output/0/content/0/text"])
		.or_else(|| extract_json_output_array_value(&body))
		.or_else(|| extract_json_value(&body, &["/text", "/choices/0/message/content"]))
		.ok_or_else(|| "rewrite response has no usable text".to_string())
}

#[cfg(target_os = "macos")]
fn rewrite_with_guard(text: &str, model: &str) -> Result<RewriteResult, String> {
	let rewritten = rewrite(text, model)?;
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
fn post_multipart(
	url: &str,
	file_bytes: &[u8],
	file_name: &str,
	model: &str,
) -> Result<String, String> {
	let (api_key, account_id) = auth_token()?;

	let client = Client::builder()
		.timeout(Duration::from_secs(120))
		.build()
		.map_err(|err| format!("failed to build OpenAI HTTP client: {err}"))?;

	let file_part = multipart::Part::bytes(file_bytes.to_vec())
		.file_name(file_name.to_string())
		.mime_str("audio/wav")
		.map_err(|err| format!("invalid file mime: {err}"))?;

	let form = multipart::Form::new().text("model", model.to_string()).part("file", file_part);
	let mut request = client.post(url).bearer_auth(api_key).multipart(form);
	if let Some(account_id) = account_id {
		request = request.header("ChatGPT-Account-ID", account_id);
	}
	let response = request.send().map_err(|err| format!("transcription request failed: {err}"))?;

	check_status(response, "transcription")
}

#[cfg(target_os = "macos")]
fn post_json(url: &str, body: Value) -> Result<String, String> {
	let (api_key, account_id) = auth_token()?;

	let client = Client::builder()
		.timeout(Duration::from_secs(120))
		.build()
		.map_err(|err| format!("failed to build OpenAI HTTP client: {err}"))?;

	let mut request = client.post(url).bearer_auth(api_key).json(&body);
	if let Some(account_id) = account_id {
		request = request.header("ChatGPT-Account-ID", account_id);
	}
	let response = request.send().map_err(|err| format!("rewrite request failed: {err}"))?;

	check_status(response, "rewrite")
}

#[cfg(target_os = "macos")]
fn check_status(response: reqwest::blocking::Response, step: &str) -> Result<String, String> {
	if !response.status().is_success() {
		let status = response.status();
		let body = response.text().unwrap_or_else(|_| "<failed to read response body>".to_string());
		return Err(format!("{step} failed with status {status}: {body}"));
	}

	response.text().map_err(|err| format!("failed to read {step} response body: {err}"))
}

#[cfg(target_os = "macos")]
fn extract_json_value(body: &str, pointers: &[&str]) -> Option<String> {
	let value = serde_json::from_str::<Value>(body).ok()?;

	pointers
		.iter()
		.find_map(|pointer| value.pointer(pointer).and_then(Value::as_str).map(str::to_string))
}

#[cfg(target_os = "macos")]
fn extract_json_output_array_value(body: &str) -> Option<String> {
	let value = serde_json::from_str::<Value>(body).ok()?;
	let outputs = value.get("output")?.as_array()?;

	outputs.iter().find_map(|entry| {
		entry.get("content").and_then(Value::as_array)?.iter().find_map(|chunk| {
			chunk
				.get("text")
				.or_else(|| chunk.get("transcript"))
				.and_then(Value::as_str)
				.map(str::to_string)
		})
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

#[cfg(target_os = "macos")]
fn auth_token() -> Result<(String, Option<String>), String> {
	crate::auth::access_token().map_err(|err| format!("auth token not available: {err}"))
}

#[cfg(test)]
#[cfg(target_os = "macos")]
mod tests {
	use super::{
		normalize_currency_token, normalize_date_token, normalize_numeric_token,
		protected_token_multiset,
	};

	#[test]
	fn normalize_numeric_token_extracts_stable_forms() {
		assert_eq!(normalize_numeric_token("12,345.60"), Some("12345.60".to_string()));
		assert_eq!(normalize_numeric_token("abc"), None);
	}

	#[test]
	fn normalize_currency_token_parses_common_markers() {
		assert_eq!(normalize_currency_token("$12.50"), Some("$12.50".to_string()));
		assert_eq!(normalize_currency_token("€1,200"), Some("€1200".to_string()));
		assert_eq!(normalize_currency_token("100"), None);
	}

	#[test]
	fn normalize_date_token_parses_common_patterns() {
		assert_eq!(normalize_date_token("2026-02-28"), Some("date|2026-02-28".to_string()));
		assert_eq!(normalize_date_token("02/28/26"), Some("date|26-28-26".to_string()));
		assert_eq!(normalize_date_token("abc"), None);
	}

	#[test]
	fn rewrite_guard_flags_numeric_changes() {
		let raw = protected_token_multiset("call me at 120 and send 25 dollars on 2026-02-28");
		let rewritten =
			protected_token_multiset("call me at one twenty and send 26 dollars on 2026-02-28");
		assert_ne!(raw, rewritten);
	}
}
