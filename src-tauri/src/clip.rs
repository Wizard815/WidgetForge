//! The `clipboard` source: recent copied text, kept in memory only.
//! Nothing is written to disk, because people copy passwords.

use serde_json::{json, Value};

const MAX_ITEMS: usize = 25;
const MAX_CHARS: usize = 5000;

pub struct ClipState {
    board: Option<arboard::Clipboard>,
    history: Vec<String>,
    last: Option<String>,
}

// arboard's clipboard handle is only touched while AppState.clipboard is locked.
unsafe impl Send for ClipState {}

impl ClipState {
    pub fn new() -> Self {
        Self { board: None, history: Vec::new(), last: None }
    }

    fn board(&mut self) -> Result<&mut arboard::Clipboard, String> {
        if self.board.is_none() {
            self.board = Some(arboard::Clipboard::new().map_err(|e| e.to_string())?);
        }
        Ok(self.board.as_mut().unwrap())
    }

    fn push(&mut self, text: String) {
        if text.trim().is_empty() {
            return;
        }
        let text: String = text.chars().take(MAX_CHARS).collect();
        self.history.retain(|t| *t != text);
        self.history.insert(0, text);
        self.history.truncate(MAX_ITEMS);
    }

    /// Reads the clipboard, records new text, and returns the history.
    pub fn poll(&mut self) -> Value {
        if let Ok(board) = self.board() {
            if let Ok(text) = board.get_text() {
                if self.last.as_ref() != Some(&text) {
                    self.last = Some(text.clone());
                    self.push(text);
                }
            }
        }
        self.value()
    }

    pub fn copy(&mut self, text: String) -> Result<(), String> {
        self.board()?.set_text(text.clone()).map_err(|e| e.to_string())?;
        self.last = Some(text.clone());
        self.push(text);
        Ok(())
    }

    pub fn clear(&mut self) {
        self.history.clear();
    }

    fn value(&self) -> Value {
        let items: Vec<Value> = self
            .history
            .iter()
            .map(|t| {
                let first = t.lines().find(|l| !l.trim().is_empty()).unwrap_or("").trim();
                let preview: String = first.chars().take(120).collect();
                json!({ "text": t, "preview": preview, "lines": t.lines().count(), "chars": t.chars().count() })
            })
            .collect();
        json!({ "items": items, "count": items.len() })
    }
}
