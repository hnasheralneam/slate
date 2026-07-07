use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

/// Finds all non-overlapping occurrences of `needle` in `haystack`, returning
/// (start, end) char-index pairs.
fn find_char_matches(haystack: &[char], needle: &[char]) -> Vec<(usize, usize)> {
    let mut out = Vec::new();
    let qlen = needle.len();
    if qlen == 0 || haystack.len() < qlen {
        return out;
    }
    let mut i = 0;
    while i <= haystack.len() - qlen {
        if &haystack[i..i + qlen] == needle {
            out.push((i, i + qlen));
            i += qlen;
        } else {
            i += 1;
        }
    }
    out
}

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

        let content_chars: Vec<char> = content.chars().collect();
        let q_lower: Vec<char> = self.query.chars().map(|c| c.to_lowercase().next().unwrap_or(c)).collect();
        let content_lower: Vec<char> = content_chars.iter().map(|c| c.to_lowercase().next().unwrap_or(*c)).collect();

        for (start_char, end_char) in find_char_matches(&content_lower, &q_lower) {
            self.matches.push(SearchMatch { start_char, end_char });
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

    /// Runs a global text search across the entire vault.
    /// This method is called by `tick_debounce` after the user stops typing.
    pub fn run_search(&mut self, vault_path: &Path) {
        self.results.clear();
        self.selected = 0;

        // Require at least 2 characters (char count, not byte count) to start searching.
        if self.query.chars().count() < 2 {
            return;
        }

        let q_chars: Vec<char> = self.query.chars().map(|c| c.to_lowercase().next().unwrap_or(c)).collect();

        // `WalkBuilder` is imported from the `ignore` crate.
        // It automatically handles `.gitignore` and ignores hidden files!
        let walker = ignore::WalkBuilder::new(vault_path)
            .build()
            .filter_map(|e| e.ok())
            .filter(|e| e.file_type().map_or(false, |ft| ft.is_file()));

        for entry in walker {
            if self.results.len() >= 200 {
                return;
            }

            let ext = entry
                .path()
                .extension()
                .and_then(|e| e.to_str())
                .unwrap_or("")
                .to_lowercase();

            // We only want to search within text-based files.
            // Adjust this list to support more languages/extensions!
            if !matches!(ext.as_str(), "md" | "txt" | "sh" | "rs" | "js" | "py" | "") {
                continue;
            }

            // Attempt to read the entire file into a string.
            // If the file isn't valid UTF-8, this will fail and we just skip it.
            let text = match std::fs::read_to_string(entry.path()) {
                Ok(t) => t,
                Err(_) => continue,
            };

            // Process line by line and look for all occurrences of the search query.
            for (line_no, line) in text.lines().enumerate() {
                let line_chars: Vec<char> = line.chars().collect();
                let line_lower: Vec<char> = line_chars.iter().map(|c| c.to_lowercase().next().unwrap_or(*c)).collect();

                for (col_start, col_end) in find_char_matches(&line_lower, &q_chars) {
                    self.results.push(GlobalMatch {
                        path: entry.path().to_path_buf(),
                        line_no,
                        line_text: line.to_string(),
                        col_start,
                        col_end,
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

}
