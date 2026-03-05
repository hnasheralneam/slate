use crate::filetree::FileTree;
use crate::search::{SearchState, GlobalSearch};
use anyhow::{Context, Result};
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers, MouseEvent, MouseEventKind};
use std::path::PathBuf;

#[derive(Debug, Clone, PartialEq)]
pub enum Mode {
    Normal,
    SidePanel,
    InFileSearch,
    FileOpen,      // fuzzy open by name
    GlobalSearch,  // search across all files
}

#[derive(Debug, Clone, PartialEq)]
pub enum Pane {
    Editor,
    Sidebar,
}

pub struct App {
    pub vault_path: PathBuf,
    pub mode: Mode,
    pub active_pane: Pane,

    // Sidebar
    pub sidebar_visible: bool,
    pub file_tree: FileTree,

    // Editor
    pub current_file: Option<PathBuf>,
    pub content: Vec<String>,          // raw lines
    pub scroll_offset: usize,
    pub viewport_height: usize,

    // In-file search
    pub in_file_search: SearchState,

    // File open dialog
    pub file_open: FileOpenState,

    // Global search
    pub global_search: GlobalSearch,

    pub status_msg: String,
}

pub struct FileOpenState {
    pub query: String,
    pub results: Vec<PathBuf>,
    pub selected: usize,
}

impl FileOpenState {
    pub fn new() -> Self {
        Self { query: String::new(), results: Vec::new(), selected: 0 }
    }
}

impl App {
    pub fn new(vault_path: String) -> Result<Self> {
        let vault_path = PathBuf::from(vault_path)
            .canonicalize()
            .context("Vault path not found")?;
        let file_tree = FileTree::new(&vault_path)?;

        Ok(Self {
            vault_path,
            mode: Mode::Normal,
            active_pane: Pane::Editor,
            sidebar_visible: true,
            file_tree,
            current_file: None,
            content: Vec::new(),
            scroll_offset: 0,
            viewport_height: 20,
            in_file_search: SearchState::new(),
            file_open: FileOpenState::new(),
            global_search: GlobalSearch::new(),
            status_msg: String::from("noted — press ? for help"),
        })
    }

    pub fn open_file(&mut self, path: PathBuf) -> Result<()> {
        let ext = path
            .extension()
            .and_then(|e| e.to_str())
            .unwrap_or("")
            .to_lowercase();

        let viewable = matches!(ext.as_str(), "md" | "txt" | "sh" | "");

        if !viewable {
            self.status_msg = format!("Opening {} in default app…", path.display());
            let _ = open::that(&path);
            return Ok(());
        }

        let text = std::fs::read_to_string(&path)
            .context("Could not read file")?;
        self.content = text.lines().map(|l| l.to_string()).collect();
        self.current_file = Some(path.clone());
        self.scroll_offset = 0;
        self.in_file_search = SearchState::new();
        self.status_msg = format!("{}", path.display());
        Ok(())
    }

    pub fn handle_key(&mut self, key: KeyEvent) -> Result<bool> {
        match &self.mode {
            Mode::InFileSearch => return self.handle_key_in_file_search(key),
            Mode::FileOpen     => return self.handle_key_file_open(key),
            Mode::GlobalSearch => return self.handle_key_global_search(key),
            _ => {}
        }

        // Sidebar navigation mode
        if self.mode == Mode::SidePanel {
            return self.handle_key_sidebar(key);
        }

        // Normal mode
        match (key.modifiers, key.code) {
            // Quit
            (KeyModifiers::CONTROL, KeyCode::Char('q')) |
            (KeyModifiers::NONE, KeyCode::Char('q')) => return Ok(true),

            // Toggle sidebar
            (KeyModifiers::CONTROL, KeyCode::Char('b')) => {
                self.sidebar_visible = !self.sidebar_visible;
            }

            // Focus sidebar
            (KeyModifiers::CONTROL, KeyCode::Char('e')) => {
                if self.sidebar_visible {
                    self.mode = Mode::SidePanel;
                    self.active_pane = Pane::Sidebar;
                }
            }

            // In-file search
            (KeyModifiers::CONTROL, KeyCode::Char('f')) => {
                self.mode = Mode::InFileSearch;
                self.in_file_search = SearchState::new();
            }

            // File open by name
            (KeyModifiers::CONTROL, KeyCode::Char('p')) => {
                self.mode = Mode::FileOpen;
                self.file_open = FileOpenState::new();
                self.refresh_file_open_results();
            }

            // Global search
            (KeyModifiers::CONTROL, KeyCode::Char('g')) => {
                self.mode = Mode::GlobalSearch;
                self.global_search = GlobalSearch::new();
            }

            // Scroll
            (KeyModifiers::NONE, KeyCode::Down) |
            (KeyModifiers::NONE, KeyCode::Char('j')) => {
                if self.scroll_offset + self.viewport_height < self.content.len() {
                    self.scroll_offset += 1;
                }
            }
            (KeyModifiers::NONE, KeyCode::Up) |
            (KeyModifiers::NONE, KeyCode::Char('k')) => {
                self.scroll_offset = self.scroll_offset.saturating_sub(1);
            }
            (KeyModifiers::NONE, KeyCode::PageDown) => {
                let step = self.viewport_height.saturating_sub(2);
                self.scroll_offset = (self.scroll_offset + step)
                    .min(self.content.len().saturating_sub(1));
            }
            (KeyModifiers::NONE, KeyCode::PageUp) => {
                let step = self.viewport_height.saturating_sub(2);
                self.scroll_offset = self.scroll_offset.saturating_sub(step);
            }
            (KeyModifiers::NONE, KeyCode::Home) |
            (KeyModifiers::NONE, KeyCode::Char('g')) => {
                self.scroll_offset = 0;
            }
            (KeyModifiers::NONE, KeyCode::End) |
            (KeyModifiers::SHIFT, KeyCode::Char('G')) => {
                self.scroll_offset = self.content.len().saturating_sub(1);
            }

            // Next/prev search match
            (KeyModifiers::NONE, KeyCode::Char('n')) => {
                self.in_file_search.next_match(&self.content);
                if let Some(line) = self.in_file_search.current_match_line() {
                    self.scroll_to_line(line);
                }
            }
            (KeyModifiers::SHIFT, KeyCode::Char('N')) => {
                self.in_file_search.prev_match(&self.content);
                if let Some(line) = self.in_file_search.current_match_line() {
                    self.scroll_to_line(line);
                }
            }

            _ => {}
        }
        Ok(false)
    }

    fn handle_key_sidebar(&mut self, key: KeyEvent) -> Result<bool> {
        match (key.modifiers, key.code) {
            (KeyModifiers::NONE, KeyCode::Esc) => {
                self.mode = Mode::Normal;
                self.active_pane = Pane::Editor;
            }
            (KeyModifiers::CONTROL, KeyCode::Char('b')) => {
                self.sidebar_visible = !self.sidebar_visible;
                self.mode = Mode::Normal;
                self.active_pane = Pane::Editor;
            }
            (KeyModifiers::NONE, KeyCode::Down) |
            (KeyModifiers::NONE, KeyCode::Char('j')) => {
                self.file_tree.move_down();
            }
            (KeyModifiers::NONE, KeyCode::Up) |
            (KeyModifiers::NONE, KeyCode::Char('k')) => {
                self.file_tree.move_up();
            }
            (KeyModifiers::NONE, KeyCode::Enter) |
            (KeyModifiers::NONE, KeyCode::Char('l')) |
            (KeyModifiers::NONE, KeyCode::Right) => {
                if let Some(path) = self.file_tree.selected_path() {
                    if path.is_dir() {
                        self.file_tree.toggle_expand();
                    } else {
                        let p = path.clone();
                        self.open_file(p)?;
                        self.mode = Mode::Normal;
                        self.active_pane = Pane::Editor;
                    }
                }
            }
            (KeyModifiers::NONE, KeyCode::Char('h')) |
            (KeyModifiers::NONE, KeyCode::Left) => {
                self.file_tree.collapse_or_parent();
            }
            (KeyModifiers::NONE, KeyCode::Char(' ')) => {
                self.file_tree.toggle_expand();
            }
            _ => {}
        }
        Ok(false)
    }

    fn handle_key_in_file_search(&mut self, key: KeyEvent) -> Result<bool> {
        match (key.modifiers, key.code) {
            (KeyModifiers::NONE, KeyCode::Esc) => {
                self.mode = Mode::Normal;
            }
            (KeyModifiers::NONE, KeyCode::Enter) => {
                self.in_file_search.next_match(&self.content);
                if let Some(line) = self.in_file_search.current_match_line() {
                    self.scroll_to_line(line);
                }
                self.mode = Mode::Normal;
            }
            (KeyModifiers::NONE, KeyCode::Backspace) => {
                self.in_file_search.query.pop();
                self.in_file_search.recompute_matches(&self.content);
            }
            (KeyModifiers::NONE, KeyCode::Char(c)) => {
                self.in_file_search.query.push(c);
                self.in_file_search.recompute_matches(&self.content);
                if let Some(line) = self.in_file_search.current_match_line() {
                    self.scroll_to_line(line);
                }
            }
            _ => {}
        }
        Ok(false)
    }

    fn handle_key_file_open(&mut self, key: KeyEvent) -> Result<bool> {
        match (key.modifiers, key.code) {
            (KeyModifiers::NONE, KeyCode::Esc) => {
                self.mode = Mode::Normal;
            }
            (KeyModifiers::NONE, KeyCode::Enter) => {
                if let Some(path) = self.file_open.results.get(self.file_open.selected).cloned() {
                    self.open_file(path)?;
                }
                self.mode = Mode::Normal;
            }
            (KeyModifiers::NONE, KeyCode::Down) |
            (KeyModifiers::CONTROL, KeyCode::Char('j')) => {
                if self.file_open.selected + 1 < self.file_open.results.len() {
                    self.file_open.selected += 1;
                }
            }
            (KeyModifiers::NONE, KeyCode::Up) |
            (KeyModifiers::CONTROL, KeyCode::Char('k')) => {
                self.file_open.selected = self.file_open.selected.saturating_sub(1);
            }
            (KeyModifiers::NONE, KeyCode::Backspace) => {
                self.file_open.query.pop();
                self.file_open.selected = 0;
                self.refresh_file_open_results();
            }
            (KeyModifiers::NONE, KeyCode::Char(c)) => {
                self.file_open.query.push(c);
                self.file_open.selected = 0;
                self.refresh_file_open_results();
            }
            _ => {}
        }
        Ok(false)
    }

    fn handle_key_global_search(&mut self, key: KeyEvent) -> Result<bool> {
        match (key.modifiers, key.code) {
            (KeyModifiers::NONE, KeyCode::Esc) => {
                self.mode = Mode::Normal;
            }
            (KeyModifiers::NONE, KeyCode::Enter) => {
                if let Some((path, line_no, _)) = self.global_search.selected_result() {
                    let path = path.clone();
                    let line_no = line_no;
                    self.open_file(path)?;
                    self.scroll_to_line(line_no);
                    self.mode = Mode::Normal;
                }
            }
            (KeyModifiers::NONE, KeyCode::Down) |
            (KeyModifiers::CONTROL, KeyCode::Char('j')) => {
                self.global_search.move_down();
            }
            (KeyModifiers::NONE, KeyCode::Up) |
            (KeyModifiers::CONTROL, KeyCode::Char('k')) => {
                self.global_search.move_up();
            }
            (KeyModifiers::NONE, KeyCode::Backspace) => {
                self.global_search.query.pop();
                self.global_search.mark_dirty();
            }
            (KeyModifiers::NONE, KeyCode::Char(c)) => {
                self.global_search.query.push(c);
                self.global_search.mark_dirty();
            }
            _ => {}
        }
        Ok(false)
    }

    pub fn handle_mouse(&mut self, mouse: MouseEvent) {
        if let MouseEventKind::ScrollDown = mouse.kind {
            if self.scroll_offset + self.viewport_height < self.content.len() {
                self.scroll_offset += 3;
            }
        }
        if let MouseEventKind::ScrollUp = mouse.kind {
            self.scroll_offset = self.scroll_offset.saturating_sub(3);
        }
    }

    fn scroll_to_line(&mut self, line: usize) {
        if line < self.scroll_offset {
            self.scroll_offset = line;
        } else if line >= self.scroll_offset + self.viewport_height {
            self.scroll_offset = line.saturating_sub(self.viewport_height / 2);
        }
    }

    fn refresh_file_open_results(&mut self) {
        let q = self.file_open.query.to_lowercase();
        let mut results = Vec::new();
        for entry in walkdir::WalkDir::new(&self.vault_path)
            .follow_links(true)
            .into_iter()
            .filter_map(|e| e.ok())
        {
            if entry.file_type().is_file() {
                let name = entry.file_name().to_string_lossy().to_lowercase();
                if q.is_empty() || name.contains(&q) {
                    results.push(entry.into_path());
                    if results.len() >= 50 {
                        break;
                    }
                }
            }
        }
        self.file_open.results = results;
    }
}
