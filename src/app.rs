use crate::filetree::FileTree;
use crate::search::GlobalSearch;
use crate::tabs::Tab;
use anyhow::{Context, Result};
use ratatui::layout::Rect;
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

    pub fn update_status(&mut self) {
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

}

pub fn collect_all_files(root: &std::path::Path) -> Vec<PathBuf> {
    ignore::WalkBuilder::new(root)
        .build()
        .filter_map(|e| e.ok())
        .filter(|e| e.file_type().map_or(false, |ft| ft.is_file()))
        .map(|e| e.into_path())
        .collect()
}
