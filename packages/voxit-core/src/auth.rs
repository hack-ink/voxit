//! ChatGPT authentication using OAuth and device-code flow with secure token storage.

use std::{
	collections::HashMap,
	env,
	fs::{self, File, OpenOptions, Permissions},
	io::{self, Error, ErrorKind, Read as _, Write as _},
	os::unix::fs::{OpenOptionsExt as _, PermissionsExt as _},
	path::{Path, PathBuf},
	string::{String, ToString},
	sync::{Condvar, Mutex, OnceLock, RwLock},
	thread,
	time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};

use base64::Engine;
use directories::ProjectDirs;
use keyring::Entry;
use rand::RngExt as _;
use reqwest::blocking::Client;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use sha2::{Digest, Sha256};
use tiny_http::{Header, Request, Server};
use url::{Url, form_urlencoded};

type AuthResult<T> = std::result::Result<T, String>;

const CLIENT_ID: &str = "app_EMoamEEZ73f0CkXaXp7hrann";
const DEFAULT_ISSUER: &str = "https://auth.openai.com";
const DEFAULT_PORT: u16 = 1_455;
const REDIRECT_URI_PATH: &str = "/auth/callback";
const CODEX_OAUTH_ORIGINATOR: &str = "codex_cli_rs";
const KEYRING_SERVICE: &str = "Voxit Auth";
const KEYRING_KEY_PREFIX: &str = "cli|";
const AUTH_FILE_NAME: &str = "auth.json";
const AUTH_FILE_FALLBACK_ENV: &str = "VOXIT_AUTH_FILE_FALLBACK";
const KEYRING_VERIFY_ENABLED_ENV: &str = "VOXIT_VERIFY_KEYRING";
const KEYRING_VERIFY_ATTEMPTS: usize = 5;
const KEYRING_VERIFY_DELAY_MS: u64 = 120;
#[cfg(test)]
const TEST_FORCE_KEYRING_ERROR_ENV: &str = "VOXIT_TEST_FORCE_KEYRING_ERROR";

static SESSION_TOKEN_CACHE: OnceLock<RwLock<Option<TokenData>>> = OnceLock::new();
static STORED_AUTH_CACHE: OnceLock<(Mutex<StoredAuthCacheState>, Condvar)> = OnceLock::new();

/// Authentication result returned to the UI after sign-in.
#[derive(Clone, Debug)]
pub struct AuthRecord {
	/// Optional account id extracted from token claims.
	pub account_id: Option<String>,
}

/// Compact auth status for UI display.
#[derive(Clone, Debug)]
pub struct AuthStatus {
	/// Whether valid token credentials exist in local storage.
	pub signed_in: bool,
	/// Optional ChatGPT account id claim from stored token claims.
	pub account_id: Option<String>,
}

#[derive(Clone, Debug, Default)]
struct StoredAuthCacheState {
	loading: bool,
	result: Option<Option<TokenData>>,
}

#[derive(Clone, Debug, PartialEq, Deserialize, Serialize)]
struct TokenData {
	id_token: String,
	access_token: String,
	refresh_token: Option<String>,
	account_id: Option<String>,
	created_at_unix: u64,
	expires_in_seconds: Option<u64>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
struct StoredAuth {
	auth_mode: Option<String>,
	#[serde(rename = "OPENAI_API_KEY")]
	openai_api_key: Option<String>,

	tokens: Option<TokenData>,
}

#[derive(Debug)]
struct PkceCodes {
	code_verifier: String,
	code_challenge: String,
}

#[derive(Debug)]
struct DeviceCode {
	device_auth_id: String,
	user_code: String,
	interval_secs: u64,
}

#[derive(Debug)]
struct DeviceLoginCode {
	authorization_code: String,
	code_verifier: String,
}

/// Return stored authentication status without leaking token payload.
pub fn status() -> AuthStatus {
	if let Some(tokens) = load_cached_tokens() {
		return AuthStatus { signed_in: true, account_id: tokens.account_id };
	}

	match load_stored_auth_tokens() {
		Ok(Some(auth)) => {
			cache_session_tokens(&auth);

			AuthStatus { signed_in: true, account_id: auth.account_id }
		},
		_ => AuthStatus { signed_in: false, account_id: None },
	}
}

/// Remove stored auth tokens (keyring and fallback file).
pub fn sign_out() -> AuthResult<()> {
	clear_cached_session_tokens();
	clear_stored_auth_cache();

	let base = app_config_dir()?;

	clear_keyring_entry(&base)?;
	clear_auth_file(&base)?;

	Ok(())
}

/// Start browser login flow and store OAuth credentials.
pub fn sign_in_with_chatgpt() -> AuthResult<AuthRecord> {
	sign_in_with_chatgpt_browser()
}

/// Start device-code flow and store OAuth credentials.
#[allow(dead_code)]
pub fn sign_in_with_device_code() -> AuthResult<AuthRecord> {
	sign_in_with_device_code_with_progress(|_, _| {})
}

/// Start device-code flow and report the one-time user code + verification URI.
/// The callback is invoked after user code retrieval and before polling starts.
pub fn sign_in_with_device_code_with_progress<F>(on_device_code: F) -> AuthResult<AuthRecord>
where
	F: Fn(&str, &str),
{
	let device_code = request_device_code()?;
	let verification_uri = format!("{DEFAULT_ISSUER}/codex/device");

	on_device_code(&device_code.user_code, &verification_uri);

	let _ = webbrowser::open(&verification_uri);
	let login_code = poll_device_code(&device_code)?;
	let redirect_uri = format!("{DEFAULT_ISSUER}/deviceauth/callback");
	let pkce = PkceCodes { code_verifier: login_code.code_verifier, code_challenge: String::new() };
	let tokens = exchange_authorization_code(
		&pkce,
		&login_code.authorization_code,
		&redirect_uri,
		DEFAULT_ISSUER,
	)?;

	store_tokens(&tokens)?;

	Ok(AuthRecord { account_id: tokens.account_id })
}

/// Returns `(access_token, account_id)` for API calls.
/// Falls back to `OPENAI_API_KEY` only when no stored OAuth token exists.
pub fn access_token() -> AuthResult<(String, Option<String>)> {
	if let Some(tokens) = load_cached_tokens() {
		return Ok((tokens.access_token, tokens.account_id));
	}
	if let Some(tokens) = load_stored_auth_tokens()? {
		cache_session_tokens(&tokens);

		return Ok((tokens.access_token, tokens.account_id));
	}

	env::var("OPENAI_API_KEY")
		.map(|value| (value, None))
		.map_err(|_| "not signed in and OPENAI_API_KEY is not set".to_string())
}

fn sign_in_with_chatgpt_browser() -> AuthResult<AuthRecord> {
	let pkce = generate_pkce();
	let state = generate_state();
	let redirect_uri = browser_redirect_uri();
	let authorize_url =
		build_authorize_url(&redirect_uri, &pkce.code_challenge, &state, DEFAULT_ISSUER);

	webbrowser::open(&authorize_url)
		.map_err(|_| "failed to open browser for ChatGPT login".to_string())?;

	wait_for_callback(&state, &pkce, &redirect_uri, DEFAULT_ISSUER)
}

fn browser_redirect_uri() -> String {
	// Codex OSS uses http://localhost:<port>/auth/callback for browser OAuth redirect URI.
	// Aligning here avoids auth.openai.com rejecting 127.0.0.1 redirect URIs for this client id.
	format!("http://localhost:{DEFAULT_PORT}{REDIRECT_URI_PATH}")
}

fn valid_tokens_or_none(auth: Option<StoredAuth>) -> AuthResult<Option<TokenData>> {
	let auth = match auth {
		Some(auth) => auth,
		None => return Ok(None),
	};
	let tokens = match auth.tokens {
		Some(tokens) => tokens,
		None => return Ok(None),
	};

	if is_token_expired(tokens.created_at_unix, tokens.expires_in_seconds) {
		return Ok(None);
	}

	Ok(Some(tokens))
}

fn load_cached_tokens() -> Option<TokenData> {
	let cached = {
		let cache = session_token_cache().read().unwrap_or_else(|err| err.into_inner());

		cache.clone()
	};
	let tokens = cached?;

	if is_token_expired(tokens.created_at_unix, tokens.expires_in_seconds) {
		clear_cached_session_tokens();

		return None;
	}

	Some(tokens)
}

fn request_device_code() -> AuthResult<DeviceCode> {
	let payload = serde_json::json!({ "client_id": CLIENT_ID });
	let response = post_json(
		&format!("{DEFAULT_ISSUER}/api/accounts/deviceauth/usercode"),
		&payload.to_string(),
	)?;
	let parsed: Value = serde_json::from_str(&response)
		.map_err(|err| format!("invalid device code response: {err}"))?;
	let interval_secs = parsed
		.get("interval")
		.and_then(|value| value.as_str())
		.and_then(|value| value.parse::<u64>().ok())
		.or_else(|| parsed.get("interval").and_then(|value| value.as_u64()))
		.unwrap_or(5);
	let device_auth_id =
		parsed.get("device_auth_id").and_then(|value| value.as_str()).unwrap_or_else(|| {
			parsed.get("deviceauth_id").and_then(|value| value.as_str()).unwrap_or("")
		});
	let user_code = parsed
		.get("user_code")
		.and_then(|value| value.as_str())
		.or_else(|| parsed.get("usercode").and_then(|value| value.as_str()))
		.ok_or_else(|| "user code missing".to_string())?;

	if device_auth_id.is_empty() {
		return Err("device_auth_id missing".to_string());
	}

	Ok(DeviceCode {
		device_auth_id: device_auth_id.to_string(),
		user_code: user_code.to_string(),
		interval_secs,
	})
}

fn poll_device_code(device_code: &DeviceCode) -> AuthResult<DeviceLoginCode> {
	let endpoint = format!("{DEFAULT_ISSUER}/api/accounts/deviceauth/token");
	let start = Instant::now();
	let max_wait = Duration::from_secs(15 * 60);
	let payload = serde_json::json!({
		"device_auth_id": device_code.device_auth_id,
		"user_code": device_code.user_code,
	});
	let body = serde_json::to_vec(&payload).map_err(|err| format!("invalid poll body: {err}"))?;

	loop {
		let response = post_raw(&endpoint, &body)?;

		if response.status().is_success() {
			let text = response
				.text()
				.map_err(|err| format!("failed to read device-auth response: {err}"))?;
			let parsed: Value = serde_json::from_str(&text)
				.map_err(|err| format!("invalid device-auth token response: {err}"))?;
			let authorization_code = parsed
				.get("authorization_code")
				.and_then(|value| value.as_str())
				.ok_or_else(|| "authorization_code missing".to_string())?;
			let code_verifier = parsed
				.get("code_verifier")
				.and_then(|value| value.as_str())
				.ok_or_else(|| "code_verifier missing".to_string())?;

			return Ok(DeviceLoginCode {
				authorization_code: authorization_code.to_string(),
				code_verifier: code_verifier.to_string(),
			});
		}
		if response.status().as_u16() != 403 && response.status().as_u16() != 404 {
			return Err(format!("device-auth polling failed: status {}", response.status()));
		}
		if start.elapsed() > max_wait {
			return Err("device auth expired".to_string());
		}

		thread::sleep(
			Duration::from_secs(device_code.interval_secs).min(max_wait - start.elapsed()),
		);
	}
}

fn exchange_authorization_code(
	pkce: &PkceCodes,
	authorization_code: &str,
	redirect_uri: &str,
	issuer: &str,
) -> AuthResult<TokenData> {
	let form = format!(
		"grant_type=authorization_code&code={}&redirect_uri={}&client_id={}&code_verifier={}",
		url_encode(authorization_code),
		url_encode(redirect_uri),
		url_encode(CLIENT_ID),
		url_encode(&pkce.code_verifier),
	);
	let response = post_form(&format!("{issuer}/oauth/token"), &form)?;
	let parsed: Value =
		serde_json::from_str(&response).map_err(|err| format!("invalid token response: {err}"))?;
	let id_token = parsed
		.get("id_token")
		.and_then(|value| value.as_str())
		.ok_or_else(|| "id_token missing".to_string())?;
	let access_token = parsed
		.get("access_token")
		.and_then(|value| value.as_str())
		.ok_or_else(|| "access_token missing".to_string())?;
	let refresh_token =
		parsed.get("refresh_token").and_then(|value| value.as_str()).map(str::to_string);
	let expires_in_seconds = parsed.get("expires_in").and_then(|value| value.as_u64());
	let account_id = extract_claims(id_token).and_then(|claims| {
		claims.get("chatgpt_account_id").and_then(|v| v.as_str()).map(str::to_string)
	});

	Ok(TokenData {
		id_token: id_token.to_string(),
		access_token: access_token.to_string(),
		refresh_token,
		account_id,
		created_at_unix: now_unix(),
		expires_in_seconds,
	})
}

fn store_tokens(tokens: &TokenData) -> AuthResult<()> {
	let auth = StoredAuth {
		auth_mode: Some("chatgpt".to_string()),
		openai_api_key: None,
		tokens: Some(tokens.clone()),
	};
	let base = app_config_dir()?;
	let serialized =
		serde_json::to_string_pretty(&auth).map_err(|err| format!("serialize auth: {err}"))?;
	let keyring_saved = match save_to_keyring(&base, &serialized) {
		Ok(()) => {
			if should_verify_keyring_storage() {
				verify_keyring_storage(&base, tokens)?;
			}

			true
		},
		Err(err) =>
			if auth_file_fallback_enabled() {
				tracing::warn!(error = %err, "auth keyring save failed, falling back to auth.json");

				save_to_file(&base, &serialized)?;

				false
			} else {
				return Err(format!(
					"auth keychain save failed: {err} (set {AUTH_FILE_FALLBACK_ENV}=1 to allow insecure auth.json fallback)"
				));
			},
	};

	cache_session_tokens(tokens);
	cache_stored_auth_tokens(Some(tokens.clone()));

	if keyring_saved {
		let _ = clear_auth_file(&base);
	}

	Ok(())
}

fn verify_keyring_storage(base: &Path, expected_tokens: &TokenData) -> AuthResult<()> {
	let mut last_err = String::from("auth keyring save verification timed out");

	for _ in 0..KEYRING_VERIFY_ATTEMPTS {
		match load_from_keyring(base)? {
			Some(auth) => {
				let stored_tokens = auth.tokens.ok_or_else(|| {
					String::from("auth keyring verification found stored auth without tokens")
				})?;

				if stored_tokens == *expected_tokens {
					return Ok(());
				}

				return Err(String::from("auth keyring verification mismatch"));
			},
			None => {
				last_err = String::from("auth keyring verification not found");

				thread::sleep(Duration::from_millis(KEYRING_VERIFY_DELAY_MS));
			},
		}
	}

	Err(last_err)
}

fn load_stored_auth() -> AuthResult<Option<StoredAuth>> {
	let base = app_config_dir()?;

	if let Some(auth) = load_from_keyring(&base)? {
		return Ok(Some(auth));
	}

	if auth_file_fallback_enabled() { load_from_file(&base) } else { Ok(None) }
}

fn auth_file_fallback_enabled() -> bool {
	env_flag_enabled(AUTH_FILE_FALLBACK_ENV)
}

fn should_verify_keyring_storage() -> bool {
	env_flag_enabled(KEYRING_VERIFY_ENABLED_ENV)
}

fn env_flag_enabled(name: &str) -> bool {
	match env::var(name) {
		Ok(value) => {
			let value = value.trim().to_ascii_lowercase();

			value == "1" || value == "true" || value == "yes"
		},
		Err(_) => false,
	}
}

fn stored_auth_cache() -> &'static (Mutex<StoredAuthCacheState>, Condvar) {
	STORED_AUTH_CACHE.get_or_init(|| (Mutex::new(StoredAuthCacheState::default()), Condvar::new()))
}

fn load_stored_auth_tokens() -> AuthResult<Option<TokenData>> {
	let (cache_lock, cache_cv) = stored_auth_cache();
	let mut state = cache_lock.lock().unwrap_or_else(|err| err.into_inner());

	loop {
		if let Some(cached) = state.result.clone() {
			match cached {
				Some(tokens) => {
					if is_token_expired(tokens.created_at_unix, tokens.expires_in_seconds) {
						clear_cached_session_tokens();

						state.result = None;

						tracing::debug!("stored auth cache expired, refreshing");

						continue;
					}

					tracing::debug!("stored auth cache hit");

					return Ok(Some(tokens));
				},
				None => {
					tracing::debug!("stored auth cache hit (no tokens)");

					return Ok(None);
				},
			}
		}

		if state.loading {
			tracing::debug!("waiting for in-flight stored-auth load");

			state = cache_cv.wait(state).unwrap_or_else(|err| err.into_inner());

			continue;
		}

		state.loading = true;

		drop(state);

		tracing::debug!("stored auth cache miss, reading keyring/fallback");

		let loaded = load_stored_auth().and_then(valid_tokens_or_none);

		state = cache_lock.lock().unwrap_or_else(|err| err.into_inner());
		state.loading = false;

		if let Ok(ref tokens) = loaded {
			state.result = Some(tokens.clone());
		} else {
			state.result = None;
		}

		cache_cv.notify_all();

		if let Err(err) = &loaded {
			tracing::warn!(error = %err, "stored auth read failed");
		}

		return loaded;
	}
}

fn cache_stored_auth_tokens(tokens: Option<TokenData>) {
	let (cache_lock, _) = stored_auth_cache();
	let mut state = cache_lock.lock().unwrap_or_else(|err| err.into_inner());

	state.result = Some(tokens);
}

fn clear_stored_auth_cache() {
	let (cache_lock, cache_cv) = stored_auth_cache();
	let mut state = cache_lock.lock().unwrap_or_else(|err| err.into_inner());

	*state = StoredAuthCacheState::default();

	cache_cv.notify_all();
}

fn session_token_cache() -> &'static RwLock<Option<TokenData>> {
	SESSION_TOKEN_CACHE.get_or_init(|| RwLock::new(None))
}

fn cache_session_tokens(tokens: &TokenData) {
	let mut cache = session_token_cache().write().unwrap_or_else(|err| err.into_inner());

	*cache = Some(tokens.clone());
}

fn clear_cached_session_tokens() {
	let mut cache = session_token_cache().write().unwrap_or_else(|err| err.into_inner());

	*cache = None;
}

fn save_to_keyring(base: &Path, payload: &str) -> io::Result<()> {
	let key = auth_key(base).map_err(Error::other)?;

	#[cfg(test)]
	if env_flag_enabled(TEST_FORCE_KEYRING_ERROR_ENV) {
		return Err(Error::other("forced test keyring error"));
	}

	let entry = Entry::new(KEYRING_SERVICE, &key).map_err(Error::other)?;

	entry.set_password(payload).map_err(Error::other)
}

fn load_from_keyring(base: &Path) -> AuthResult<Option<StoredAuth>> {
	let key = auth_key(base)?;
	let entry = match Entry::new(KEYRING_SERVICE, &key) {
		Ok(entry) => entry,
		Err(_) => return Ok(None),
	};
	let value = match entry.get_password() {
		Ok(value) => value,
		Err(err) => {
			let err_text = err.to_string();

			if err_text.contains("not found") {
				return Ok(None);
			}

			return Err(format!("keyring read failed: {err_text}"));
		},
	};
	let parsed = serde_json::from_str(&value)
		.map_err(|err| format!("decode keyring auth json failed: {err}"))?;

	Ok(Some(parsed))
}

fn clear_keyring_entry(base: &Path) -> AuthResult<()> {
	let key = auth_key(base)?;

	match Entry::new(KEYRING_SERVICE, &key) {
		Ok(entry) => {
			if let Err(err) = entry.delete_credential() {
				let message = err.to_string();

				if !message.contains("not found") {
					return Err(format!("keyring delete failed: {message}"));
				}
			}

			Ok(())
		},
		Err(_) => Ok(()),
	}
}

fn save_to_file(base: &Path, payload: &str) -> AuthResult<()> {
	let path = base.join(AUTH_FILE_NAME);

	if let Some(parent) = path.parent() {
		fs::create_dir_all(parent).map_err(|err| format!("create auth dir failed: {err}"))?;
	}

	let mut builder = OpenOptions::new();

	builder.create(true).truncate(true).write(true);
	#[cfg(unix)]
	{
		builder.mode(0o600);
	}

	let mut file = builder
		.create(true)
		.truncate(true)
		.write(true)
		.open(&path)
		.map_err(|err| format!("create auth file failed: {err}"))?;

	file.write_all(payload.as_bytes()).map_err(|err| format!("write auth file failed: {err}"))?;
	#[cfg(unix)]
	{
		fs::set_permissions(&path, Permissions::from_mode(0o600))
			.map_err(|err| format!("set auth file permissions failed: {err}"))?;
	}

	Ok(())
}

fn load_from_file(base: &Path) -> AuthResult<Option<StoredAuth>> {
	let path = base.join(AUTH_FILE_NAME);
	let mut file = match File::open(&path) {
		Ok(file) => file,
		Err(err) if err.kind() == ErrorKind::NotFound => return Ok(None),
		Err(err) => return Err(format!("open auth file failed: {err}")),
	};
	let mut payload = String::new();

	file.read_to_string(&mut payload).map_err(|err| format!("read auth file failed: {err}"))?;

	let parsed: StoredAuth =
		serde_json::from_str(&payload).map_err(|err| format!("decode auth file failed: {err}"))?;

	Ok(Some(parsed))
}

fn clear_auth_file(base: &Path) -> AuthResult<()> {
	let path = base.join(AUTH_FILE_NAME);

	match fs::remove_file(&path) {
		Ok(()) => Ok(()),
		Err(err) if err.kind() == ErrorKind::NotFound => Ok(()),
		Err(err) => Err(format!("remove auth file failed: {err}")),
	}
}

fn wait_for_callback(
	expected_state: &str,
	pkce: &PkceCodes,
	redirect_uri: &str,
	issuer: &str,
) -> AuthResult<AuthRecord> {
	let server = Server::http(format!("localhost:{DEFAULT_PORT}"))
		.map_err(|err| format!("failed to bind local callback server: {err}"))?;
	let start = Instant::now();
	let timeout = Duration::from_secs(180);

	loop {
		let request = match server.recv_timeout(Duration::from_millis(200)) {
			Ok(Some(request)) => request,
			Ok(None) => {
				if start.elapsed() > timeout {
					return Err("browser login timeout".to_string());
				}

				continue;
			},
			Err(err) => return Err(format!("auth callback wait failed: {err}")),
		};

		match handle_callback_request(request, expected_state, pkce, redirect_uri, issuer)? {
			Some(record) => return Ok(record),
			None =>
				if start.elapsed() > timeout {
					return Err("browser login timeout".to_string());
				},
		}
	}
}

fn handle_callback_request(
	request: Request,
	expected_state: &str,
	pkce: &PkceCodes,
	redirect_uri: &str,
	issuer: &str,
) -> AuthResult<Option<AuthRecord>> {
	let full_url = format!("http://localhost{}", request.url());
	let parsed = match Url::parse(&full_url) {
		Ok(parsed) => parsed,
		Err(err) => {
			respond_error(request, 400, &format!("bad request: {err}"));

			return Err("callback url parse failed".to_string());
		},
	};

	if parsed.path() != REDIRECT_URI_PATH {
		respond_text(request, 404, "not found");

		return Ok(None);
	}

	let params: HashMap<String, String> = parsed.query_pairs().into_owned().collect();

	if let Some(error) = params.get("error").filter(|v| !v.is_empty()) {
		let details = params.get("error_description").map_or_else(String::new, ToString::to_string);
		let message = if details.is_empty() {
			format!("oauth callback error: {error}")
		} else {
			format!("oauth callback error: {error} ({details})")
		};

		respond_error(request, 400, &message);

		return Err(message);
	}

	let state = params.get("state").map_or("", String::as_str);

	if state != expected_state {
		let message = "state mismatch".to_string();

		respond_error(request, 400, &message);

		return Err(message);
	}

	let record = match params.get("code") {
		Some(code) => {
			let tokens = match exchange_authorization_code(pkce, code, redirect_uri, issuer) {
				Ok(tokens) => tokens,
				Err(err) => {
					let message = format!("oauth token exchange failed: {err}");

					respond_error(request, 400, &message);

					return Err(message);
				},
			};

			if let Err(err) = store_tokens(&tokens) {
				let message = format!("oauth token save failed: {err}");

				respond_error(request, 500, &message);

				return Err(message);
			}

			respond_html(request, 200, &success_redirect_page_html());

			AuthRecord { account_id: tokens.account_id }
		},
		None => {
			let message = "missing authorization code".to_string();

			respond_error(request, 400, &message);

			return Err(message);
		},
	};

	Ok(Some(record))
}

fn respond_text(request: Request, status_code: u16, body: &str) {
	let response = tiny_http::Response::from_string(body).with_status_code(status_code);
	let _ = request.respond(response);
}

fn respond_html(request: Request, status_code: u16, body: &str) {
	let response = tiny_http::Response::from_string(body)
		.with_status_code(status_code)
		.with_header(
			Header::from_bytes("Content-Type", "text/html; charset=utf-8")
				.expect("valid Content-Type header"),
		)
		.with_header(
			Header::from_bytes("Cache-Control", "no-store").expect("valid Cache-Control header"),
		);
	let _ = request.respond(response);
}

fn respond_error(request: Request, status_code: u16, message: &str) {
	let body = format!(
		r#"<html><body><p>{}</p><p>Close this window and retry from Voxit.</p></body></html>"#,
		html_escape(message)
	);

	respond_html(request, status_code, &body);
}

fn success_redirect_page_html() -> String {
	// Browsers often block `window.close()` unless the window was opened by script.
	// Still attempt auto-close to match common OAuth UX; always show a manual close instruction.
	r#"<!doctype html>
<html lang="en">
  <head>
    <meta charset="utf-8" />
    <meta name="viewport" content="width=device-width, initial-scale=1" />
    <title>Voxit Sign-in</title>
    <style>
      :root { color-scheme: light; }
      body {
        margin: 0;
        min-height: 100vh;
        font-family: system-ui, -apple-system, BlinkMacSystemFont, "Segoe UI", Roboto, Oxygen,
          Ubuntu, Cantarell, "Open Sans", "Helvetica Neue", sans-serif;
        background: radial-gradient(circle at top, #f7f8fb 0%, #ffffff 48%);
        color: #0d0d0d;
      }
      .container {
        min-height: 100vh;
        padding: 24px;
        box-sizing: border-box;
        display: flex;
        align-items: center;
        justify-content: center;
      }
      .card {
        width: min(560px, 100%);
        border-radius: 16px;
        border: 1px solid rgba(13, 13, 13, 0.12);
        box-shadow: 0 12px 32px rgba(0, 0, 0, 0.06);
        background: #ffffff;
        padding: 24px;
        text-align: center;
      }
      .title {
        margin: 0 0 8px 0;
        font-size: 18px;
        font-weight: 650;
        letter-spacing: -0.01em;
      }
      .desc { margin: 0 0 10px 0; color: rgba(13, 13, 13, 0.76); line-height: 1.45; }
      .muted { margin: 0; color: rgba(13, 13, 13, 0.60); font-size: 13px; line-height: 1.4; }
      .pill {
        display: inline-block;
        margin-top: 14px;
        padding: 6px 10px;
        border-radius: 999px;
        border: 1px solid rgba(13, 13, 13, 0.12);
        background: rgba(13, 13, 13, 0.03);
        font-size: 13px;
        color: rgba(13, 13, 13, 0.72);
      }
    </style>
  </head>
  <body>
    <div class="container">
      <div class="card">
        <h1 class="title">Signed in</h1>
        <p class="desc">
          You may return to Voxit. This window will try to close automatically in
          <strong><span id="seconds">7</span>s</strong>.
        </p>
        <p class="muted">If it doesn’t close automatically, please close this window manually.</p>
        <div class="pill">You can safely close this page.</div>
      </div>
    </div>
    <script>
      (function () {
        var remaining = 7;
        var el = document.getElementById("seconds");
        function update() {
          if (el) el.textContent = String(Math.max(0, remaining));
        }
        function attemptClose() {
          try { window.open("", "_self"); } catch (e) {}
          try { window.close(); } catch (e) {}
        }
        update();
        var timer = setInterval(function () {
          remaining -= 1;
          update();
          if (remaining <= 0) {
            clearInterval(timer);
            attemptClose();
          }
        }, 1000);
        setTimeout(attemptClose, 7000);
      })();
    </script>
  </body>
</html>
"#
	.to_string()
}

fn build_authorize_url(
	redirect_uri: &str,
	code_challenge: &str,
	state: &str,
	issuer: &str,
) -> String {
	let mut url = Url::parse(&format!("{issuer}/oauth/authorize")).unwrap_or_else(|_| {
		Url::parse("https://auth.openai.com/oauth/authorize")
			.unwrap_or_else(|_| Url::parse("https://example.com").expect("valid fallback url"))
	});

	url.query_pairs_mut().append_pair("response_type", "code");
	url.query_pairs_mut().append_pair("client_id", CLIENT_ID);
	url.query_pairs_mut().append_pair("redirect_uri", redirect_uri);
	url.query_pairs_mut().append_pair("scope", "openid profile email offline_access");
	url.query_pairs_mut().append_pair("code_challenge", code_challenge);
	url.query_pairs_mut().append_pair("code_challenge_method", "S256");
	url.query_pairs_mut().append_pair("id_token_add_organizations", "true");
	url.query_pairs_mut().append_pair("codex_cli_simplified_flow", "true");
	url.query_pairs_mut().append_pair("state", state);
	url.query_pairs_mut().append_pair("originator", CODEX_OAUTH_ORIGINATOR);

	url.to_string()
}

fn post_json(url: &str, body: &str) -> AuthResult<String> {
	let client = Client::builder()
		.timeout(Duration::from_secs(120))
		.build()
		.map_err(|err| format!("failed to build auth client: {err}"))?;
	let response = client
		.post(url)
		.header("Content-Type", "application/json")
		.body(body.to_string())
		.send()
		.map_err(|err| format!("request failed: {err}"))?;

	parse_response(response, url)
}

fn post_form(url: &str, body: &str) -> AuthResult<String> {
	let client = Client::builder()
		.timeout(Duration::from_secs(120))
		.build()
		.map_err(|err| format!("failed to build auth client: {err}"))?;
	let response = client
		.post(url)
		.header("Content-Type", "application/x-www-form-urlencoded")
		.body(body.to_string())
		.send()
		.map_err(|err| format!("request failed: {err}"))?;

	parse_response(response, url)
}

fn post_raw(url: &str, body: &[u8]) -> AuthResult<reqwest::blocking::Response> {
	let client = Client::builder()
		.timeout(Duration::from_secs(120))
		.build()
		.map_err(|err| format!("failed to build auth client: {err}"))?;
	let response = client
		.post(url)
		.header("Content-Type", "application/json")
		.body(body.to_vec())
		.send()
		.map_err(|err| format!("request failed: {err}"))?;

	Ok(response)
}

fn parse_response(response: reqwest::blocking::Response, context: &str) -> AuthResult<String> {
	if !response.status().is_success() {
		let status = response.status();
		let body = response.text().unwrap_or_else(|_| "<unreadable>".to_string());
		let detail = parse_error_text(&body);

		return Err(format!("oauth request to {context} failed ({status}): {detail}"));
	}

	response.text().map_err(|err| format!("failed to read response body: {err}"))
}

fn parse_error_text(raw: &str) -> String {
	let parsed = serde_json::from_str::<Value>(raw).ok();

	if let Some(json) = parsed {
		if let Some(error_description) =
			json.get("error_description").and_then(|value| value.as_str())
		{
			return error_description.to_string();
		}
		if let Some(error) = json.get("error").and_then(|value| value.as_str()) {
			return error.to_string();
		}
	}

	raw.to_string()
}

fn extract_claims(id_token: &str) -> Option<HashMap<String, Value>> {
	let mut parts = id_token.split('.');
	let _header = parts.next()?;
	let payload = parts.next()?;
	let payload =
		base64::engine::general_purpose::URL_SAFE_NO_PAD.decode(payload.as_bytes()).ok()?;
	let value: Value = serde_json::from_slice(&payload).ok()?;
	let claims = value.get("https://api.openai.com/auth")?.as_object()?;

	Some(claims.clone().into_iter().collect())
}

fn is_token_expired(created_at_unix: u64, expires_in: Option<u64>) -> bool {
	let ttl = expires_in.unwrap_or(3_600).saturating_sub(60);
	let now = now_unix();

	now >= created_at_unix.saturating_add(ttl)
}

fn generate_pkce() -> PkceCodes {
	let mut bytes = [0_u8; 64];
	let mut rng = rand::rng();

	rng.fill(&mut bytes);

	let code_verifier = base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(bytes);
	let challenge = Sha256::digest(code_verifier.as_bytes());
	let code_challenge = base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(challenge);

	PkceCodes { code_verifier, code_challenge }
}

fn generate_state() -> String {
	let mut bytes = [0_u8; 32];
	let mut rng = rand::rng();

	rng.fill(&mut bytes);

	base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(bytes)
}

fn now_unix() -> u64 {
	SystemTime::now().duration_since(UNIX_EPOCH).map_or(0, |d| d.as_secs())
}

fn app_config_dir() -> AuthResult<PathBuf> {
	ProjectDirs::from("", "hack.ink", "voxit")
		.map(|dirs| dirs.config_dir().to_path_buf())
		.ok_or_else(|| "failed to resolve app config dir".to_string())
}

fn auth_key(base_path: &Path) -> AuthResult<String> {
	let canonical = base_path.canonicalize().unwrap_or_else(|_| base_path.to_path_buf());
	let mut hasher = Sha256::new();

	hasher.update(canonical.to_string_lossy().as_bytes());

	let digest = hasher.finalize();
	let hex = format!("{digest:x}");
	let short = hex.get(..16).unwrap_or(&hex);

	Ok(format!("{KEYRING_KEY_PREFIX}{short}"))
}

fn url_encode(value: &str) -> String {
	form_urlencoded::byte_serialize(value.as_bytes()).collect::<String>()
}

fn html_escape(raw: &str) -> String {
	let mut out = String::new();

	for ch in raw.chars() {
		match ch {
			'&' => out.push_str("&amp;"),
			'<' => out.push_str("&lt;"),
			'>' => out.push_str("&gt;"),
			'"' => out.push_str("&quot;"),
			'\'' => out.push_str("&#39;"),
			_ => out.push(ch),
		}
	}

	out
}

#[cfg(test)]
mod tests {
	use std::{env, fs, sync::Mutex};

	use crate::auth::{
		self, AUTH_FILE_FALLBACK_ENV, CLIENT_ID, CODEX_OAUTH_ORIGINATOR, DEFAULT_ISSUER,
		DEFAULT_PORT, HashMap, KEYRING_VERIFY_ENABLED_ENV, REDIRECT_URI_PATH, StoredAuth,
		TEST_FORCE_KEYRING_ERROR_ENV, TokenData, Url,
	};

	static TEST_MUTEX: Mutex<()> = Mutex::new(());

	fn set_env(key: &str, value: Option<&str>) -> String {
		let previous = env::var(key).unwrap_or_default();

		if let Some(value) = value {
			unsafe { env::set_var(key, value) };
		} else {
			unsafe { env::remove_var(key) };
		}

		previous
	}

	fn restore_env(key: &str, previous: String) {
		if previous.is_empty() {
			unsafe { env::remove_var(key) };
		} else {
			unsafe { env::set_var(key, previous) };
		}
	}

	#[test]
	fn browser_redirect_uri_matches_codex() {
		assert_eq!(
			auth::browser_redirect_uri(),
			format!("http://localhost:{DEFAULT_PORT}{REDIRECT_URI_PATH}")
		);
	}

	#[test]
	fn authorize_url_includes_expected_codex_params() {
		let url = auth::build_authorize_url(
			&auth::browser_redirect_uri(),
			"challenge123",
			"state123",
			DEFAULT_ISSUER,
		);
		let parsed = Url::parse(&url).expect("valid authorize url");
		let params: HashMap<String, String> = parsed.query_pairs().into_owned().collect();

		assert_eq!(parsed.path(), "/oauth/authorize");
		assert_eq!(params.get("response_type").map(String::as_str), Some("code"));
		assert_eq!(params.get("client_id").map(String::as_str), Some(CLIENT_ID));
		assert_eq!(
			params.get("redirect_uri").map(String::as_str),
			Some(auth::browser_redirect_uri().as_str())
		);
		assert_eq!(
			params.get("scope").map(String::as_str),
			Some("openid profile email offline_access")
		);
		assert_eq!(params.get("code_challenge").map(String::as_str), Some("challenge123"));
		assert_eq!(params.get("code_challenge_method").map(String::as_str), Some("S256"));
		assert_eq!(params.get("id_token_add_organizations").map(String::as_str), Some("true"));
		assert_eq!(params.get("codex_cli_simplified_flow").map(String::as_str), Some("true"));
		assert_eq!(params.get("state").map(String::as_str), Some("state123"));
		assert_eq!(params.get("originator").map(String::as_str), Some(CODEX_OAUTH_ORIGINATOR));
	}

	#[test]
	fn status_and_access_token_use_in_memory_cache() {
		let _guard = TEST_MUTEX.lock().unwrap();

		auth::cache_session_tokens(&TokenData {
			id_token: "id-token".to_string(),
			access_token: "access-token".to_string(),
			refresh_token: None,
			account_id: Some("account-1".to_string()),
			created_at_unix: auth::now_unix(),
			expires_in_seconds: Some(3_600),
		});

		let status = auth::status();

		assert!(status.signed_in);
		assert_eq!(status.account_id, Some("account-1".to_string()));

		let token = auth::access_token().expect("token from cache");

		assert_eq!(token.0, "access-token");
		assert_eq!(token.1, Some("account-1".to_string()));

		auth::clear_cached_session_tokens();
	}

	#[test]
	fn stored_auth_cache_reuses_tokens_until_invalidation() {
		let _guard = TEST_MUTEX.lock().unwrap();

		auth::clear_cached_session_tokens();
		auth::clear_stored_auth_cache();
		auth::cache_stored_auth_tokens(Some(TokenData {
			id_token: "cache-id".to_string(),
			access_token: "cache-access".to_string(),
			refresh_token: Some("cache-refresh".to_string()),
			account_id: Some("cache-account".to_string()),
			created_at_unix: auth::now_unix(),
			expires_in_seconds: Some(3_600),
		}));

		let first = auth::load_stored_auth_tokens().expect("read cache").expect("tokens");

		assert_eq!(first.access_token, "cache-access");

		let second = auth::load_stored_auth_tokens().expect("read cache again").expect("tokens");

		assert_eq!(second.access_token, "cache-access");

		auth::cache_stored_auth_tokens(None);

		let cleared = auth::load_stored_auth_tokens().expect("read cleared cache");

		assert!(cleared.is_none());

		auth::clear_stored_auth_cache();
	}

	#[test]
	fn keyring_verification_is_disabled_by_default() {
		let _guard = TEST_MUTEX.lock().unwrap();
		let previous = set_env(KEYRING_VERIFY_ENABLED_ENV, None);

		assert!(!auth::should_verify_keyring_storage());

		restore_env(KEYRING_VERIFY_ENABLED_ENV, previous);
	}

	#[test]
	fn keyring_verification_respects_env_flag() {
		let _guard = TEST_MUTEX.lock().unwrap();
		let previous = set_env(KEYRING_VERIFY_ENABLED_ENV, Some("1"));

		assert!(auth::should_verify_keyring_storage());

		restore_env(KEYRING_VERIFY_ENABLED_ENV, previous);
	}

	#[test]
	fn fallback_to_auth_json_preserves_file_when_keyring_fails() {
		let _guard = TEST_MUTEX.lock().unwrap();
		let home = env::temp_dir().join(format!("voxit-auth-test-{}", auth::now_unix()));
		let home = home.to_string_lossy().to_string();
		let previous_home = set_env("HOME", Some(&home));
		let previous_fallback = set_env(AUTH_FILE_FALLBACK_ENV, Some("1"));
		let previous_force = set_env(TEST_FORCE_KEYRING_ERROR_ENV, Some("1"));
		let _ = fs::remove_dir_all(home.clone());
		let base = auth::app_config_dir().expect("app config dir");
		let original_payload = StoredAuth {
			auth_mode: Some("chatgpt".to_string()),
			openai_api_key: Some("legacy-key".to_string()),
			tokens: Some(TokenData {
				id_token: "legacy-id".to_string(),
				access_token: "legacy-access".to_string(),
				refresh_token: Some("legacy-refresh".to_string()),
				account_id: Some("legacy-account".to_string()),
				created_at_unix: auth::now_unix(),
				expires_in_seconds: Some(3_600),
			}),
		};
		let original_auth_json =
			serde_json::to_string_pretty(&original_payload).expect("serialize fallback auth");

		auth::save_to_file(&base, &original_auth_json).expect("seed fallback auth file");

		let tokens = TokenData {
			id_token: "new-id".to_string(),
			access_token: "new-access".to_string(),
			refresh_token: Some("new-refresh".to_string()),
			account_id: Some("new-account".to_string()),
			created_at_unix: auth::now_unix(),
			expires_in_seconds: Some(3_600),
		};

		assert!(auth::store_tokens(&tokens).is_ok());

		let saved =
			auth::load_from_file(&base).expect("read fallback auth file").expect("file exists");

		assert_eq!(saved.tokens.expect("tokens").access_token, "new-access");

		restore_env(TEST_FORCE_KEYRING_ERROR_ENV, previous_force);
		restore_env(AUTH_FILE_FALLBACK_ENV, previous_fallback);
		restore_env("HOME", previous_home);

		let _ = fs::remove_dir_all(home);
	}
}
