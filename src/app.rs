use crate::filetree::FileTree;
use crate::search::{SearchState, GlobalSearch};
use anyhow::{Context, Result};
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers, MouseEvent};
use std::path::PathBuf;
use ratatui_code_editor::editor::Editor;
use ratatui_code_editor::theme::vesper;
use ratatui::layout::Rect;

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
    pub editor: Editor,
    pub dirty: bool,
    pub in_file_search: SearchState,
}

impl Tab {
    pub fn empty() -> Self {
        Self {
            path: None,
            editor: Editor::new("md", "", vesper()).unwrap(),
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

        let ext = path.extension().and_then(|e| e.to_str()).unwrap_or("txt");
        let lang = match ext {
            "rs" => "rust",
            "md" => "markdown",
            "js" => "javascript",
            "ts" => "typescript",
            "py" => "python",
            "yaml" | "yml" => "yaml",
            "toml" => "toml",
            "json" => "json",
            "html" => "html",
            "css" => "css",
            "sh" => "bash",
            _ => "text",
        };

        let editor = Editor::new(lang, &text, vesper()).unwrap_or_else(|_| Editor::new("text", &text, vesper()).unwrap());
        Ok(Self {
            path: Some(path),
            editor,
            dirty: false,
            in_file_search: SearchState::new(),
        })
    }

    pub fn save(&mut self) -> Result<()> {
        if let Some(ref path) = self.path {
            std::fs::write(path, self.editor.get_content())
                .with_context(|| format!("Could not save {}", path.display()))?;
            self.dirty = false;
        }
        Ok(())
    }

    pub fn update_search_marks(&mut self) {
        if self.in_file_search.matches.is_empty() {
            self.editor.remove_marks();
        } else {
            let mut marks = Vec::new();
            for (i, m) in self.in_file_search.matches.iter().enumerate() {
                let color = if i == self.in_file_search.current { "#00ff00" } else { "#ffff00" };
                marks.push((m.start_char, m.end_char, color));
            }
            self.editor.set_marks(marks);
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
    pub editor_area: Rect,

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
            editor_area: Rect::default(),
            file_open: FileOpenState::new(),
            global_search: GlobalSearch::new(),
            status_msg: String::from("Slate — Ctrl+T new tab  Ctrl+W close  Alt+←/→ switch"),
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
            self.status_msg = "Slate — Ctrl+P to open a file".to_string();
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
            && self.tabs[self.active_tab].editor.get_content().is_empty();

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
        let area = self.editor_area;
        match &self.mode {
            Mode::InFileSearch  => return self.handle_key_in_file_search(key),
            Mode::FileOpen      => return self.handle_key_file_open(key),
            Mode::GlobalSearch  => return self.handle_key_global_search(key),
            Mode::SidePanel     => return self.handle_key_sidebar(key),
            Mode::Insert => {
                if key.code == KeyCode::Esc {
                    self.mode = Mode::Normal;
                    self.update_status();
                    return Ok(false);
                }
                if key.modifiers == KeyModifiers::CONTROL && key.code == KeyCode::Char('s') {
                    self.save_file()?;
                    return Ok(false);
                }
                self.tab_mut().editor.input(key, &area)?;
                self.tab_mut().dirty = true;
                return Ok(false);
            }
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
            (KeyModifiers::NONE, KeyCode::Char('i')) |
            (KeyModifiers::NONE, KeyCode::Enter) => {
                if self.tab().path.is_some() || true {
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
                self.tab_mut().editor.remove_marks();
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

            // Scroll / movement bindings in normal mode (pass through to editor)
            (KeyModifiers::NONE, KeyCode::Down) |
            (KeyModifiers::NONE, KeyCode::Char('j')) => {
                let _ = self.tab_mut().editor.input(KeyEvent::new(KeyCode::Down, KeyModifiers::NONE), &area);
            }
            (KeyModifiers::NONE, KeyCode::Up) |
            (KeyModifiers::NONE, KeyCode::Char('k')) => {
                let _ = self.tab_mut().editor.input(KeyEvent::new(KeyCode::Up, KeyModifiers::NONE), &area);
            }
            (KeyModifiers::NONE, KeyCode::Left) |
            (KeyModifiers::NONE, KeyCode::Char('h')) => {
                let _ = self.tab_mut().editor.input(KeyEvent::new(KeyCode::Left, KeyModifiers::NONE), &area);
            }
            (KeyModifiers::NONE, KeyCode::Right) |
            (KeyModifiers::NONE, KeyCode::Char('l')) => {
                let _ = self.tab_mut().editor.input(KeyEvent::new(KeyCode::Right, KeyModifiers::NONE), &area);
            }
            (KeyModifiers::NONE, KeyCode::PageDown) => {
                let area_height = self.editor_area.height as usize;
                self.tab_mut().editor.scroll_down(area_height);
            }
            (KeyModifiers::NONE, KeyCode::PageUp) => {
                self.tab_mut().editor.scroll_up();
            }
            (KeyModifiers::NONE, KeyCode::Home) |
            (KeyModifiers::NONE, KeyCode::Char('0')) => {
                // To start of line could be implemented by fetching line bounds, but we can leave this stubbed or implement via editor.input
                let _ = self.tab_mut().editor.input(KeyEvent::new(KeyCode::Home, KeyModifiers::NONE), &area);
            }
            (KeyModifiers::NONE, KeyCode::End) |
            (KeyModifiers::SHIFT, KeyCode::Char('$')) => {
                let _ = self.tab_mut().editor.input(KeyEvent::new(KeyCode::End, KeyModifiers::NONE), &area);
            }
            (KeyModifiers::SHIFT, KeyCode::Char('G')) => {
                let tab = self.tab_mut();
                let len = tab.editor.get_content().chars().count();
                tab.editor.set_cursor(len);
            }
            (KeyModifiers::NONE, KeyCode::Char('n')) => {
                let tab = self.tab_mut();
                let content = tab.editor.get_content();
                tab.in_file_search.next_match(&content);
                if let Some(m) = tab.in_file_search.current_match() {
                    tab.editor.set_cursor(m.start_char);
                }
                tab.update_search_marks();
            }
            (KeyModifiers::SHIFT, KeyCode::Char('N')) => {
                let tab = self.tab_mut();
                let content = tab.editor.get_content();
                tab.in_file_search.prev_match(&content);
                if let Some(m) = tab.in_file_search.current_match() {
                    tab.editor.set_cursor(m.start_char);
                }
                tab.update_search_marks();
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
            (KeyModifiers::NONE, KeyCode::Esc) => {
                self.mode = Mode::Normal;
                self.tab_mut().editor.remove_marks();
            }
            (KeyModifiers::NONE, KeyCode::Enter) => {
                let tab = self.tab_mut();
                let content = tab.editor.get_content();
                tab.in_file_search.next_match(&content);
                if let Some(m) = tab.in_file_search.current_match() {
                    tab.editor.set_cursor(m.start_char);
                }
                tab.update_search_marks();
                self.mode = Mode::Normal;
            }
            (KeyModifiers::NONE, KeyCode::Backspace) => {
                let tab = self.tab_mut();
                tab.in_file_search.query.pop();
                let content = tab.editor.get_content();
                tab.in_file_search.recompute_matches(&content);
                tab.update_search_marks();
            }
            (KeyModifiers::NONE, KeyCode::Char(c)) => {
                let tab = self.tab_mut();
                tab.in_file_search.query.push(c);
                let content = tab.editor.get_content();
                tab.in_file_search.recompute_matches(&content);
                if let Some(m) = tab.in_file_search.current_match() {
                    tab.editor.set_cursor(m.start_char);
                }
                tab.update_search_marks();
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

                    let tab = self.tab_mut();
                    let content = tab.editor.get_content();

                    // Simple heuristic: count chars to line
                    let mut char_count = 0;
                    for (i, line) in content.lines().enumerate() {
                        if i == line_no {
                            break;
                        }
                        char_count += line.chars().count() + 1; // +1 for newline
                    }
                    tab.editor.set_cursor(char_count);

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
        let area = self.editor_area;
        if area.contains(ratatui::layout::Position::new(mouse.column, mouse.row)) {
            let _ = self.tab_mut().editor.mouse(mouse, &area);
        }
    }
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
