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
}

/// Prompt profile selected by contextual routing.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PromptProfile {
	/// Stable profile id.
	pub id: String,
	/// Human-readable profile title.
	pub title: String,
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
		id: impl Into<String>,
		title: impl Into<String>,
		tier: VoiceInteractionTier,
		reasoning_effort: VoiceReasoningEffort,
		output_policy: VoiceOutputPolicy,
	) -> Self {
		Self { id: id.into(), title: title.into(), tier, reasoning_effort, output_policy }
	}
}

/// Concrete plan for one voice session.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct VoiceSessionPlan {
	/// Selected profile id.
	pub profile_id: String,
	/// Selected profile display title.
	pub profile_title: String,
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
			profile_id: profile.id,
			profile_title: profile.title,
			tier: profile.tier,
			reasoning_effort: profile.reasoning_effort,
			output_policy: profile.output_policy,
		}
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
		"fast-dictation",
		"Fast Dictation",
		VoiceInteractionTier::FastDictation,
		VoiceReasoningEffort::Minimal,
		VoiceOutputPolicy::InsertText,
	)
}

fn messaging_profile() -> PromptProfile {
	PromptProfile::new(
		"messaging",
		"Messaging",
		VoiceInteractionTier::ContextRewrite,
		VoiceReasoningEffort::Low,
		VoiceOutputPolicy::InsertText,
	)
}

fn mail_profile() -> PromptProfile {
	PromptProfile::new(
		"mail",
		"Mail",
		VoiceInteractionTier::ContextRewrite,
		VoiceReasoningEffort::Low,
		VoiceOutputPolicy::PreviewBeforeInsert,
	)
}

fn code_editor_profile() -> PromptProfile {
	PromptProfile::new(
		"code-editor",
		"Code Editor",
		VoiceInteractionTier::ContextRewrite,
		VoiceReasoningEffort::Low,
		VoiceOutputPolicy::PreviewBeforeInsert,
	)
}

fn terminal_profile() -> PromptProfile {
	PromptProfile::new(
		"terminal",
		"Terminal",
		VoiceInteractionTier::VoiceIntent,
		VoiceReasoningEffort::Medium,
		VoiceOutputPolicy::ConfirmBeforeAction,
	)
}

fn work_tracker_profile() -> PromptProfile {
	PromptProfile::new(
		"work-tracker",
		"Work Tracker",
		VoiceInteractionTier::ContextRewrite,
		VoiceReasoningEffort::Medium,
		VoiceOutputPolicy::PreviewBeforeInsert,
	)
}

#[cfg(test)]
mod tests {
	use crate::contextual::{
		ContextualVoiceRouter, FocusedAppContext, VoiceInteractionTier, VoiceOutputPolicy,
		VoiceReasoningEffort,
	};

	#[test]
	fn default_context_uses_fast_dictation() {
		let router = ContextualVoiceRouter;
		let plan = router.plan_for_context(&FocusedAppContext::new());

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

		assert_eq!(plan.profile_id, "messaging");
		assert_eq!(plan.tier, VoiceInteractionTier::ContextRewrite);
		assert_eq!(plan.output_policy, VoiceOutputPolicy::InsertText);
	}

	#[test]
	fn cursor_context_previews_code_editor_output() {
		let router = ContextualVoiceRouter;
		let context = FocusedAppContext::new().with_app("com.todesktop.230313mzl4w4u92", "Cursor");
		let plan = router.plan_for_context(&context);

		assert_eq!(plan.profile_id, "code-editor");
		assert_eq!(plan.output_policy, VoiceOutputPolicy::PreviewBeforeInsert);
	}

	#[test]
	fn terminal_context_requires_confirmation() {
		let router = ContextualVoiceRouter;
		let context = FocusedAppContext::new().with_app("com.apple.Terminal", "Terminal");
		let plan = router.plan_for_context(&context);

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

		assert_eq!(plan.profile_id, "work-tracker");
		assert_eq!(plan.reasoning_effort, VoiceReasoningEffort::Medium);
	}
}
