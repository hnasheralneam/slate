use crate::app::{App, Mode, Pane};
use crate::highlight::{apply_search_highlight, highlight_markdown_line};
use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span, Text},
    widgets::{Block, Borders, Clear, List, ListItem, Paragraph},
    Frame,
};
use std::path::PathBuf;

pub fn draw(f: &mut Frame, app: &mut App) {
    let size = f.area();

    // Outer split: sidebar | right_column
    let h_chunks = if app.sidebar_visible {
        Layout::default()
            .direction(Direction::Horizontal)
            .constraints([Constraint::Length(30), Constraint::Min(0)])
            .split(size)
    } else {
        Layout::default()
            .direction(Direction::Horizontal)
            .constraints([Constraint::Length(0), Constraint::Min(0)])
            .split(size)
    };

    // Right column: tab_bar | editor | status
    let v_chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(1),  // tab bar
            Constraint::Min(0),     // editor
            Constraint::Length(1),  // status bar
        ])
        .split(h_chunks[1]);

    // viewport_height = editor area minus borders
    app.viewport_height = v_chunks[1].height.saturating_sub(2) as usize;

    if app.sidebar_visible {
        draw_sidebar(f, app, h_chunks[0]);
    }
    draw_tab_bar(f, app, v_chunks[0]);
    draw_editor(f, app, v_chunks[1]);
    draw_statusbar(f, app, v_chunks[2]);

    // Overlays
    match &app.mode {
        Mode::InFileSearch => draw_search_bar(f, app, h_chunks[1]),
        Mode::FileOpen     => draw_file_open(f, app, size),
        Mode::GlobalSearch => draw_global_search(f, app, size),
        _ => {}
    }
}

// ── Tab bar ───────────────────────────────────────────────────────────────────

fn draw_tab_bar(f: &mut Frame, app: &App, area: Rect) {
    let mut spans: Vec<Span> = Vec::new();

    for (i, tab) in app.tabs.iter().enumerate() {
        let is_active = i == app.active_tab;
        let title = tab.title();

        if is_active {
            spans.push(Span::styled(
                format!(" {} ", title),
                Style::default()
                    .bg(Color::Rgb(50, 80, 130))
                    .fg(Color::White)
                    .add_modifier(Modifier::BOLD),
            ));
        } else {
            spans.push(Span::styled(
                format!(" {} ", title),
                Style::default().fg(Color::DarkGray).bg(Color::Rgb(20, 20, 20)),
            ));
        }
        // Separator
        spans.push(Span::styled("│", Style::default().fg(Color::Rgb(50, 50, 50))));
    }

    // Fill rest of bar
    spans.push(Span::styled(
        " ".repeat(area.width as usize),
        Style::default().bg(Color::Rgb(20, 20, 20)),
    ));

    f.render_widget(
        Paragraph::new(Line::from(spans)).style(Style::default().bg(Color::Rgb(20, 20, 20))),
        area,
    );
}

// ── Sidebar ───────────────────────────────────────────────────────────────────

fn draw_sidebar(f: &mut Frame, app: &App, area: Rect) {
    let focused = app.active_pane == Pane::Sidebar;
    let border_style = if focused {
        Style::default().fg(Color::Blue)
    } else {
        Style::default().fg(Color::DarkGray)
    };

    let block = Block::default()
        .title(" Explorer ")
        .borders(Borders::ALL)
        .border_style(border_style);
    let inner = block.inner(area);
    f.render_widget(block, area);

    let visible_height = inner.height as usize;
    let scroll_offset = if app.file_tree.selected >= visible_height {
        app.file_tree.selected - visible_height + 1
    } else {
        0
    };

    let active_path = app.tab().path.as_deref();

    let items: Vec<ListItem> = app.file_tree.flat
        .iter()
        .enumerate()
        .skip(scroll_offset)
        .take(visible_height)
        .map(|(idx, node)| {
            let indent = "  ".repeat(node.depth);
            let icon = if node.is_dir {
                if node.expanded { "▾ " } else { "▸ " }
            } else {
                file_icon(&node.path)
            };

            // Highlight if open in any tab
            let open_in_tab = app.tabs.iter().any(|t| t.path.as_deref() == Some(&node.path));
            let is_active_file = active_path == Some(&node.path);

            let style = if idx == app.file_tree.selected {
                Style::default().bg(Color::Rgb(50, 80, 130)).fg(Color::White)
            } else if is_active_file {
                Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD)
            } else if open_in_tab {
                Style::default().fg(Color::Cyan)
            } else if node.is_dir {
                Style::default().fg(Color::Yellow)
            } else {
                Style::default().fg(Color::White)
            };

            ListItem::new(Line::from(vec![
                Span::raw(indent),
                Span::styled(format!("{}{}", icon, node.name), style),
            ]))
        })
        .collect();

    f.render_widget(List::new(items), inner);
}

fn file_icon(path: &PathBuf) -> &'static str {
    match path.extension().and_then(|e| e.to_str()).unwrap_or("") {
        "md"                              => "󰆼 ",
        "txt"                             => " ",
        "sh"                              => " ",
        "rs"                              => " ",
        "py"                              => " ",
        "js" | "ts"                       => " ",
        "json"                            => " ",
        "toml" | "yaml" | "yml"           => " ",
        "png" | "jpg" | "jpeg" | "gif"    => " ",
        "pdf"                             => " ",
        _                                 => " ",
    }
}

// ── Editor ────────────────────────────────────────────────────────────────────

fn draw_editor(f: &mut Frame, app: &App, area: Rect) {
    let is_insert = app.mode == Mode::Insert;
    let tab = app.tab();

    let title = match &tab.path {
        None => " [ new tab ] ".to_string(),
        Some(p) => {
            let name = p.file_name().and_then(|n| n.to_str()).unwrap_or("?");
            let dirty = if tab.dirty { " [+]" } else { "" };
            format!(" {}{} ", name, dirty)
        }
    };

    let border_color = if is_insert {
        Color::Green
    } else if app.active_pane == Pane::Editor {
        Color::Blue
    } else {
        Color::DarkGray
    };

    let block = Block::default()
        .title(title)
        .borders(Borders::ALL)
        .border_style(Style::default().fg(border_color));
    let inner = block.inner(area);
    f.render_widget(block, area);

    if tab.content.is_empty() {
        let help = Paragraph::new(Text::from(vec![
            Line::from(""),
            Line::from(Span::styled("  noted", Style::default().fg(Color::Blue).add_modifier(Modifier::BOLD))),
            Line::from(""),
            Line::from(vec![Span::styled("  Ctrl+T  ", Style::default().fg(Color::Yellow)), Span::raw("New tab")]),
            Line::from(vec![Span::styled("  Ctrl+W  ", Style::default().fg(Color::Yellow)), Span::raw("Close tab")]),
            Line::from(vec![Span::styled("  Alt+←/→ ", Style::default().fg(Color::Yellow)), Span::raw("Switch tab")]),
            Line::from(vec![Span::styled("  Alt+1-9 ", Style::default().fg(Color::Yellow)), Span::raw("Jump to tab")]),
            Line::from(""),
            Line::from(vec![Span::styled("  Ctrl+P  ", Style::default().fg(Color::Yellow)), Span::raw("Open file")]),
            Line::from(vec![Span::styled("  Ctrl+B  ", Style::default().fg(Color::Yellow)), Span::raw("Toggle sidebar")]),
            Line::from(vec![Span::styled("  Ctrl+G  ", Style::default().fg(Color::Yellow)), Span::raw("Grep all files")]),
            Line::from(vec![Span::styled("  Ctrl+F  ", Style::default().fg(Color::Yellow)), Span::raw("Search in file")]),
            Line::from(vec![Span::styled("  e / i   ", Style::default().fg(Color::Yellow)), Span::raw("Edit file")]),
            Line::from(vec![Span::styled("  Ctrl+S  ", Style::default().fg(Color::Yellow)), Span::raw("Save")]),
            Line::from(vec![Span::styled("  Ctrl+Q  ", Style::default().fg(Color::Yellow)), Span::raw("Quit")]),
        ]));
        f.render_widget(help, inner);
        return;
    }

    let mut in_code_block = false;
    let cursor_line = tab.cursor_line;
    let cursor_col  = tab.cursor_col;

    let lines: Vec<Line> = tab.content
        .iter()
        .enumerate()
        .skip(tab.scroll_offset)
        .take(inner.height as usize)
        .map(|(line_idx, raw_line)| {
            if raw_line.starts_with("```") {
                in_code_block = !in_code_block;
            }

            let styled_spans = if in_code_block && !raw_line.starts_with("```") {
                vec![crate::highlight::StyledSpan {
                    text: raw_line.clone(),
                    style: Style::default().fg(Color::Rgb(220, 220, 170)),
                }]
            } else {
                highlight_markdown_line(raw_line)
            };

            let search_matches = tab.in_file_search.matches_on_line(line_idx);
            let is_cur_match   = tab.in_file_search.current_match().map(|m| m.line == line_idx).unwrap_or(false);

            let final_spans = if !search_matches.is_empty() {
                apply_search_highlight(&styled_spans, &search_matches, is_cur_match)
            } else {
                styled_spans
            };

            let gutter_style = if line_idx == cursor_line {
                Style::default().fg(Color::White)
            } else {
                Style::default().fg(Color::Rgb(70, 70, 70))
            };
            let gutter = Span::styled(format!("{:4} ", line_idx + 1), gutter_style);

            let mut spans: Vec<Span> = vec![gutter];

            if is_insert && line_idx == cursor_line {
                let chars: Vec<char> = raw_line.chars().collect();
                let col = cursor_col.min(chars.len());
                let before: String = chars[..col].iter().collect();
                let cur_ch = if col < chars.len() { chars[col].to_string() } else { " ".to_string() };
                let after: String  = if col < chars.len() { chars[col+1..].iter().collect() } else { String::new() };

                for s in highlight_markdown_line(&before) {
                    spans.push(Span::styled(s.text, s.style));
                }
                spans.push(Span::styled(cur_ch, Style::default().bg(Color::White).fg(Color::Black)));
                if !after.is_empty() {
                    for s in highlight_markdown_line(&after) {
                        spans.push(Span::styled(s.text, s.style));
                    }
                }
            } else {
                for s in final_spans {
                    spans.push(Span::styled(s.text, s.style));
                }
            }

            Line::from(spans)
        })
        .collect();

    f.render_widget(Paragraph::new(Text::from(lines)), inner);
}

// ── Status bar ────────────────────────────────────────────────────────────────

fn draw_statusbar(f: &mut Frame, app: &App, area: Rect) {
    let (mode_str, mode_color) = match &app.mode {
        Mode::Normal       => (" NORMAL ", Color::Blue),
        Mode::Insert       => (" INSERT ", Color::Green),
        Mode::SidePanel    => (" TREE   ", Color::Yellow),
        Mode::InFileSearch => (" FIND   ", Color::Cyan),
        Mode::FileOpen     => (" OPEN   ", Color::Magenta),
        Mode::GlobalSearch => (" GREP   ", Color::Cyan),
    };

    let tab = app.tab();

    let match_info = if !tab.in_file_search.query.is_empty() {
        let total = tab.in_file_search.matches.len();
        let cur   = if total > 0 { tab.in_file_search.current + 1 } else { 0 };
        format!("  [{}/{}]", cur, total)
    } else {
        String::new()
    };

    let tab_info = format!(
        "  tab {}/{}  ",
        app.active_tab + 1,
        app.tabs.len()
    );

    let pos_info = if !tab.content.is_empty() {
        format!("{}:{}", tab.cursor_line + 1, tab.cursor_col + 1)
    } else {
        String::new()
    };

    let line = Line::from(vec![
        Span::styled(
            format!(" {} ", mode_str),
            Style::default().bg(mode_color).fg(Color::Black).add_modifier(Modifier::BOLD),
        ),
        Span::styled(
            format!(" {}{}", app.status_msg, match_info),
            Style::default().fg(Color::Rgb(180, 180, 180)),
        ),
        Span::styled(tab_info, Style::default().fg(Color::DarkGray)),
        Span::styled(pos_info, Style::default().fg(Color::DarkGray)),
    ]);
    f.render_widget(Paragraph::new(Text::from(vec![line])), area);
}

// ── Overlays ──────────────────────────────────────────────────────────────────

fn draw_search_bar(f: &mut Frame, app: &App, area: Rect) {
    let bar_width = 50.min(area.width.saturating_sub(4));
    let bar_area = Rect {
        x: area.x + area.width - bar_width - 2,
        y: area.y + area.height.saturating_sub(4),
        width: bar_width,
        height: 3,
    };
    f.render_widget(Clear, bar_area);

    let tab   = app.tab();
    let total = tab.in_file_search.matches.len();
    let cur   = if total > 0 { tab.in_file_search.current + 1 } else { 0 };

    let block = Block::default()
        .title(format!(" Find [{}/{}] ", cur, total))
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Green));
    let inner = block.inner(bar_area);
    f.render_widget(block, bar_area);

    f.render_widget(Paragraph::new(Line::from(vec![
        Span::raw(&tab.in_file_search.query),
        Span::styled("█", Style::default().fg(Color::Green)),
    ])), inner);
}

fn draw_file_open(f: &mut Frame, app: &App, area: Rect) {
    let w = (area.width as f32 * 0.6) as u16;
    let h = 20u16.min(area.height.saturating_sub(4));
    let modal = centered(area, w, h);
    f.render_widget(Clear, modal);

    let block = Block::default()
        .title(" Open File — ↑↓ select  Enter open  Esc cancel ")
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Magenta));
    let inner = block.inner(modal);
    f.render_widget(block, modal);

    let input_area = Rect { height: 1, ..inner };
    let sep_area   = Rect { y: inner.y + 1, height: 1, ..inner };
    let list_area  = Rect { y: inner.y + 2, height: inner.height.saturating_sub(2), ..inner };

    f.render_widget(Paragraph::new(Line::from(vec![
        Span::styled("› ", Style::default().fg(Color::Magenta)),
        Span::raw(&app.file_open.query),
        Span::styled("█", Style::default().fg(Color::Magenta)),
    ])), input_area);

    f.render_widget(Paragraph::new(Span::styled(
        "─".repeat(inner.width as usize),
        Style::default().fg(Color::DarkGray),
    )), sep_area);

    let items: Vec<ListItem> = app.file_open.results.iter().enumerate().map(|(i, path)| {
        let name   = path.file_name().and_then(|n| n.to_str()).unwrap_or("?");
        let parent = path.parent().and_then(|p| p.to_str()).unwrap_or("");
        let style  = if i == app.file_open.selected {
            Style::default().bg(Color::Rgb(50, 80, 130))
        } else {
            Style::default()
        };
        ListItem::new(Line::from(vec![
            Span::styled(format!("  {} ", name), style.add_modifier(Modifier::BOLD)),
            Span::styled(format!("  {}", parent), style.fg(Color::DarkGray)),
        ]))
    }).collect();
    f.render_widget(List::new(items), list_area);
}

fn draw_global_search(f: &mut Frame, app: &App, area: Rect) {
    let w = (area.width as f32 * 0.8) as u16;
    let h = (area.height as f32 * 0.7) as u16;
    let modal = centered(area, w, h);
    f.render_widget(Clear, modal);

    let block = Block::default()
        .title(" Grep All Files — ↑↓ select  Enter jump  Esc cancel ")
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Cyan));
    let inner = block.inner(modal);
    f.render_widget(block, modal);

    let input_area = Rect { height: 1, ..inner };
    let sep_area   = Rect { y: inner.y + 1, height: 1, ..inner };
    let list_area  = Rect { y: inner.y + 2, height: inner.height.saturating_sub(2), ..inner };

    let count_str = if app.global_search.results.len() >= 200 {
        "200+".to_string()
    } else {
        app.global_search.results.len().to_string()
    };
    let pending = if app.global_search.dirty { " …" } else { "" };

    f.render_widget(Paragraph::new(Line::from(vec![
        Span::styled("› ", Style::default().fg(Color::Cyan)),
        Span::raw(&app.global_search.query),
        Span::styled("█", Style::default().fg(Color::Cyan)),
        Span::styled(
            format!("   ({} matches{})", count_str, pending),
            Style::default().fg(Color::DarkGray),
        ),
    ])), input_area);

    f.render_widget(Paragraph::new(Span::styled(
        "─".repeat(inner.width as usize),
        Style::default().fg(Color::DarkGray),
    )), sep_area);

    let visible = list_area.height as usize;
    let scroll  = if app.global_search.selected >= visible {
        app.global_search.selected - visible + 1
    } else { 0 };

    let items: Vec<ListItem> = app.global_search.results.iter().enumerate()
        .skip(scroll).take(visible)
        .map(|(i, m)| {
            let fname = m.path.file_name().and_then(|n| n.to_str()).unwrap_or("?");
            let is_sel = i == app.global_search.selected;
            let bg = if is_sel { Color::Rgb(30, 60, 100) } else { Color::Reset };

            let lo = m.col_start.min(m.line_text.len());
            let hi = m.col_end.min(m.line_text.len());
            let before  = &m.line_text[..lo];
            let matched = &m.line_text[lo..hi];
            let after   = &m.line_text[hi..];
            let bt = if before.len() > 20 { &before[before.len()-20..] } else { before };
            let at = if after.len()  > 40 { &after[..40] } else { after };

            ListItem::new(Line::from(vec![
                Span::styled(format!("  {:20} {:4}  ", fname, m.line_no + 1),
                    Style::default().bg(bg).fg(Color::Yellow)),
                Span::styled(bt.to_string(),      Style::default().bg(bg).fg(Color::White)),
                Span::styled(matched.to_string(),
                    Style::default().bg(Color::Cyan).fg(Color::Black).add_modifier(Modifier::BOLD)),
                Span::styled(at.to_string(),      Style::default().bg(bg).fg(Color::White)),
            ]))
        }).collect();

    f.render_widget(List::new(items), list_area);
}

fn centered(area: Rect, w: u16, h: u16) -> Rect {
    Rect {
        x: (area.width.saturating_sub(w)) / 2,
        y: (area.height.saturating_sub(h)) / 2,
        width: w,
        height: h,
    }
}
