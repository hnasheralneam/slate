use ratatui::style::{Color, Modifier, Style};

/// A span of styled text
#[derive(Debug, Clone)]
pub struct StyledSpan {
    pub text: String,
    pub style: Style,
}

/// Parse a single markdown line into styled spans
pub fn highlight_markdown_line(line: &str) -> Vec<StyledSpan> {
    // Headings
    if let Some(rest) = line.strip_prefix("### ") {
        return vec![
            StyledSpan { text: "### ".into(), style: Style::default().fg(Color::DarkGray) },
            StyledSpan { text: rest.to_string(), style: Style::default().fg(Color::LightCyan).add_modifier(Modifier::BOLD) },
        ];
    }
    if let Some(rest) = line.strip_prefix("## ") {
        return vec![
            StyledSpan { text: "## ".into(), style: Style::default().fg(Color::DarkGray) },
            StyledSpan { text: rest.to_string(), style: Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD) },
        ];
    }
    if let Some(rest) = line.strip_prefix("# ") {
        return vec![
            StyledSpan { text: "# ".into(), style: Style::default().fg(Color::DarkGray) },
            StyledSpan { text: rest.to_string(), style: Style::default().fg(Color::Blue).add_modifier(Modifier::BOLD) },
        ];
    }

    // Horizontal rule
    if line == "---" || line == "***" || line == "===" {
        return vec![StyledSpan {
            text: line.to_string(),
            style: Style::default().fg(Color::DarkGray),
        }];
    }

    // Blockquote
    if line.starts_with("> ") {
        let rest = &line[2..];
        return vec![
            StyledSpan { text: "> ".into(), style: Style::default().fg(Color::DarkGray) },
            StyledSpan { text: rest.to_string(), style: Style::default().fg(Color::Rgb(180, 180, 180)).add_modifier(Modifier::ITALIC) },
        ];
    }

    // Code block fences
    if line.starts_with("```") {
        return vec![StyledSpan {
            text: line.to_string(),
            style: Style::default().fg(Color::Yellow),
        }];
    }

    // Unordered list
    let ul_stripped = if line.starts_with("- ") || line.starts_with("* ") {
        Some((&line[..2], &line[2..]))
    } else if line.starts_with("  - ") || line.starts_with("  * ") {
        Some((&line[..4], &line[4..]))
    } else {
        None
    };

    if let Some((bullet, rest)) = ul_stripped {
        let mut spans = vec![
            StyledSpan { text: bullet.to_string(), style: Style::default().fg(Color::Green) },
        ];
        spans.extend(parse_inline(rest));
        return spans;
    }

    // Ordered list
    let trimmed = line.trim_start();
    let leading_spaces = line.len() - trimmed.len();
    if let Some(pos) = trimmed.find(". ") {
        let maybe_num = &trimmed[..pos];
        if maybe_num.chars().all(|c| c.is_ascii_digit()) {
            let mut spans = vec![
                StyledSpan {
                    text: format!("{}{}", &line[..leading_spaces], &trimmed[..pos + 2]),
                    style: Style::default().fg(Color::Green),
                },
            ];
            spans.extend(parse_inline(&trimmed[pos + 2..]));
            return spans;
        }
    }

    // Task list
    if line.starts_with("- [ ] ") {
        return vec![
            StyledSpan { text: "- [ ] ".into(), style: Style::default().fg(Color::Yellow) },
            StyledSpan { text: line[6..].to_string(), style: Style::default() },
        ];
    }
    if line.starts_with("- [x] ") || line.starts_with("- [X] ") {
        return vec![
            StyledSpan { text: "- [x] ".into(), style: Style::default().fg(Color::Green) },
            StyledSpan { text: line[6..].to_string(), style: Style::default().fg(Color::DarkGray).add_modifier(Modifier::CROSSED_OUT) },
        ];
    }

    // Default: parse inline markdown
    parse_inline(line)
}

/// Parse inline markdown (bold, italic, code, links) into styled spans
fn parse_inline(line: &str) -> Vec<StyledSpan> {
    let mut spans = Vec::new();
    let chars: Vec<char> = line.chars().collect();
    let len = chars.len();
    let mut i = 0;
    let mut current = String::new();

    while i < len {
        // Inline code: `code`
        if chars[i] == '`' {
            if !current.is_empty() {
                spans.push(StyledSpan { text: current.clone(), style: Style::default() });
                current.clear();
            }
            let start = i + 1;
            let mut end = start;
            while end < len && chars[end] != '`' {
                end += 1;
            }
            let code: String = chars[start..end].iter().collect();
            spans.push(StyledSpan {
                text: format!("`{}`", code),
                style: Style::default().fg(Color::Yellow).bg(Color::Rgb(40, 40, 40)),
            });
            i = end + 1;
            continue;
        }

        // Bold+italic: ***text***
        if i + 2 < len && chars[i] == '*' && chars[i+1] == '*' && chars[i+2] == '*' {
            if !current.is_empty() {
                spans.push(StyledSpan { text: current.clone(), style: Style::default() });
                current.clear();
            }
            let start = i + 3;
            let mut end = start;
            while end + 2 < len {
                if chars[end] == '*' && chars[end+1] == '*' && chars[end+2] == '*' {
                    break;
                }
                end += 1;
            }
            let text: String = chars[start..end].iter().collect();
            spans.push(StyledSpan {
                text: format!("***{}***", text),
                style: Style::default().add_modifier(Modifier::BOLD | Modifier::ITALIC),
            });
            i = end + 3;
            continue;
        }

        // Bold: **text**
        if i + 1 < len && chars[i] == '*' && chars[i+1] == '*' {
            if !current.is_empty() {
                spans.push(StyledSpan { text: current.clone(), style: Style::default() });
                current.clear();
            }
            let start = i + 2;
            let mut end = start;
            while end + 1 < len {
                if chars[end] == '*' && chars[end+1] == '*' {
                    break;
                }
                end += 1;
            }
            let text: String = chars[start..end].iter().collect();
            spans.push(StyledSpan {
                text: format!("**{}**", text),
                style: Style::default().add_modifier(Modifier::BOLD),
            });
            i = end + 2;
            continue;
        }

        // Italic: *text* or _text_
        if (chars[i] == '*' || chars[i] == '_') && i + 1 < len {
            let marker = chars[i];
            if !current.is_empty() {
                spans.push(StyledSpan { text: current.clone(), style: Style::default() });
                current.clear();
            }
            let start = i + 1;
            let mut end = start;
            while end < len && chars[end] != marker {
                end += 1;
            }
            let text: String = chars[start..end].iter().collect();
            spans.push(StyledSpan {
                text: format!("{}{}{}", marker, text, marker),
                style: Style::default().add_modifier(Modifier::ITALIC),
            });
            i = end + 1;
            continue;
        }

        // Link: [text](url)
        if chars[i] == '[' {
            let _link_start = i;
            let mut bracket_end = i + 1;
            while bracket_end < len && chars[bracket_end] != ']' {
                bracket_end += 1;
            }
            if bracket_end < len && bracket_end + 1 < len && chars[bracket_end + 1] == '(' {
                let paren_start = bracket_end + 2;
                let mut paren_end = paren_start;
                while paren_end < len && chars[paren_end] != ')' {
                    paren_end += 1;
                }
                if paren_end < len {
                    if !current.is_empty() {
                        spans.push(StyledSpan { text: current.clone(), style: Style::default() });
                        current.clear();
                    }
                    let link_text: String = chars[i+1..bracket_end].iter().collect();
                    let url: String = chars[paren_start..paren_end].iter().collect();
                    spans.push(StyledSpan {
                        text: format!("[{}]({})", link_text, url),
                        style: Style::default().fg(Color::Blue).add_modifier(Modifier::UNDERLINED),
                    });
                    i = paren_end + 1;
                    continue;
                }
            }
        }

        // Wiki link: [[text]]
        if i + 1 < len && chars[i] == '[' && chars[i+1] == '[' {
            if !current.is_empty() {
                spans.push(StyledSpan { text: current.clone(), style: Style::default() });
                current.clear();
            }
            let start = i + 2;
            let mut end = start;
            while end + 1 < len {
                if chars[end] == ']' && chars[end+1] == ']' {
                    break;
                }
                end += 1;
            }
            let text: String = chars[start..end].iter().collect();
            spans.push(StyledSpan {
                text: format!("[[{}]]", text),
                style: Style::default().fg(Color::Cyan).add_modifier(Modifier::UNDERLINED),
            });
            i = end + 2;
            continue;
        }

        current.push(chars[i]);
        i += 1;
    }

    if !current.is_empty() {
        spans.push(StyledSpan { text: current, style: Style::default() });
    }

    spans
}

/// Apply search highlight on top of existing spans
pub fn apply_search_highlight(
    spans: &[StyledSpan],
    matches: &[(usize, usize)],
    is_current: bool,
) -> Vec<StyledSpan> {
    if matches.is_empty() {
        return spans.to_vec();
    }

    let _full_text: String = spans.iter().map(|s| s.text.as_str()).collect();
    let mut result = Vec::new();
    let mut char_pos = 0;

    let highlight_bg = if is_current {
        Color::Rgb(200, 120, 0)
    } else {
        Color::Rgb(100, 80, 0)
    };

    for span in spans {
        let span_start = char_pos;
        let span_end = char_pos + span.text.len();

        let mut cursor = span_start;
        for &(ms, me) in matches {
            // Overlap region
            let ol_start = ms.max(span_start);
            let ol_end = me.min(span_end);

            if ol_start >= ol_end {
                continue;
            }

            // Before overlap
            if cursor < ol_start {
                result.push(StyledSpan {
                    text: span.text[cursor - span_start..ol_start - span_start].to_string(),
                    style: span.style,
                });
            }
            // Highlighted
            result.push(StyledSpan {
                text: span.text[ol_start - span_start..ol_end - span_start].to_string(),
                style: span.style.bg(highlight_bg).fg(Color::White).add_modifier(Modifier::BOLD),
            });
            cursor = ol_end;
        }

        if cursor < span_end {
            result.push(StyledSpan {
                text: span.text[cursor - span_start..].to_string(),
                style: span.style,
            });
        }

        char_pos = span_end;
    }

    result
}
