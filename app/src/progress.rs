//! Player progress, kept on the device in localStorage (nothing leaves the phone).

use model::QuizMode;
use quiz_core::leitner;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

const KEY: &str = "ornitho.progress.v2";

#[derive(Clone, Copy, Default, Serialize, Deserialize)]
pub struct Entry {
    pub box_: u8,
    pub last_seen_day: u32,
    pub seen: u32,
    pub correct: u32,
}

#[derive(Clone, Default, Serialize, Deserialize)]
pub struct Progress(pub HashMap<String, Entry>);

fn key(mode: QuizMode, species_id: i64) -> String {
    format!("{mode:?}:{species_id}")
}

pub fn today() -> u32 {
    (js_sys::Date::now() / 86_400_000.0) as u32
}

fn storage() -> Option<web_sys::Storage> {
    web_sys::window()?.local_storage().ok()?
}

impl Progress {
    pub fn load() -> Self {
        storage()
            .and_then(|s| s.get_item(KEY).ok()?)
            .and_then(|raw| serde_json::from_str(&raw).ok())
            .unwrap_or_default()
    }

    fn save(&self) {
        if let (Some(s), Ok(raw)) = (storage(), serde_json::to_string(self)) {
            let _ = s.set_item(KEY, &raw);
        }
    }

    pub fn record(&mut self, mode: QuizMode, species_id: i64, correct: bool) {
        let e = self.0.entry(key(mode, species_id)).or_default();
        e.box_ = leitner::next_box(e.box_, correct);
        e.last_seen_day = today();
        e.seen += 1;
        e.correct += correct as u32;
        self.save();
    }

    /// How urgently a bird should be asked again (unseen birds come first).
    pub fn priority(&self, mode: QuizMode, species_id: i64) -> u32 {
        match self.0.get(&key(mode, species_id)) {
            Some(e) => leitner::priority(e.box_, Some(e.last_seen_day), today()),
            None => leitner::priority(0, None, today()),
        }
    }

    /// Total good guesses so far, across all modes.
    pub fn correct_total(&self) -> u32 {
        self.0.values().map(|e| e.correct).sum()
    }
}

/// Small string preferences (difficulty...) kept next to the progress.
pub fn load_pref(key: &str) -> Option<String> {
    storage()?.get_item(&format!("ornitho.pref.{key}")).ok()?
}

pub fn save_pref(key: &str, value: &str) {
    if let Some(s) = storage() {
        let _ = s.set_item(&format!("ornitho.pref.{key}"), value);
    }
}
