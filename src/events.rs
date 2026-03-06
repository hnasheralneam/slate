use crate::app::{App, Mode, Pane};
use crate::search::SearchState;
use anyhow::Result;
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers, MouseEvent};

/// Main keyboard event handler. Dispatches to the appropriate sub-handler based on current mode.
pub fn handle_key(app: &mut App, key: KeyEvent) -> Result<bool> {
    let area = app.editor_area;
    
    // Dispatch based on the current mode
    match &app.mode {
        Mode::InFileSearch => return handle_key_in_file_search(app, key),
        Mode::FileOpen => return handle_key_file_open(app, key),
        Mode::GlobalSearch => return handle_key_global_search(app, key),
        Mode::SidePanel => return handle_key_sidebar(app, key),
        Mode::Insert => {
            // Escape to normal mode
            if key.code == KeyCode::Esc {
                app.mode = Mode::Normal;
                app.update_status();
                return Ok(false);
            }
            // Save shortcut
            if key.modifiers == KeyModifiers::CONTROL && key.code == KeyCode::Char('s') {
                app.save_file()?;
                return Ok(false);
            }
            // Delegate other keys to the editor component
            app.tab_mut().editor.input(key, &area)?;
            app.tab_mut().dirty = true;
            return Ok(false);
        }
        Mode::Normal => {}
    }

    // Normal mode keybindings
    match (key.modifiers, key.code) {
        // Quit
        (KeyModifiers::CONTROL, KeyCode::Char('q')) => return Ok(true),

        // Tab management
        (KeyModifiers::CONTROL, KeyCode::Char('t')) => app.new_tab(),
        (KeyModifiers::CONTROL, KeyCode::Char('w')) => app.close_tab(),

        // Switch tabs
        (KeyModifiers::ALT, KeyCode::Right) | (KeyModifiers::ALT, KeyCode::Char('l')) => app.next_tab(),
        (KeyModifiers::ALT, KeyCode::Left) | (KeyModifiers::ALT, KeyCode::Char('h')) => app.prev_tab(),

        // Jump to specific tab using Alt+1..9
        (KeyModifiers::ALT, KeyCode::Char(c)) if c.is_ascii_digit() => {
            let n = c as usize - '1' as usize;
            app.goto_tab(n);
        }

        // Toggle or focus sidebar
        (KeyModifiers::CONTROL, KeyCode::Char('b')) => {
            app.sidebar_visible = !app.sidebar_visible;
        }
        (KeyModifiers::CONTROL, KeyCode::Char('e')) => {
            if app.sidebar_visible {
                app.mode = Mode::SidePanel;
                app.active_pane = Pane::Sidebar;
            }
        }

        // Enter insert/edit mode
        (KeyModifiers::NONE, KeyCode::Char('e'))
        | (KeyModifiers::NONE, KeyCode::Char('i'))
        | (KeyModifiers::NONE, KeyCode::Enter) => {
            app.mode = Mode::Insert;
            app.status_msg = "-- INSERT -- Esc=normal  Ctrl+S=save".to_string();
        }

        // Save file
        (KeyModifiers::CONTROL, KeyCode::Char('s')) => app.save_file()?,

        // Search features
        (KeyModifiers::CONTROL, KeyCode::Char('f')) => {
            app.mode = Mode::InFileSearch;
            app.tab_mut().in_file_search = SearchState::new();
            app.tab_mut().editor.remove_marks();
        }
        (KeyModifiers::CONTROL, KeyCode::Char('p')) => {
            app.mode = Mode::FileOpen;
            app.file_open.all_files = crate::app::collect_all_files(&app.vault_path);
            app.file_open.filter();
        }
        (KeyModifiers::CONTROL, KeyCode::Char('g')) => {
            app.mode = Mode::GlobalSearch;
            app.global_search = crate::search::GlobalSearch::new();
        }

        // Scroll / movement bindings in normal mode (pass through to editor)
        (KeyModifiers::NONE, KeyCode::Down) | (KeyModifiers::NONE, KeyCode::Char('j')) => {
            let _ = app.tab_mut().editor.input(KeyEvent::new(KeyCode::Down, KeyModifiers::NONE), &area);
        }
        (KeyModifiers::NONE, KeyCode::Up) | (KeyModifiers::NONE, KeyCode::Char('k')) => {
            let _ = app.tab_mut().editor.input(KeyEvent::new(KeyCode::Up, KeyModifiers::NONE), &area);
        }
        (KeyModifiers::NONE, KeyCode::Left) | (KeyModifiers::NONE, KeyCode::Char('h')) => {
            let _ = app.tab_mut().editor.input(KeyEvent::new(KeyCode::Left, KeyModifiers::NONE), &area);
        }
        (KeyModifiers::NONE, KeyCode::Right) | (KeyModifiers::NONE, KeyCode::Char('l')) => {
            let _ = app.tab_mut().editor.input(KeyEvent::new(KeyCode::Right, KeyModifiers::NONE), &area);
        }
        (KeyModifiers::NONE, KeyCode::PageDown) => {
            let area_height = app.editor_area.height as usize;
            app.tab_mut().editor.scroll_down(area_height);
        }
        (KeyModifiers::NONE, KeyCode::PageUp) => {
            app.tab_mut().editor.scroll_up();
        }
        (KeyModifiers::NONE, KeyCode::Home) | (KeyModifiers::NONE, KeyCode::Char('0')) => {
            let _ = app.tab_mut().editor.input(KeyEvent::new(KeyCode::Home, KeyModifiers::NONE), &area);
        }
        (KeyModifiers::NONE, KeyCode::End) | (KeyModifiers::SHIFT, KeyCode::Char('$')) => {
            let _ = app.tab_mut().editor.input(KeyEvent::new(KeyCode::End, KeyModifiers::NONE), &area);
        }
        (KeyModifiers::SHIFT, KeyCode::Char('G')) => {
            let tab = app.tab_mut();
            let len = tab.editor.get_content().chars().count();
            tab.editor.set_cursor(len);
        }

        // Navigate search matches
        (KeyModifiers::NONE, KeyCode::Char('n')) => {
            let tab = app.tab_mut();
            let content = tab.editor.get_content();
            tab.in_file_search.next_match(&content);
            if let Some(m) = tab.in_file_search.current_match() {
                tab.editor.set_cursor(m.start_char);
            }
            tab.update_search_marks();
        }
        (KeyModifiers::SHIFT, KeyCode::Char('N')) => {
            let tab = app.tab_mut();
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

fn handle_key_sidebar(app: &mut App, key: KeyEvent) -> Result<bool> {
    match (key.modifiers, key.code) {
        (KeyModifiers::NONE, KeyCode::Esc) => {
            app.mode = Mode::Normal;
            app.active_pane = Pane::Editor;
        }
        (KeyModifiers::CONTROL, KeyCode::Char('b')) => {
            app.sidebar_visible = !app.sidebar_visible;
            app.mode = Mode::Normal;
            app.active_pane = Pane::Editor;
        }
        (KeyModifiers::NONE, KeyCode::Down) | (KeyModifiers::NONE, KeyCode::Char('j')) => {
            app.file_tree.move_down();
        }
        (KeyModifiers::NONE, KeyCode::Up) | (KeyModifiers::NONE, KeyCode::Char('k')) => {
            app.file_tree.move_up();
        }
        (KeyModifiers::NONE, KeyCode::Enter)
        | (KeyModifiers::NONE, KeyCode::Char('l'))
        | (KeyModifiers::NONE, KeyCode::Right) => {
            if let Some(path) = app.file_tree.selected_path() {
                if path.is_dir() {
                    app.file_tree.toggle_expand();
                } else {
                    let p = path.clone();
                    app.open_file(p)?;
                    app.mode = Mode::Normal;
                    app.active_pane = Pane::Editor;
                }
            }
        }
        (KeyModifiers::NONE, KeyCode::Char('h')) | (KeyModifiers::NONE, KeyCode::Left) => {
            app.file_tree.collapse_or_parent();
        }
        (KeyModifiers::NONE, KeyCode::Char(' ')) => {
            app.file_tree.toggle_expand();
        }
        _ => {}
    }
    Ok(false)
}

fn handle_key_in_file_search(app: &mut App, key: KeyEvent) -> Result<bool> {
    match (key.modifiers, key.code) {
        (KeyModifiers::NONE, KeyCode::Esc) => {
            app.mode = Mode::Normal;
            app.tab_mut().editor.remove_marks();
        }
        (KeyModifiers::NONE, KeyCode::Enter) => {
            let tab = app.tab_mut();
            let content = tab.editor.get_content();
            tab.in_file_search.next_match(&content);
            if let Some(m) = tab.in_file_search.current_match() {
                tab.editor.set_cursor(m.start_char);
            }
            tab.update_search_marks();
            app.mode = Mode::Normal;
        }
        (KeyModifiers::NONE, KeyCode::Backspace) => {
            let tab = app.tab_mut();
            tab.in_file_search.query.pop();
            let content = tab.editor.get_content();
            tab.in_file_search.recompute_matches(&content);
            tab.update_search_marks();
        }
        (KeyModifiers::NONE, KeyCode::Char(c)) => {
            let tab = app.tab_mut();
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

fn handle_key_file_open(app: &mut App, key: KeyEvent) -> Result<bool> {
    match (key.modifiers, key.code) {
        (KeyModifiers::NONE, KeyCode::Esc) => {
            app.mode = Mode::Normal;
        }
        (KeyModifiers::NONE, KeyCode::Enter) => {
            if let Some(path) = app.file_open.results.get(app.file_open.selected).cloned() {
                app.open_file(path)?;
            }
            app.mode = Mode::Normal;
        }
        (KeyModifiers::NONE, KeyCode::Down) | (KeyModifiers::CONTROL, KeyCode::Char('j')) => {
            if app.file_open.selected + 1 < app.file_open.results.len() {
                app.file_open.selected += 1;
            }
        }
        (KeyModifiers::NONE, KeyCode::Up) | (KeyModifiers::CONTROL, KeyCode::Char('k')) => {
            app.file_open.selected = app.file_open.selected.saturating_sub(1);
        }
        (KeyModifiers::NONE, KeyCode::Backspace) => {
            app.file_open.query.pop();
            app.file_open.filter();
        }
        (KeyModifiers::NONE, KeyCode::Char(c)) => {
            app.file_open.query.push(c);
            app.file_open.filter();
        }
        _ => {}
    }
    Ok(false)
}

fn handle_key_global_search(app: &mut App, key: KeyEvent) -> Result<bool> {
    match (key.modifiers, key.code) {
        (KeyModifiers::NONE, KeyCode::Esc) => {
            app.mode = Mode::Normal;
        }
        (KeyModifiers::NONE, KeyCode::Enter) => {
            if let Some((path, line_no, _)) = app.global_search.selected_result() {
                let path = path.clone();
                let line_no = line_no;
                app.open_file(path)?;

                let tab = app.tab_mut();
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

                app.mode = Mode::Normal;
            }
        }
        (KeyModifiers::NONE, KeyCode::Down) | (KeyModifiers::CONTROL, KeyCode::Char('j')) => {
            app.global_search.move_down();
        }
        (KeyModifiers::NONE, KeyCode::Up) | (KeyModifiers::CONTROL, KeyCode::Char('k')) => {
            app.global_search.move_up();
        }
        (KeyModifiers::NONE, KeyCode::Backspace) => {
            app.global_search.query.pop();
            app.global_search.mark_dirty();
        }
        (KeyModifiers::NONE, KeyCode::Char(c)) => {
            app.global_search.query.push(c);
            app.global_search.mark_dirty();
        }
        _ => {}
    }
    Ok(false)
}

pub fn handle_mouse(app: &mut App, mouse: MouseEvent) {
    let area = app.editor_area;
    if area.contains(ratatui::layout::Position::new(mouse.column, mouse.row)) {
        let _ = app.tab_mut().editor.mouse(mouse, &area);
    }
}
