use anyhow::Result;
use ignore::WalkBuilder;
use std::path::{Path, PathBuf};

/// Represents a single file or directory node in our visible file tree.
#[derive(Debug, Clone)]
pub struct FileNode {
    /// The absolute path to this file or directory.
    pub path: PathBuf,
    /// The display name (usually just the filename).
    pub name: String,
    /// Whether this node represents a directory.
    pub is_dir: bool,
    /// The indentation level (how deep it is in the tree hierarchy).
    pub depth: usize,
    /// If it is a directory, whether its contents are currently visible.
    pub expanded: bool,
}

/// The state of the sidebar file explorer.
pub struct FileTree {
    /// The root directory of our vault/project.
    pub root: PathBuf,
    /// A flattened list of nodes currently visible in the UI. 
    /// This is what gets drawn line-by-line.
    pub flat: Vec<FileNode>,   
    /// The index of the currently selected row in `flat`.
    pub selected: usize,
}

impl FileTree {
    /// Initializes a new file tree bounded to a specific root path.
    pub fn new(root: &Path) -> Result<Self> {
        let mut tree = Self {
            root: root.to_path_buf(),
            flat: Vec::new(),
            // Start with the first item selected.
            selected: 0,
        };
        // Populate the tree initially.
        tree.rebuild();
        Ok(tree)
    }

    /// Completely rebuilds the `flat` list from the root directory.
    /// This is useful to refresh the sidebar when files have changed.
    pub fn rebuild(&mut self) {
        self.flat.clear();
        self.build_from(self.root.clone(), 0);
    }

    /// Reads a directory and appends its visible children to `self.flat`.
    /// 
    /// If `ignore` filter respects `.gitignore` and ignores hidden files.
    /// We use a shallow depth `max_depth(Some(1))` to only get direct children.
    fn build_from(&mut self, dir: PathBuf, depth: usize) {
        let mut entries: Vec<_> = WalkBuilder::new(&dir)
            // Limit to direct children (depth 1) so we only expand what's necessary
            .max_depth(Some(1))
            .hidden(true)     // hide dotfiles
            .git_ignore(true) // respect .gitignore
            .build()
            .filter_map(|e| e.ok())
            // WalkBuilder yields the root dir itself at depth 0, we skip it
            .filter(|e| e.depth() > 0)
            .collect();

        // Sort directories first, then files alphabetically.
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
            let path = entry.path().to_path_buf();
            let name = entry.file_name().to_string_lossy().to_string();
            let is_dir = entry.file_type().map(|t| t.is_dir()).unwrap_or(false);

            // Determine if the node was expanded before we rebuilt.
            // If yes, we want to retain that state.
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

            // If the folder was expanded, recursively build its children
            if should_expand {
                self.build_from(path, depth + 1);
            }
        }
    }

    /// Returns the absolute path of the currently highlighted file or directory.
    pub fn selected_path(&self) -> Option<&PathBuf> {
        self.flat.get(self.selected).map(|n| &n.path)
    }

    /// Moves the selection cursor down by one, bounding at the last item.
    pub fn move_down(&mut self) {
        if self.selected + 1 < self.flat.len() {
            self.selected += 1;
        }
    }

    /// Moves the selection cursor up by one, stopping at the first item (index 0).
    pub fn move_up(&mut self) {
        self.selected = self.selected.saturating_sub(1);
    }

    /// Toggles the expanded/collapsed state of a directory.
    pub fn toggle_expand(&mut self) {
        // We use an index based lookup to get a mutable reference to the selected node
        if let Some(node) = self.flat.get_mut(self.selected) {
            if node.is_dir {
                node.expanded = !node.expanded;
                let path = node.path.clone();
                let depth = node.depth;
                let expanded = node.expanded;

                if !expanded {
                    // **Collapsing**: remove all children from `flat`.
                    // We step forward through the list and delete anything deeper than
                    // the current directory until we hit a sibling or parent folder.
                    let start = self.selected + 1;
                    let mut end = start;
                    while end < self.flat.len() && self.flat[end].depth > depth {
                        end += 1;
                    }
                    self.flat.drain(start..end);
                } else {
                    // **Expanding**: fetch direct children and insert them right after our node.
                    let insert_pos = self.selected + 1;
                    let mut new_nodes = Vec::new();
                    Self::collect_children(&path, depth + 1, &mut new_nodes);
                    for (i, child_node) in new_nodes.into_iter().enumerate() {
                        self.flat.insert(insert_pos + i, child_node);
                    }
                }
            }
        }
    }

    /// Gathers the direct children of a given directory, appending them to `out`.
    /// Similar to `build_from` but without recursion; used specifically when a user
    /// opens a directory by hitting enter/space.
    fn collect_children(dir: &Path, depth: usize, out: &mut Vec<FileNode>) {
        let mut entries: Vec<_> = WalkBuilder::new(dir)
            .max_depth(Some(1))
            .hidden(true)
            .git_ignore(true)
            .build()
            .filter_map(|e| e.ok())
            .filter(|e| e.depth() > 0)
            .collect();

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
            let path = entry.path().to_path_buf();
            let is_dir = entry.file_type().map(|t| t.is_dir()).unwrap_or(false);
            
            out.push(FileNode { 
                path, 
                name, 
                is_dir, 
                depth, 
                // newly loaded children are collapsed by default
                expanded: false 
            });
        }
    }

    /// Closes the current directory if it is open, otherwise jumps selection
    /// to the parent directory. (Usually mapped to the LEFT arrow key or 'h').
    pub fn collapse_or_parent(&mut self) {
        if let Some(node) = self.flat.get(self.selected) {
            let depth = node.depth;
            let is_expanded_dir = node.is_dir && node.expanded;

            if is_expanded_dir {
                // It's an open folder, so just close it.
                self.toggle_expand();
            } else if depth > 0 {
                // Move selection to the immediate parent in the tree hierarchy.
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
