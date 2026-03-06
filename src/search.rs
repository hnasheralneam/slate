use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

#[derive(Debug, Clone)]
pub struct SearchMatch {
    pub start_char: usize,
    pub end_char: usize,
}

pub struct SearchState {
    pub query: String,
    pub matches: Vec<SearchMatch>,
    pub current: usize,
}

impl SearchState {
    pub fn new() -> Self {
        Self { query: String::new(), matches: Vec::new(), current: 0 }
    }

    pub fn recompute_matches(&mut self, content: &str) {
        self.matches.clear();
        self.current = 0;
        if self.query.is_empty() {
            return;
        }
        let q = self.query.to_lowercase();
        let lower = content.to_lowercase();
        
        let mut byte_idx = 0;
        while let Some(pos) = lower[byte_idx..].find(&q) {
            let start_byte = byte_idx + pos;
            let end_byte = start_byte + q.len();
            
            let start_char = content[..start_byte].chars().count();
            let end_char = start_char + content[start_byte..end_byte].chars().count();
            
            self.matches.push(SearchMatch { start_char, end_char });
            byte_idx = end_byte;
        }
    }

    pub fn next_match(&mut self, content: &str) {
        if self.matches.is_empty() {
            self.recompute_matches(content);
        }
        if !self.matches.is_empty() {
            self.current = (self.current + 1) % self.matches.len();
        }
    }

    pub fn prev_match(&mut self, content: &str) {
        if self.matches.is_empty() {
            self.recompute_matches(content);
        }
        if !self.matches.is_empty() {
            if self.current == 0 {
                self.current = self.matches.len() - 1;
            } else {
                self.current -= 1;
            }
        }
    }

    pub fn current_match(&self) -> Option<&SearchMatch> {
        self.matches.get(self.current)
    }
}

// Global search result
#[derive(Debug, Clone)]
pub struct GlobalMatch {
    pub path: PathBuf,
    pub line_no: usize,
    pub line_text: String,
    pub col_start: usize,
    pub col_end: usize,
}

pub struct GlobalSearch {
    pub query: String,
    pub results: Vec<GlobalMatch>,
    pub selected: usize,
    pub dirty: bool,
    pub last_typed: Option<Instant>,
}

impl GlobalSearch {
    pub fn new() -> Self {
        Self {
            query: String::new(),
            results: Vec::new(),
            selected: 0,
            dirty: false,
            last_typed: None,
        }
    }

    /// Call this on every keypress instead of run_search directly
    pub fn mark_dirty(&mut self) {
        self.dirty = true;
        self.last_typed = Some(Instant::now());
    }

    /// Returns true if it actually ran the search
    pub fn tick_debounce(&mut self, vault_path: &Path) -> bool {
        if !self.dirty {
            return false;
        }
        let ready = self
            .last_typed
            .map(|t| t.elapsed() >= Duration::from_millis(150))
            .unwrap_or(false);
        if ready {
            self.run_search(vault_path);
            self.dirty = false;
            true
        } else {
            false
        }
    }

    pub fn run_search(&mut self, vault_path: &Path) {
        self.results.clear();
        self.selected = 0;
        if self.query.len() < 2 {
            return;
        }
        let q = self.query.to_lowercase();

        for entry in walkdir::WalkDir::new(vault_path)
            .follow_links(true)
            .into_iter()
            .filter_map(|e| e.ok())
        {
            if !entry.file_type().is_file() {
                continue;
            }
            let ext = entry
                .path()
                .extension()
                .and_then(|e| e.to_str())
                .unwrap_or("")
                .to_lowercase();
            if !matches!(ext.as_str(), "md" | "txt" | "sh" | "") {
                continue;
            }

            let text = match std::fs::read_to_string(entry.path()) {
                Ok(t) => t,
                Err(_) => continue,
            };

            for (line_no, line) in text.lines().enumerate() {
                let lower = line.to_lowercase();
                if let Some(pos) = lower.find(&q) {
                    self.results.push(GlobalMatch {
                        path: entry.path().to_path_buf(),
                        line_no,
                        line_text: line.to_string(),
                        col_start: pos,
                        col_end: pos + q.len(),
                    });
                    if self.results.len() >= 200 {
                        return;
                    }
                }
            }
        }
    }

    pub fn move_down(&mut self) {
        if self.selected + 1 < self.results.len() {
            self.selected += 1;
        }
    }

    pub fn move_up(&mut self) {
        self.selected = self.selected.saturating_sub(1);
    }

    pub fn selected_result(&self) -> Option<(&PathBuf, usize, &str)> {
        self.results
            .get(self.selected)
            .map(|m| (&m.path, m.line_no, m.line_text.as_str()))
    }
}
