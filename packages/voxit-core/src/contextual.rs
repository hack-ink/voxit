//! Context-aware voice input planning contracts.
//!
//! Rust Core owns contextual voice behavior so native hosts can stay focused on
//! platform UI, context capture, and user confirmation.

/// User-facing interaction tier selected for a voice session.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum VoiceInteractionTier {
	/// Lowest-latency speech-to-clean-text path.
	FastDictation,
	/// App-aware rewrite path that shapes output for the destination.
	ContextRewrite,
	/// Intent-oriented path that produces a preview or action proposal.
	VoiceIntent,
}

/// Reasoning effort requested for a contextual voice session.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum VoiceReasoningEffort {
	/// Use the fastest viable reasoning path.
	Minimal,
	/// Use light reasoning for common contextual rewrites.
	Low,
	/// Use deeper reasoning for multi-step or high-precision output.
	Medium,
	/// Use the strongest reasoning for constrained or failure-sensitive output.
	High,
}

/// Policy for applying the final voice output.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum VoiceOutputPolicy {
	/// Insert or paste final text directly.
	InsertText,
	/// Show the output before insertion.
	PreviewBeforeInsert,
	/// Require confirmation before action-like output.
	ConfirmBeforeAction,
}

/// Built-in prompt profile selected by contextual routing.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PromptProfileKind {
	/// Default low-latency dictation profile.
	FastDictation,
	/// Messaging profile for short conversational destinations.
	Messaging,
	/// Mail profile for complete email prose.
	Mail,
	/// Code editor profile for programming-related dictation.
	CodeEditor,
	/// Terminal profile for command-like proposals.
	Terminal,
	/// Work tracker profile for issue, review, and planning destinations.
	WorkTracker,
}

/// Host-collected context for the app that was focused when dictation started.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct FocusedAppContext {
	/// Focused app bundle id.
	pub bundle_id: Option<String>,
	/// Focused app display name.
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
impl FocusedAppContext {
	/// Build an empty focused app context.
	pub fn new() -> Self {
		Self::default()
	}

	/// Whether the context has no routing signal.
	pub fn is_empty(&self) -> bool {
		self.bundle_id.as_deref().is_none_or(str::is_empty)
			&& self.app_name.as_deref().is_none_or(str::is_empty)
			&& self.window_title.as_deref().is_none_or(str::is_empty)
			&& self.url_domain.as_deref().is_none_or(str::is_empty)
			&& self.focused_element_role.as_deref().is_none_or(str::is_empty)
			&& !self.selected_text_present
	}

	/// Attach app identity to the context.
	pub fn with_app(mut self, bundle_id: impl Into<String>, app_name: impl Into<String>) -> Self {
		self.bundle_id = Some(bundle_id.into());
		self.app_name = Some(app_name.into());

		self
	}

	/// Attach a URL domain to the context.
	pub fn with_url_domain(mut self, url_domain: impl Into<String>) -> Self {
		self.url_domain = Some(url_domain.into());

		self
	}

	/// Attach a focused window title to the context.
	pub fn with_window_title(mut self, window_title: impl Into<String>) -> Self {
		self.window_title = Some(window_title.into());

		self
	}

	/// Attach the focused accessibility role to the context.
	pub fn with_focused_element_role(mut self, focused_element_role: impl Into<String>) -> Self {
		self.focused_element_role = Some(focused_element_role.into());

		self
	}

	/// Attach selected-text presence to the context.
	pub fn with_selected_text_present(mut self, selected_text_present: bool) -> Self {
		self.selected_text_present = selected_text_present;

		self
	}
}

/// Prompt profile selected by contextual routing.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PromptProfile {
	/// Built-in profile kind.
	pub kind: PromptProfileKind,
	/// Stable profile id.
	pub id: String,
	/// Human-readable profile title.
	pub title: String,
	/// Prompt direction applied to app-aware rewrite and future reasoning sessions.
	pub prompt_directive: String,
	/// User-facing interaction tier.
	pub tier: VoiceInteractionTier,
	/// Default reasoning effort for this profile.
	pub reasoning_effort: VoiceReasoningEffort,
	/// Default output policy for this profile.
	pub output_policy: VoiceOutputPolicy,
}
impl PromptProfile {
	/// Build a prompt profile.
	pub fn new(
		kind: PromptProfileKind,
		id: impl Into<String>,
		title: impl Into<String>,
		prompt_directive: impl Into<String>,
		tier: VoiceInteractionTier,
		reasoning_effort: VoiceReasoningEffort,
		output_policy: VoiceOutputPolicy,
	) -> Self {
		Self {
			kind,
			id: id.into(),
			title: title.into(),
			prompt_directive: prompt_directive.into(),
			tier,
			reasoning_effort,
			output_policy,
		}
	}
}

/// Concrete plan for one voice session.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct VoiceSessionPlan {
	/// Selected built-in prompt profile.
	pub profile_kind: PromptProfileKind,
	/// Selected profile id.
	pub profile_id: String,
	/// Selected profile display title.
	pub profile_title: String,
	/// Prompt direction selected for this session.
	pub prompt_directive: String,
	/// Selected interaction tier.
	pub tier: VoiceInteractionTier,
	/// Selected reasoning effort.
	pub reasoning_effort: VoiceReasoningEffort,
	/// Selected output policy.
	pub output_policy: VoiceOutputPolicy,
}
impl VoiceSessionPlan {
	fn from_profile(profile: PromptProfile) -> Self {
		Self {
			profile_kind: profile.kind,
			profile_id: profile.id,
			profile_title: profile.title,
			prompt_directive: profile.prompt_directive,
			tier: profile.tier,
			reasoning_effort: profile.reasoning_effort,
			output_policy: profile.output_policy,
		}
	}

	/// Build provider instructions for a contextual rewrite pass.
	pub fn rewrite_instructions(&self, style: &str, max_output_chars: u32) -> String {
		let max_output_chars = max_output_chars.max(1);

		format!(
			"You are Voxit, a contextual voice input layer. Rewrite the transcript for the destination app, not as generic ASR cleanup.\n\
			Active profile: {profile_title} ({profile_id}).\n\
			Profile direction: {prompt_directive}\n\
			Interaction tier: {tier}.\n\
			Reasoning effort target: {reasoning_effort}.\n\
			Output policy: {output_policy}.\n\
			Style preset: {style}.\n\
			Constraints: preserve meaning, numbers, dates, money amounts, names, identifiers, and file paths unless the user explicitly said to change them. Keep the answer under {max_output_chars} characters. Return only the final text to insert or preview.",
			profile_title = self.profile_title,
			profile_id = self.profile_id,
			prompt_directive = self.prompt_directive,
			tier = interaction_tier_label(self.tier),
			reasoning_effort = reasoning_effort_label(self.reasoning_effort),
			output_policy = output_policy_label(self.output_policy),
		)
	}
}

/// Deterministic router from focused app context to a voice session plan.
#[derive(Clone, Debug, Default)]
pub struct ContextualVoiceRouter;
impl ContextualVoiceRouter {
	/// Plan a contextual voice session from focused app context.
	pub fn plan_for_context(&self, context: &FocusedAppContext) -> VoiceSessionPlan {
		let profile = if context_matches_any(context, &["com.tinyspeck.slackmacgap", "discord"]) {
			messaging_profile()
		} else if context_matches_any(context, &["com.apple.mail"]) {
			mail_profile()
		} else if context_matches_any(context, &["cursor", "vscode", "xcode"]) {
			code_editor_profile()
		} else if context_matches_any(context, &["terminal", "iterm"]) {
			terminal_profile()
		} else if domain_matches_any(context, &["linear.app", "github.com"]) {
			work_tracker_profile()
		} else {
			default_dictation_profile()
		};

		VoiceSessionPlan::from_profile(profile)
	}
}

fn context_matches_any(context: &FocusedAppContext, needles: &[&str]) -> bool {
	context_text(context).is_some_and(|text| needles.iter().any(|needle| text.contains(needle)))
}

fn domain_matches_any(context: &FocusedAppContext, domains: &[&str]) -> bool {
	context
		.url_domain
		.as_deref()
		.map(str::to_ascii_lowercase)
		.is_some_and(|domain| domains.iter().any(|needle| domain.ends_with(needle)))
}

fn context_text(context: &FocusedAppContext) -> Option<String> {
	let mut values = Vec::new();

	if let Some(bundle_id) = context.bundle_id.as_deref() {
		values.push(bundle_id);
	}
	if let Some(app_name) = context.app_name.as_deref() {
		values.push(app_name);
	}

	if values.is_empty() { None } else { Some(values.join(" ").to_ascii_lowercase()) }
}

fn default_dictation_profile() -> PromptProfile {
	PromptProfile::new(
		PromptProfileKind::FastDictation,
		"fast-dictation",
		"Fast Dictation",
		"Clean punctuation and readability with the lowest possible latency. Do not expand terse speech into a different structure.",
		VoiceInteractionTier::FastDictation,
		VoiceReasoningEffort::Minimal,
		VoiceOutputPolicy::InsertText,
	)
}

fn messaging_profile() -> PromptProfile {
	PromptProfile::new(
		PromptProfileKind::Messaging,
		"messaging",
		"Messaging",
		"Shape the transcript as a concise conversational message for chat. Prefer natural short paragraphs and avoid email-like signoffs.",
		VoiceInteractionTier::ContextRewrite,
		VoiceReasoningEffort::Low,
		VoiceOutputPolicy::InsertText,
	)
}

fn mail_profile() -> PromptProfile {
	PromptProfile::new(
		PromptProfileKind::Mail,
		"mail",
		"Mail",
		"Shape the transcript as complete but restrained email prose. Preserve intent while adding only necessary greeting, punctuation, and paragraph structure.",
		VoiceInteractionTier::ContextRewrite,
		VoiceReasoningEffort::Low,
		VoiceOutputPolicy::PreviewBeforeInsert,
	)
}

fn code_editor_profile() -> PromptProfile {
	PromptProfile::new(
		PromptProfileKind::CodeEditor,
		"code-editor",
		"Code Editor",
		"Shape the transcript for code-editing work. Preserve symbols, identifiers, filenames, APIs, and quoted code-like phrases exactly when possible.",
		VoiceInteractionTier::ContextRewrite,
		VoiceReasoningEffort::Low,
		VoiceOutputPolicy::PreviewBeforeInsert,
	)
}

fn terminal_profile() -> PromptProfile {
	PromptProfile::new(
		PromptProfileKind::Terminal,
		"terminal",
		"Terminal",
		"Produce a terminal-focused command proposal or explanation. Never imply that a command has run; keep risky shell actions clearly previewable.",
		VoiceInteractionTier::VoiceIntent,
		VoiceReasoningEffort::Medium,
		VoiceOutputPolicy::ConfirmBeforeAction,
	)
}

fn work_tracker_profile() -> PromptProfile {
	PromptProfile::new(
		PromptProfileKind::WorkTracker,
		"work-tracker",
		"Work Tracker",
		"Shape the transcript as a practical issue, pull request, review, status, or acceptance-criteria note. Prefer concrete bullets when they improve scanability.",
		VoiceInteractionTier::ContextRewrite,
		VoiceReasoningEffort::Medium,
		VoiceOutputPolicy::PreviewBeforeInsert,
	)
}

fn interaction_tier_label(tier: VoiceInteractionTier) -> &'static str {
	match tier {
		VoiceInteractionTier::FastDictation => "fast_dictation",
		VoiceInteractionTier::ContextRewrite => "context_rewrite",
		VoiceInteractionTier::VoiceIntent => "voice_intent",
	}
}

fn reasoning_effort_label(effort: VoiceReasoningEffort) -> &'static str {
	match effort {
		VoiceReasoningEffort::Minimal => "minimal",
		VoiceReasoningEffort::Low => "low",
		VoiceReasoningEffort::Medium => "medium",
		VoiceReasoningEffort::High => "high",
	}
}

fn output_policy_label(policy: VoiceOutputPolicy) -> &'static str {
	match policy {
		VoiceOutputPolicy::InsertText => "insert_text",
		VoiceOutputPolicy::PreviewBeforeInsert => "preview_before_insert",
		VoiceOutputPolicy::ConfirmBeforeAction => "confirm_before_action",
	}
}

#[cfg(test)]
mod tests {
	use crate::contextual::{
		ContextualVoiceRouter, FocusedAppContext, PromptProfileKind, VoiceInteractionTier,
		VoiceOutputPolicy, VoiceReasoningEffort,
	};

	#[test]
	fn default_context_uses_fast_dictation() {
		let router = ContextualVoiceRouter;
		let plan = router.plan_for_context(&FocusedAppContext::new());

		assert_eq!(plan.profile_kind, PromptProfileKind::FastDictation);
		assert_eq!(plan.profile_id, "fast-dictation");
		assert_eq!(plan.tier, VoiceInteractionTier::FastDictation);
		assert_eq!(plan.output_policy, VoiceOutputPolicy::InsertText);
		assert_eq!(plan.reasoning_effort, VoiceReasoningEffort::Minimal);
	}

	#[test]
	fn slack_context_uses_messaging_profile() {
		let router = ContextualVoiceRouter;
		let context = FocusedAppContext::new().with_app("com.tinyspeck.slackmacgap", "Slack");
		let plan = router.plan_for_context(&context);

		assert_eq!(plan.profile_kind, PromptProfileKind::Messaging);
		assert_eq!(plan.profile_id, "messaging");
		assert_eq!(plan.tier, VoiceInteractionTier::ContextRewrite);
		assert_eq!(plan.output_policy, VoiceOutputPolicy::InsertText);
	}

	#[test]
	fn cursor_context_previews_code_editor_output() {
		let router = ContextualVoiceRouter;
		let context = FocusedAppContext::new().with_app("com.todesktop.230313mzl4w4u92", "Cursor");
		let plan = router.plan_for_context(&context);

		assert_eq!(plan.profile_kind, PromptProfileKind::CodeEditor);
		assert_eq!(plan.profile_id, "code-editor");
		assert_eq!(plan.output_policy, VoiceOutputPolicy::PreviewBeforeInsert);
	}

	#[test]
	fn terminal_context_requires_confirmation() {
		let router = ContextualVoiceRouter;
		let context = FocusedAppContext::new().with_app("com.apple.Terminal", "Terminal");
		let plan = router.plan_for_context(&context);

		assert_eq!(plan.profile_kind, PromptProfileKind::Terminal);
		assert_eq!(plan.profile_id, "terminal");
		assert_eq!(plan.tier, VoiceInteractionTier::VoiceIntent);
		assert_eq!(plan.output_policy, VoiceOutputPolicy::ConfirmBeforeAction);
		assert_eq!(plan.reasoning_effort, VoiceReasoningEffort::Medium);
	}

	#[test]
	fn linear_domain_uses_work_tracker_profile() {
		let router = ContextualVoiceRouter;
		let context = FocusedAppContext::new()
			.with_app("com.apple.Safari", "Safari")
			.with_url_domain("linear.app");
		let plan = router.plan_for_context(&context);

		assert_eq!(plan.profile_kind, PromptProfileKind::WorkTracker);
		assert_eq!(plan.profile_id, "work-tracker");
		assert_eq!(plan.reasoning_effort, VoiceReasoningEffort::Medium);
	}

	#[test]
	fn rewrite_instructions_include_profile_policy_and_limits() {
		let router = ContextualVoiceRouter;
		let context = FocusedAppContext::new().with_app("com.apple.Terminal", "Terminal");
		let plan = router.plan_for_context(&context);
		let instructions = plan.rewrite_instructions("concise", 1200);

		assert!(instructions.contains("Terminal"));
		assert!(instructions.contains("confirm_before_action"));
		assert!(instructions.contains("concise"));
		assert!(instructions.contains("1200"));
	}
}
