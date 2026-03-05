use anyhow::Result;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone)]
pub struct FileNode {
    pub path: PathBuf,
    pub name: String,
    pub is_dir: bool,
    pub depth: usize,
    pub expanded: bool,
}

pub struct FileTree {
    pub root: PathBuf,
    pub flat: Vec<FileNode>,   // flattened visible list
    pub selected: usize,
}

impl FileTree {
    pub fn new(root: &Path) -> Result<Self> {
        let mut tree = Self {
            root: root.to_path_buf(),
            flat: Vec::new(),
            selected: 0,
        };
        tree.rebuild();
        Ok(tree)
    }

    pub fn rebuild(&mut self) {
        self.flat.clear();
        self.build_from(self.root.clone(), 0);
    }

    fn build_from(&mut self, dir: PathBuf, depth: usize) {
        let mut entries: Vec<_> = match std::fs::read_dir(&dir) {
            Ok(rd) => rd.filter_map(|e| e.ok()).collect(),
            Err(_) => return,
        };

        // Dirs first, then files, alphabetical
        entries.sort_by(|a, b| {
            let a_is_dir = a.file_type().map(|t| t.is_dir()).unwrap_or(false);
            let b_is_dir = b.file_type().map(|t| t.is_dir()).unwrap_or(false);
            match (a_is_dir, b_is_dir) {
                (true, false) => std::cmp::Ordering::Less,
                (false, true) => std::cmp::Ordering::Greater,
                _ => a.file_name().cmp(&b.file_name()),
            }
        });

        for entry in entries {
            let path = entry.path();
            let name = entry.file_name().to_string_lossy().to_string();

            // Skip hidden files/dirs
            if name.starts_with('.') {
                continue;
            }

            let is_dir = entry.file_type().map(|t| t.is_dir()).unwrap_or(false);

            // Check if this node was previously expanded
            let expanded = if is_dir {
                self.flat
                    .iter()
                    .find(|n| n.path == path)
                    .map(|n| n.expanded)
                    .unwrap_or(false)
            } else {
                false
            };

            let node = FileNode {
                path: path.clone(),
                name,
                is_dir,
                depth,
                expanded,
            };

            let should_expand = is_dir && expanded;
            self.flat.push(node);

            if should_expand {
                self.build_from(path, depth + 1);
            }
        }
    }

    pub fn selected_path(&self) -> Option<&PathBuf> {
        self.flat.get(self.selected).map(|n| &n.path)
    }

    pub fn move_down(&mut self) {
        if self.selected + 1 < self.flat.len() {
            self.selected += 1;
        }
    }

    pub fn move_up(&mut self) {
        self.selected = self.selected.saturating_sub(1);
    }

    pub fn toggle_expand(&mut self) {
        if let Some(node) = self.flat.get_mut(self.selected) {
            if node.is_dir {
                node.expanded = !node.expanded;
                let path = node.path.clone();
                let depth = node.depth;
                let expanded = node.expanded;

                // Remove children if collapsing
                if !expanded {
                    let start = self.selected + 1;
                    let mut end = start;
                    while end < self.flat.len() && self.flat[end].depth > depth {
                        end += 1;
                    }
                    self.flat.drain(start..end);
                } else {
                    // Insert children after current
                    let insert_pos = self.selected + 1;
                    let mut new_nodes = Vec::new();
                    Self::collect_children(&path, depth + 1, &mut new_nodes);
                    for (i, node) in new_nodes.into_iter().enumerate() {
                        self.flat.insert(insert_pos + i, node);
                    }
                }
            }
        }
    }

    fn collect_children(dir: &Path, depth: usize, out: &mut Vec<FileNode>) {
        let mut entries: Vec<_> = match std::fs::read_dir(dir) {
            Ok(rd) => rd.filter_map(|e| e.ok()).collect(),
            Err(_) => return,
        };

        entries.sort_by(|a, b| {
            let a_is_dir = a.file_type().map(|t| t.is_dir()).unwrap_or(false);
            let b_is_dir = b.file_type().map(|t| t.is_dir()).unwrap_or(false);
            match (a_is_dir, b_is_dir) {
                (true, false) => std::cmp::Ordering::Less,
                (false, true) => std::cmp::Ordering::Greater,
                _ => a.file_name().cmp(&b.file_name()),
            }
        });

        for entry in entries {
            let name = entry.file_name().to_string_lossy().to_string();
            if name.starts_with('.') {
                continue;
            }
            let path = entry.path();
            let is_dir = entry.file_type().map(|t| t.is_dir()).unwrap_or(false);
            out.push(FileNode { path: path.clone(), name, is_dir, depth, expanded: false });
        }
    }

    pub fn collapse_or_parent(&mut self) {
        if let Some(node) = self.flat.get(self.selected) {
            let depth = node.depth;
            let is_expanded_dir = node.is_dir && node.expanded;

            if is_expanded_dir {
                self.toggle_expand(); // collapse
            } else if depth > 0 {
                // Move to parent
                let current = self.selected;
                for i in (0..current).rev() {
                    if self.flat[i].depth < depth {
                        self.selected = i;
                        break;
                    }
                }
            }
        }
    }
}
