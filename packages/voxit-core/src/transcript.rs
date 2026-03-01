//! Transcript assembly for Pass1 realtime events.

use std::collections::{HashMap, HashSet};

/// Transcript update from a realtime event stream.
#[derive(Debug, Clone)]
pub enum TranscriptEvent {
	/// Incremental partial text for an in-progress speech item.
	Delta {
		/// Stable OpenAI realtime item identifier.
		item_id: String,
		/// Prior item id in the finalized chain.
		previous_item_id: Option<String>,
		/// Partial text chunk.
		delta: String,
	},
	/// Finalized transcript text for one speech item.
	Completed {
		/// Stable OpenAI realtime item identifier.
		item_id: String,
		/// Prior item id in the finalized chain.
		previous_item_id: Option<String>,
		/// Final transcript text for this item.
		transcript: String,
	},
}

/// Rendered snapshot of the current committed + draft transcript state.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct TranscriptSnapshot {
	/// Committed transcript, kept in deterministic `previous_item_id` order.
	pub committed: String,
	/// Current draft text (latest active not-yet-committed item).
	pub draft: String,
	/// Draft item id when draft exists.
	pub draft_item_id: Option<String>,
}

/// Assembles `delta` and `completed` events into stable committed + draft text.
#[derive(Debug, Default)]
pub struct TranscriptAssembler {
	committed: HashMap<String, CommittedItem>,
	drafts: HashMap<String, DraftItem>,
	arrival_seq: u64,
	dirty_order: bool,
	ordered_committed_ids: Vec<String>,
}
impl TranscriptAssembler {
	/// Build a new transcript assembler.
	pub fn new() -> Self {
		Self::default()
	}

	/// Handle a realtime item delta event.
	pub fn on_delta(&mut self, item_id: String, _previous_item_id: Option<String>, delta: String) {
		let seen_seq = self.bump_seq();
		let draft = self.drafts.entry(item_id).or_default();

		draft.seen_seq = seen_seq;

		draft.text.push_str(&delta);
	}

	/// Handle a realtime item completed event.
	pub fn on_completed(
		&mut self,
		item_id: String,
		previous_item_id: Option<String>,
		transcript: String,
	) {
		let seen_seq = self.bump_seq();

		self.committed.insert(
			item_id.clone(),
			CommittedItem { text: transcript, previous_item_id, seen_seq },
		);
		self.drafts.remove(&item_id);

		self.dirty_order = true;
	}

	/// Reset all state for a fresh session.
	pub fn reset(&mut self) {
		self.committed.clear();
		self.drafts.clear();

		self.arrival_seq = 0;
		self.dirty_order = true;

		self.ordered_committed_ids.clear();
	}

	/// Apply a combined transcript event.
	pub fn apply(&mut self, event: TranscriptEvent) {
		match event {
			TranscriptEvent::Delta { item_id, previous_item_id, delta } =>
				self.on_delta(item_id, previous_item_id, delta),
			TranscriptEvent::Completed { item_id, previous_item_id, transcript } =>
				self.on_completed(item_id, previous_item_id, transcript),
		}
	}

	/// Current committed transcript text.
	pub fn committed_text(&mut self) -> String {
		if self.dirty_order {
			self.rebuild_order();

			self.dirty_order = false;
		}

		self.ordered_committed_ids
			.iter()
			.filter_map(|item_id| self.committed.get(item_id))
			.map(|entry| entry.text.as_str())
			.collect::<Vec<_>>()
			.join(" ")
	}

	/// Current draft text, if any.
	pub fn draft_text(&self) -> String {
		self.drafts
			.iter()
			.max_by_key(|(_, draft)| draft.seen_seq)
			.map(|(_, draft)| draft.text.clone())
			.unwrap_or_default()
	}

	/// Current draft item id, if any.
	pub fn draft_item_id(&self) -> Option<String> {
		self.drafts.iter().max_by_key(|(_, draft)| draft.seen_seq).map(|(id, _)| id.clone())
	}

	/// Snapshot committed + draft text in one call.
	pub fn snapshot(&mut self) -> TranscriptSnapshot {
		TranscriptSnapshot {
			committed: self.committed_text(),
			draft: self.draft_text(),
			draft_item_id: self.draft_item_id(),
		}
	}

	fn bump_seq(&mut self) -> u64 {
		let current = self.arrival_seq;

		self.arrival_seq += 1;

		current
	}

	fn rebuild_order(&mut self) {
		let mut next_map: HashMap<Option<String>, Vec<(String, u64)>> = HashMap::new();
		let mut item_ids: HashSet<String> = HashSet::new();

		for (item_id, item) in &self.committed {
			item_ids.insert(item_id.clone());
			next_map
				.entry(item.previous_item_id.clone())
				.or_default()
				.push((item_id.clone(), item.seen_seq));
		}
		for siblings in next_map.values_mut() {
			siblings.sort_by_key(|(_, seen_seq)| *seen_seq);
		}

		let mut ordered_ids = Vec::with_capacity(self.committed.len());
		let mut consumed = HashSet::new();
		let mut frontier = next_map.remove(&None).unwrap_or_default();

		frontier.sort_by_key(|(_, seen_seq)| *seen_seq);

		for (item_id, _) in frontier {
			self.visit_chain(&item_id, &next_map, &mut consumed, &mut ordered_ids);
		}

		let mut remaining: Vec<(u64, String)> = item_ids
			.into_iter()
			.filter_map(|item_id| self.committed.get(&item_id).map(|item| (item.seen_seq, item_id)))
			.collect();

		remaining.sort_by_key(|(seen_seq, _)| *seen_seq);

		for (_, item_id) in remaining {
			self.visit_chain(&item_id, &next_map, &mut consumed, &mut ordered_ids);
		}

		self.ordered_committed_ids = ordered_ids;
	}

	fn visit_chain(
		&self,
		item_id: &str,
		next_map: &HashMap<Option<String>, Vec<(String, u64)>>,
		consumed: &mut HashSet<String>,
		ordered_ids: &mut Vec<String>,
	) {
		if !consumed.insert(item_id.to_string()) {
			return;
		}

		ordered_ids.push(item_id.to_string());

		let next_id = Some(item_id.to_string());

		if let Some(children) = next_map.get(&next_id) {
			for (child_id, _) in children {
				self.visit_chain(child_id, next_map, consumed, ordered_ids);
			}
		}
	}
}

#[derive(Debug, Default)]
struct CommittedItem {
	text: String,
	previous_item_id: Option<String>,
	seen_seq: u64,
}

#[derive(Debug, Default)]
struct DraftItem {
	text: String,
	seen_seq: u64,
}

#[cfg(test)]
mod tests {
	use crate::transcript::{TranscriptAssembler, TranscriptEvent};

	#[test]
	fn delta_then_completed_updates_transcript() {
		let mut assembler = TranscriptAssembler::new();

		assembler.apply(TranscriptEvent::Delta {
			item_id: "item-1".to_string(),
			previous_item_id: None,
			delta: "hel".to_string(),
		});
		assembler.apply(TranscriptEvent::Delta {
			item_id: "item-1".to_string(),
			previous_item_id: None,
			delta: "lo".to_string(),
		});
		assembler.apply(TranscriptEvent::Completed {
			item_id: "item-1".to_string(),
			previous_item_id: None,
			transcript: "hello".to_string(),
		});

		let snapshot = assembler.snapshot();

		assert_eq!(snapshot.committed, "hello");
		assert_eq!(snapshot.draft, "");
	}

	#[test]
	fn committed_items_are_ordered_by_previous_chain() {
		let mut assembler = TranscriptAssembler::new();

		assembler.apply(TranscriptEvent::Completed {
			item_id: "item-c".to_string(),
			previous_item_id: Some("item-b".to_string()),
			transcript: "c".to_string(),
		});
		assembler.apply(TranscriptEvent::Completed {
			item_id: "item-b".to_string(),
			previous_item_id: Some("item-a".to_string()),
			transcript: "b".to_string(),
		});
		assembler.apply(TranscriptEvent::Completed {
			item_id: "item-a".to_string(),
			previous_item_id: None,
			transcript: "a".to_string(),
		});

		assert_eq!(assembler.committed_text(), "a b c");
	}

	#[test]
	fn completed_arrival_does_not_change_latest_draft_id() {
		let mut assembler = TranscriptAssembler::new();

		assembler.apply(TranscriptEvent::Delta {
			item_id: "item-2".to_string(),
			previous_item_id: None,
			delta: "draft".to_string(),
		});
		assembler.apply(TranscriptEvent::Completed {
			item_id: "item-1".to_string(),
			previous_item_id: None,
			transcript: "final".to_string(),
		});

		assert_eq!(assembler.committed_text(), "final");
		assert_eq!(assembler.draft_text(), "draft");
	}

	#[test]
	fn completed_chain_uses_previous_item_id_even_with_arrival_reordering() {
		let mut assembler = TranscriptAssembler::new();

		assembler.apply(TranscriptEvent::Completed {
			item_id: "item-2".to_string(),
			previous_item_id: Some("item-1".to_string()),
			transcript: "two".to_string(),
		});
		assembler.apply(TranscriptEvent::Completed {
			item_id: "item-1".to_string(),
			previous_item_id: None,
			transcript: "one".to_string(),
		});
		assembler.apply(TranscriptEvent::Completed {
			item_id: "item-3".to_string(),
			previous_item_id: Some("item-2".to_string()),
			transcript: "three".to_string(),
		});

		assert_eq!(assembler.committed_text(), "one two three");
	}
}
