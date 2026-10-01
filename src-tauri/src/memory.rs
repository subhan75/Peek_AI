use std::collections::{HashMap, VecDeque};
use std::sync::Mutex;

/// Capped per-app, so a long session in one app can't grow memory or prompt
/// size unboundedly; kept in-memory only (never persisted to disk).
const MAX_TURNS_PER_APP: usize = 10;

struct ConversationTurn {
    query: String,
    answer: String,
    /// The previous turn's raw annotations, verbatim as Gemini produced them
    /// (JSON array, box_2d still in its native 0-1000 space) -- echoed back
    /// so a follow-up about the same subject can reuse these exact
    /// coordinates instead of re-deriving (and drifting from) them.
    annotations_json: String,
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
            .map(|t| {
                if t.annotations_json == "[]" {
                    format!("User: {}\nAssistant: {}", t.query, t.answer)
                } else {
                    format!(
                        "User: {}\nAssistant: {}\nAssistant's annotations for that answer (box_2d in 0-1000 screen coordinates): {}",
                        t.query, t.answer, t.annotations_json
                    )
                }
            })
            .collect::<Vec<_>>()
            .join("\n\n")
    }

    pub fn push_turn(&self, app_id: &str, query: String, answer: String, annotations_json: String) {
        let mut store = self.0.lock().unwrap();
        let turns = store.entry(app_id.to_string()).or_default();
        turns.push_back(ConversationTurn { query, answer, annotations_json });
        while turns.len() > MAX_TURNS_PER_APP {
            turns.pop_front();
        }
    }
}
