use crate::filetree::FileTree;
use crate::search::{SearchState, GlobalSearch};
use anyhow::{Context, Result};
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers, MouseEvent, MouseEventKind};
use std::path::PathBuf;

#[derive(Debug, Clone, PartialEq)]
pub enum Mode {
    Normal,
    Insert,
    SidePanel,
    InFileSearch,
    FileOpen,
    GlobalSearch,
}

#[derive(Debug, Clone, PartialEq)]
pub enum Pane {
    Editor,
    Sidebar,
}

// ── Per-tab state ─────────────────────────────────────────────────────────────

pub struct Tab {
    pub path: Option<PathBuf>,
    pub content: Vec<String>,
    pub cursor_line: usize,
    pub cursor_col: usize,
    pub scroll_offset: usize,
    pub dirty: bool,
    pub in_file_search: SearchState,
}

impl Tab {
    pub fn empty() -> Self {
        Self {
            path: None,
            content: Vec::new(),
            cursor_line: 0,
            cursor_col: 0,
            scroll_offset: 0,
            dirty: false,
            in_file_search: SearchState::new(),
        }
    }

    pub fn title(&self) -> String {
        match &self.path {
            None => "[ new ]".to_string(),
            Some(p) => {
                let name = p.file_name().and_then(|n| n.to_str()).unwrap_or("?");
                if self.dirty {
                    format!("{} [+]", name)
                } else {
                    name.to_string()
                }
            }
        }
    }

    pub fn load(path: PathBuf) -> Result<Self> {
        let text = std::fs::read_to_string(&path)
            .with_context(|| format!("Could not read {}", path.display()))?;
        let mut content: Vec<String> = text.lines().map(|l| l.to_string()).collect();
        if content.is_empty() {
            content.push(String::new());
        }
        Ok(Self {
            path: Some(path),
            content,
            cursor_line: 0,
            cursor_col: 0,
            scroll_offset: 0,
            dirty: false,
            in_file_search: SearchState::new(),
        })
    }

    pub fn save(&mut self) -> Result<()> {
        if let Some(ref path) = self.path {
            std::fs::write(path, self.content.join("\n"))
                .with_context(|| format!("Could not save {}", path.display()))?;
            self.dirty = false;
        }
        Ok(())
    }

    pub fn current_line_len(&self) -> usize {
        self.content.get(self.cursor_line).map(|l| l.chars().count()).unwrap_or(0)
    }

    pub fn scroll_to_cursor(&mut self, viewport_height: usize) {
        if self.cursor_line < self.scroll_offset {
            self.scroll_offset = self.cursor_line;
        } else if self.cursor_line >= self.scroll_offset + viewport_height {
            self.scroll_offset = self.cursor_line.saturating_sub(viewport_height / 2);
        }
    }
}

// ── App ───────────────────────────────────────────────────────────────────────

pub struct FileOpenState {
    pub query: String,
    pub all_files: Vec<PathBuf>,
    pub results: Vec<PathBuf>,
    pub selected: usize,
}

impl FileOpenState {
    pub fn new() -> Self {
        Self { query: String::new(), all_files: Vec::new(), results: Vec::new(), selected: 0 }
    }

    pub fn filter(&mut self) {
        let q = self.query.to_lowercase();
        self.results = if q.is_empty() {
            self.all_files.iter().take(50).cloned().collect()
        } else {
            self.all_files.iter()
                .filter(|p| p.file_name().and_then(|n| n.to_str())
                    .map(|n| n.to_lowercase().contains(&q)).unwrap_or(false))
                .take(50).cloned().collect()
        };
        self.selected = 0;
    }
}

pub struct App {
    pub vault_path: PathBuf,
    pub mode: Mode,
    pub active_pane: Pane,

    // Tabs
    pub tabs: Vec<Tab>,
    pub active_tab: usize,

    // Sidebar
    pub sidebar_visible: bool,
    pub file_tree: FileTree,

    // Shared editor state
    pub viewport_height: usize,

    // Dialogs
    pub file_open: FileOpenState,
    pub global_search: GlobalSearch,

    pub status_msg: String,
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
            tabs: vec![Tab::empty()],
            active_tab: 0,
            sidebar_visible: true,
            file_tree,
            viewport_height: 20,
            file_open: FileOpenState::new(),
            global_search: GlobalSearch::new(),
            status_msg: String::from("noted — Ctrl+T new tab  Ctrl+W close  Alt+←/→ switch"),
        })
    }

    // ── tab helpers ──────────────────────────────────────────────────────────

    pub fn tab(&self) -> &Tab { &self.tabs[self.active_tab] }
    pub fn tab_mut(&mut self) -> &mut Tab { &mut self.tabs[self.active_tab] }

    pub fn new_tab(&mut self) {
        self.tabs.insert(self.active_tab + 1, Tab::empty());
        self.active_tab += 1;
        self.status_msg = "New tab — Ctrl+P to open a file".to_string();
    }

    pub fn close_tab(&mut self) {
        if self.tabs.len() == 1 {
            // Last tab: just blank it out instead of closing
            self.tabs[0] = Tab::empty();
            self.status_msg = "noted — Ctrl+P to open a file".to_string();
            return;
        }
        self.tabs.remove(self.active_tab);
        if self.active_tab >= self.tabs.len() {
            self.active_tab = self.tabs.len() - 1;
        }
        self.update_status();
    }

    pub fn next_tab(&mut self) {
        self.active_tab = (self.active_tab + 1) % self.tabs.len();
        self.update_status();
    }

    pub fn prev_tab(&mut self) {
        if self.active_tab == 0 {
            self.active_tab = self.tabs.len() - 1;
        } else {
            self.active_tab -= 1;
        }
        self.update_status();
    }

    pub fn goto_tab(&mut self, n: usize) {
        if n < self.tabs.len() {
            self.active_tab = n;
            self.update_status();
        }
    }

    fn update_status(&mut self) {
        let tab = &self.tabs[self.active_tab];
        match &tab.path {
            None => self.status_msg = "[ new tab ] — Ctrl+P to open".to_string(),
            Some(p) => {
                let name = p.file_name().and_then(|n| n.to_str()).unwrap_or("?");
                self.status_msg = format!("{} — e=edit  Ctrl+S=save", name);
            }
        }
    }

    // ── open / save ──────────────────────────────────────────────────────────

    /// Open path in the current tab (or a new tab if current tab has content)
    pub fn open_file(&mut self, path: PathBuf) -> Result<()> {
        let ext = path.extension().and_then(|e| e.to_str()).unwrap_or("").to_lowercase();
        if !matches!(ext.as_str(), "md" | "txt" | "sh" | "") {
            self.status_msg = format!("Opening {} in default app…", path.display());
            let _ = open::that(&path);
            return Ok(());
        }

        // If already open in a tab, switch to it
        if let Some(idx) = self.tabs.iter().position(|t| t.path.as_deref() == Some(&path)) {
            self.active_tab = idx;
            self.update_status();
            return Ok(());
        }

        // If current tab is blank/empty, reuse it; otherwise open in new tab
        let reuse = self.tabs[self.active_tab].path.is_none()
            && self.tabs[self.active_tab].content.is_empty();

        let new_tab = Tab::load(path.clone())?;
        if reuse {
            self.tabs[self.active_tab] = new_tab;
        } else {
            self.tabs.insert(self.active_tab + 1, new_tab);
            self.active_tab += 1;
        }
        self.update_status();
        Ok(())
    }

    pub fn save_file(&mut self) -> Result<()> {
        self.tabs[self.active_tab].save()?;
        self.update_status();
        let name = self.tabs[self.active_tab].path.as_ref()
            .and_then(|p| p.file_name()).and_then(|n| n.to_str()).unwrap_or("?");
        self.status_msg = format!("{} — saved", name);
        Ok(())
    }

    // ── key dispatch ─────────────────────────────────────────────────────────

    pub fn handle_key(&mut self, key: KeyEvent) -> Result<bool> {
        match &self.mode {
            Mode::Insert        => return self.handle_key_insert(key),
            Mode::InFileSearch  => return self.handle_key_in_file_search(key),
            Mode::FileOpen      => return self.handle_key_file_open(key),
            Mode::GlobalSearch  => return self.handle_key_global_search(key),
            Mode::SidePanel     => return self.handle_key_sidebar(key),
            Mode::Normal        => {}
        }

        match (key.modifiers, key.code) {
            (KeyModifiers::CONTROL, KeyCode::Char('q')) => return Ok(true),

            // Tab management
            (KeyModifiers::CONTROL, KeyCode::Char('t'))  => { self.new_tab(); }
            (KeyModifiers::CONTROL, KeyCode::Char('w'))  => { self.close_tab(); }

            // Switch tabs: Alt+Left / Alt+Right  or  Ctrl+Left / Ctrl+Right
            (KeyModifiers::ALT,     KeyCode::Right) |
            (KeyModifiers::ALT,     KeyCode::Char('l')) => { self.next_tab(); }
            (KeyModifiers::ALT,     KeyCode::Left)  |
            (KeyModifiers::ALT,     KeyCode::Char('h')) => { self.prev_tab(); }

            // Alt+1..9 jump to tab
            (KeyModifiers::ALT, KeyCode::Char(c)) if c.is_ascii_digit() => {
                let n = c as usize - '1' as usize;
                self.goto_tab(n);
            }

            // Sidebar
            (KeyModifiers::CONTROL, KeyCode::Char('b')) => {
                self.sidebar_visible = !self.sidebar_visible;
            }
            (KeyModifiers::CONTROL, KeyCode::Char('e')) => {
                if self.sidebar_visible {
                    self.mode = Mode::SidePanel;
                    self.active_pane = Pane::Sidebar;
                }
            }

            // Edit
            (KeyModifiers::NONE, KeyCode::Char('e')) |
            (KeyModifiers::NONE, KeyCode::Char('i')) => {
                if self.tab().path.is_some() {
                    self.mode = Mode::Insert;
                    self.status_msg = "-- INSERT -- Esc=normal  Ctrl+S=save".to_string();
                }
            }

            // Save
            (KeyModifiers::CONTROL, KeyCode::Char('s')) => { self.save_file()?; }

            // Search
            (KeyModifiers::CONTROL, KeyCode::Char('f')) => {
                self.mode = Mode::InFileSearch;
                self.tab_mut().in_file_search = SearchState::new();
            }
            (KeyModifiers::CONTROL, KeyCode::Char('p')) => {
                self.mode = Mode::FileOpen;
                let mut state = FileOpenState::new();
                state.all_files = collect_all_files(&self.vault_path);
                state.filter();
                self.file_open = state;
            }
            (KeyModifiers::CONTROL, KeyCode::Char('g')) => {
                self.mode = Mode::GlobalSearch;
                self.global_search = GlobalSearch::new();
            }

            // Scroll / movement
            (KeyModifiers::NONE, KeyCode::Down) |
            (KeyModifiers::NONE, KeyCode::Char('j')) => { self.scroll_down(1); }
            (KeyModifiers::NONE, KeyCode::Up) |
            (KeyModifiers::NONE, KeyCode::Char('k')) => { self.scroll_up(1); }
            (KeyModifiers::NONE, KeyCode::PageDown) => {
                self.scroll_down(self.viewport_height.saturating_sub(2));
            }
            (KeyModifiers::NONE, KeyCode::PageUp) => {
                self.scroll_up(self.viewport_height.saturating_sub(2));
            }
            (KeyModifiers::NONE, KeyCode::Home) |
            (KeyModifiers::NONE, KeyCode::Char('g')) => {
                let tab = self.tab_mut();
                tab.scroll_offset = 0;
                tab.cursor_line = 0;
            }
            (KeyModifiers::NONE, KeyCode::End) => {
                let len = self.tab().content.len().saturating_sub(1);
                let vh = self.viewport_height;
                let tab = self.tab_mut();
                tab.cursor_line = len;
                tab.scroll_to_cursor(vh);
            }
            (KeyModifiers::SHIFT, KeyCode::Char('G')) => {
                let len = self.tab().content.len().saturating_sub(1);
                let vh = self.viewport_height;
                let tab = self.tab_mut();
                tab.cursor_line = len;
                tab.scroll_to_cursor(vh);
            }
            (KeyModifiers::NONE, KeyCode::Char('n')) => {
                let vh = self.viewport_height;
                let tab = self.tab_mut();
                tab.in_file_search.next_match(&tab.content);
                if let Some(line) = tab.in_file_search.current_match_line() {
                    tab.cursor_line = line;
                    tab.scroll_to_cursor(vh);
                }
            }
            (KeyModifiers::SHIFT, KeyCode::Char('N')) => {
                let vh = self.viewport_height;
                let tab = self.tab_mut();
                tab.in_file_search.prev_match(&tab.content);
                if let Some(line) = tab.in_file_search.current_match_line() {
                    tab.cursor_line = line;
                    tab.scroll_to_cursor(vh);
                }
            }

            _ => {}
        }
        Ok(false)
    }

    fn handle_key_insert(&mut self, key: KeyEvent) -> Result<bool> {
        match (key.modifiers, key.code) {
            (KeyModifiers::NONE, KeyCode::Esc) => {
                self.mode = Mode::Normal;
                self.update_status();
            }
            (KeyModifiers::CONTROL, KeyCode::Char('s')) => { self.save_file()?; }

            (KeyModifiers::NONE, KeyCode::Enter) => {
                let vh = self.viewport_height;
                let tab = self.tab_mut();
                let col = tab.cursor_col.min(tab.current_line_len());
                let rest = tab.content[tab.cursor_line][char_to_byte(&tab.content[tab.cursor_line], col)..].to_string();
                let line = tab.cursor_line;
                let truncate_at = char_to_byte(&tab.content[line], col);
                tab.content[line].truncate(truncate_at);
                tab.cursor_line += 1;
                tab.content.insert(tab.cursor_line, rest);
                tab.cursor_col = 0;
                tab.dirty = true;
                tab.scroll_to_cursor(vh);
            }
            (KeyModifiers::NONE, KeyCode::Backspace) => {
                let vh = self.viewport_height;
                let tab = self.tab_mut();
                if tab.cursor_col > 0 {
                    let col = tab.cursor_col.min(tab.current_line_len());
                    let prev = char_to_byte(&tab.content[tab.cursor_line], col - 1);
                    let cur  = char_to_byte(&tab.content[tab.cursor_line], col);
                    tab.content[tab.cursor_line].drain(prev..cur);
                    tab.cursor_col -= 1;
                    tab.dirty = true;
                } else if tab.cursor_line > 0 {
                    let cur_line = tab.content.remove(tab.cursor_line);
                    tab.cursor_line -= 1;
                    let prev_len = tab.content[tab.cursor_line].chars().count();
                    tab.content[tab.cursor_line].push_str(&cur_line);
                    tab.cursor_col = prev_len;
                    tab.dirty = true;
                    tab.scroll_to_cursor(vh);
                }
            }
            (KeyModifiers::NONE, KeyCode::Delete) => {
                let tab = self.tab_mut();
                let col = tab.cursor_col.min(tab.current_line_len());
                let line_len = tab.current_line_len();
                if col < line_len {
                    let b0 = char_to_byte(&tab.content[tab.cursor_line], col);
                    let b1 = char_to_byte(&tab.content[tab.cursor_line], col + 1);
                    tab.content[tab.cursor_line].drain(b0..b1);
                    tab.dirty = true;
                } else if tab.cursor_line + 1 < tab.content.len() {
                    let next = tab.content.remove(tab.cursor_line + 1);
                    tab.content[tab.cursor_line].push_str(&next);
                    tab.dirty = true;
                }
            }
            (KeyModifiers::NONE, KeyCode::Tab) => {
                let tab = self.tab_mut();
                let col = tab.cursor_col.min(tab.current_line_len());
                let bp = char_to_byte(&tab.content[tab.cursor_line], col);
                tab.content[tab.cursor_line].insert_str(bp, "  ");
                tab.cursor_col += 2;
                tab.dirty = true;
            }
            (KeyModifiers::NONE, KeyCode::Left) => {
                let vh = self.viewport_height;
                let tab = self.tab_mut();
                if tab.cursor_col > 0 { tab.cursor_col -= 1; }
                else if tab.cursor_line > 0 {
                    tab.cursor_line -= 1;
                    tab.cursor_col = tab.current_line_len();
                    tab.scroll_to_cursor(vh);
                }
            }
            (KeyModifiers::NONE, KeyCode::Right) => {
                let vh = self.viewport_height;
                let tab = self.tab_mut();
                let ll = tab.current_line_len();
                if tab.cursor_col < ll { tab.cursor_col += 1; }
                else if tab.cursor_line + 1 < tab.content.len() {
                    tab.cursor_line += 1;
                    tab.cursor_col = 0;
                    tab.scroll_to_cursor(vh);
                }
            }
            (KeyModifiers::NONE, KeyCode::Up) => {
                let vh = self.viewport_height;
                let tab = self.tab_mut();
                if tab.cursor_line > 0 {
                    tab.cursor_line -= 1;
                    tab.cursor_col = tab.cursor_col.min(tab.current_line_len());
                    tab.scroll_to_cursor(vh);
                }
            }
            (KeyModifiers::NONE, KeyCode::Down) => {
                let vh = self.viewport_height;
                let tab = self.tab_mut();
                if tab.cursor_line + 1 < tab.content.len() {
                    tab.cursor_line += 1;
                    tab.cursor_col = tab.cursor_col.min(tab.current_line_len());
                    tab.scroll_to_cursor(vh);
                }
            }
            (KeyModifiers::NONE, KeyCode::Home) => { self.tab_mut().cursor_col = 0; }
            (KeyModifiers::NONE, KeyCode::End) => {
                let ll = self.tab().current_line_len();
                self.tab_mut().cursor_col = ll;
            }
            (mods, KeyCode::Char(c)) if mods == KeyModifiers::NONE || mods == KeyModifiers::SHIFT => {
                let tab = self.tab_mut();
                let col = tab.cursor_col.min(tab.current_line_len());
                let bp = char_to_byte(&tab.content[tab.cursor_line], col);
                tab.content[tab.cursor_line].insert(bp, c);
                tab.cursor_col += 1;
                tab.dirty = true;
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
            (KeyModifiers::NONE, KeyCode::Down) | (KeyModifiers::NONE, KeyCode::Char('j')) => {
                self.file_tree.move_down();
            }
            (KeyModifiers::NONE, KeyCode::Up) | (KeyModifiers::NONE, KeyCode::Char('k')) => {
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
            (KeyModifiers::NONE, KeyCode::Char('h')) | (KeyModifiers::NONE, KeyCode::Left) => {
                self.file_tree.collapse_or_parent();
            }
            (KeyModifiers::NONE, KeyCode::Char(' ')) => { self.file_tree.toggle_expand(); }
            _ => {}
        }
        Ok(false)
    }

    fn handle_key_in_file_search(&mut self, key: KeyEvent) -> Result<bool> {
        match (key.modifiers, key.code) {
            (KeyModifiers::NONE, KeyCode::Esc) => { self.mode = Mode::Normal; }
            (KeyModifiers::NONE, KeyCode::Enter) => {
                let vh = self.viewport_height;
                let tab = self.tab_mut();
                tab.in_file_search.next_match(&tab.content);
                if let Some(line) = tab.in_file_search.current_match_line() {
                    tab.cursor_line = line;
                    tab.scroll_to_cursor(vh);
                }
                self.mode = Mode::Normal;
            }
            (KeyModifiers::NONE, KeyCode::Backspace) => {
                let tab = self.tab_mut();
                tab.in_file_search.query.pop();
                let content = tab.content.clone();
                tab.in_file_search.recompute_matches(&content);
            }
            (KeyModifiers::NONE, KeyCode::Char(c)) => {
                let vh = self.viewport_height;
                let tab = self.tab_mut();
                tab.in_file_search.query.push(c);
                let content = tab.content.clone();
                tab.in_file_search.recompute_matches(&content);
                if let Some(line) = tab.in_file_search.current_match_line() {
                    tab.cursor_line = line;
                    tab.scroll_to_cursor(vh);
                }
            }
            _ => {}
        }
        Ok(false)
    }

    fn handle_key_file_open(&mut self, key: KeyEvent) -> Result<bool> {
        match (key.modifiers, key.code) {
            (KeyModifiers::NONE, KeyCode::Esc) => { self.mode = Mode::Normal; }
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
                self.file_open.filter();
            }
            (KeyModifiers::NONE, KeyCode::Char(c)) => {
                self.file_open.query.push(c);
                self.file_open.filter();
            }
            _ => {}
        }
        Ok(false)
    }

    fn handle_key_global_search(&mut self, key: KeyEvent) -> Result<bool> {
        match (key.modifiers, key.code) {
            (KeyModifiers::NONE, KeyCode::Esc) => { self.mode = Mode::Normal; }
            (KeyModifiers::NONE, KeyCode::Enter) => {
                if let Some((path, line_no, _)) = self.global_search.selected_result() {
                    let path = path.clone();
                    let line_no = line_no;
                    self.open_file(path)?;
                    let vh = self.viewport_height;
                    let tab = self.tab_mut();
                    tab.cursor_line = line_no;
                    tab.scroll_to_cursor(vh);
                    self.mode = Mode::Normal;
                }
            }
            (KeyModifiers::NONE, KeyCode::Down) |
            (KeyModifiers::CONTROL, KeyCode::Char('j')) => { self.global_search.move_down(); }
            (KeyModifiers::NONE, KeyCode::Up) |
            (KeyModifiers::CONTROL, KeyCode::Char('k')) => { self.global_search.move_up(); }
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
        match mouse.kind {
            MouseEventKind::ScrollDown => self.scroll_down(3),
            MouseEventKind::ScrollUp   => self.scroll_up(3),
            _ => {}
        }
    }

    fn scroll_down(&mut self, n: usize) {
        let tab = self.tab_mut();
        let max = tab.content.len().saturating_sub(1);
        tab.scroll_offset = (tab.scroll_offset + n).min(max);
        if tab.cursor_line < tab.scroll_offset {
            tab.cursor_line = tab.scroll_offset;
        }
    }

    fn scroll_up(&mut self, n: usize) {
        let vh = self.viewport_height;
        let tab = self.tab_mut();
        tab.scroll_offset = tab.scroll_offset.saturating_sub(n);
        let bottom = tab.scroll_offset + vh.saturating_sub(1);
        if tab.cursor_line > bottom {
            tab.cursor_line = bottom.min(tab.content.len().saturating_sub(1));
        }
    }
}

pub fn char_to_byte(s: &str, char_idx: usize) -> usize {
    s.char_indices().nth(char_idx).map(|(b, _)| b).unwrap_or(s.len())
}

fn collect_all_files(root: &std::path::Path) -> Vec<PathBuf> {
    walkdir::WalkDir::new(root)
        .follow_links(true)
        .into_iter()
        .filter_map(|e| e.ok())
        .filter(|e| e.file_type().is_file())
        .filter(|e| e.file_name().to_str().map(|n| !n.starts_with('.')).unwrap_or(false))
        .map(|e| e.into_path())
        .collect()
}
