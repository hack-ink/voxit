//! Backward-compatible inference re-exports.

pub use crate::inference::{
	InferenceEvent, RewriteResult, RewriteSettings, RewriteState, rewrite_only,
	rewrite_only_with_plan, transcribe_only,
};
