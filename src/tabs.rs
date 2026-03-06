use anyhow::{Context, Result};
use ratatui_code_editor::editor::Editor;
use ratatui_code_editor::theme::vesper;
use std::path::PathBuf;

use crate::search::SearchState;

/// Represents a single open document or an empty buffer in the editor.
pub struct Tab {
    /// The path to the file on disk. If None, it's a new unsaved buffer.
    pub path: Option<PathBuf>,
    /// The ratatui-code-editor widget managing the text buffer and highlighting.
    pub editor: Editor,
    /// Whether the document has unsaved changes.
    pub dirty: bool,
    /// State for the in-file search functionality (Ctrl+F).
    pub in_file_search: SearchState,
}

impl Tab {
    /// Creates a new, empty tab named "[ new ]".
    pub fn empty() -> Self {
        Self {
            path: None,
            // Default to markdown syntax highlighting for new files
            editor: Editor::new("md", "", vesper()).unwrap(),
            dirty: false,
            in_file_search: SearchState::new(),
        }
    }

    /// Formats the tab title for display in the UI tab bar.
    pub fn title(&self) -> String {
        match &self.path {
            None => "[ new ]".to_string(),
            Some(p) => {
                let name = p.file_name().and_then(|n| n.to_str()).unwrap_or("?");
                // Append "[+]" if the file is modified
                if self.dirty {
                    format!("{} [+]", name)
                } else {
                    name.to_string()
                }
            }
        }
    }

    /// Loads a file from disk into a new Tab.
    pub fn load(path: PathBuf) -> Result<Self> {
        let text = std::fs::read_to_string(&path)
            .with_context(|| format!("Could not read {}", path.display()))?;

        // Determine language for syntax highlighting based on extension
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

        // Fallback to plain text if the language is unsupported
        let editor = Editor::new(lang, &text, vesper())
            .unwrap_or_else(|_| Editor::new("text", &text, vesper()).unwrap());

        Ok(Self {
            path: Some(path),
            editor,
            dirty: false,
            in_file_search: SearchState::new(),
        })
    }

    /// Saves the current editor content back to disk.
    pub fn save(&mut self) -> Result<()> {
        if let Some(ref path) = self.path {
            std::fs::write(path, self.editor.get_content())
                .with_context(|| format!("Could not save {}", path.display()))?;
            self.dirty = false;
        }
        Ok(())
    }

    /// Updates syntax highlighting marks based on search results.
    /// This is used to visually highlight search hits in the editor.
    pub fn update_search_marks(&mut self) {
        if self.in_file_search.matches.is_empty() {
            self.editor.remove_marks();
        } else {
            let mut marks = Vec::new();
            for (i, m) in self.in_file_search.matches.iter().enumerate() {
                // Highlight the currently selected match in light green, others in yellow
                let color = if i == self.in_file_search.current { "#00ff00" } else { "#ffff00" };
                marks.push((m.start_char, m.end_char, color));
            }
            self.editor.set_marks(marks);
        }
    }
}
