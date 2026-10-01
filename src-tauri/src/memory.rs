use std::collections::{HashMap, VecDeque};
use std::sync::Mutex;

/// Capped per-app, so a long session in one app can't grow memory or prompt
/// size unboundedly; kept in-memory only (never persisted to disk).
const MAX_TURNS_PER_APP: usize = 10;

struct ConversationTurn {
    query: String,
    answer: String,
}

#[derive(Default)]
pub struct ConversationStore(Mutex<HashMap<String, VecDeque<ConversationTurn>>>);

impl ConversationStore {
    pub fn history_text(&self, app_id: &str) -> String {
        let store = self.0.lock().unwrap();
        let Some(turns) = store.get(app_id) else {
            return String::new();
        };
        turns
            .iter()
            .map(|t| format!("User: {}\nAssistant: {}", t.query, t.answer))
            .collect::<Vec<_>>()
            .join("\n\n")
    }

    pub fn push_turn(&self, app_id: &str, query: String, answer: String) {
        let mut store = self.0.lock().unwrap();
        let turns = store.entry(app_id.to_string()).or_default();
        turns.push_back(ConversationTurn { query, answer });
        while turns.len() > MAX_TURNS_PER_APP {
            turns.pop_front();
        }
    }
}
