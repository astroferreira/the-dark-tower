//! Legends mode renderer.
//!
//! Renders the legends mode views to ratatui terminal buffers.

use ratatui::{
    prelude::*,
    widgets::{Block, Borders, Clear, Scrollbar, ScrollbarOrientation, ScrollbarState},
    style::{Color, Style, Modifier},
};

use super::mode::{LegendsMode, LegendsView};
use super::queries::Category;

/// Render the legends mode UI into the given area.
pub fn render_legends(mode: &LegendsMode, area: Rect, buf: &mut Buffer) {
    // Clear area
    Clear.render(area, buf);

    // Title block
    let title = mode.title();
    let block = Block::default()
        .title(format!(" {} ", title))
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Magenta))
        .style(Style::default().bg(Color::Black));

    let inner = block.inner(area);
    block.render(area, buf);

    match &mode.view {
        LegendsView::CategoryMenu => {
            render_category_menu(mode.category_selected, inner, buf);
        }
        LegendsView::EntityList { entries, selected, scroll, search, .. } => {
            render_entity_list(entries, *selected, *scroll, search, inner, buf);
        }
        LegendsView::EntityDetail { detail, scroll, selected, .. } => {
            render_entity_detail(detail, *scroll, *selected, inner, buf);
        }
    }

    // Footer with controls
    let footer_y = area.y + area.height.saturating_sub(1);
    if footer_y > area.y {
        let controls = match &mode.view {
            LegendsView::CategoryMenu => " [Enter] Select  [Esc/Q] Exit ".to_string(),
            LegendsView::EntityList { .. } => " [Enter] View  [Esc] Back  [Type] Search  [Backspace] Clear ".to_string(),
            LegendsView::EntityDetail { .. } => {
                if mode.current_link().is_some() {
                    " [Enter] Open  [Esc] Back  [Up/Down] Navigate ".to_string()
                } else {
                    " [Esc] Back  [Up/Down] Navigate ".to_string()
                }
            }
        };
        let style = Style::default().fg(Color::DarkGray);
        let truncated: String = controls.chars().take(area.width as usize).collect();
        buf.set_string(area.x, footer_y, &truncated, style);
    }
}

fn render_category_menu(selected: usize, area: Rect, buf: &mut Buffer) {
    let categories = Category::all();

    for (i, cat) in categories.iter().enumerate() {
        if i as u16 >= area.height {
            break;
        }

        let prefix = if i == selected { "> " } else { "  " };
        let text = format!("{}{}", prefix, cat.name());

        let style = if i == selected {
            Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD)
        } else {
            Style::default().fg(Color::White)
        };

        let truncated: String = text.chars().take(area.width as usize).collect();
        buf.set_string(area.x, area.y + i as u16, &truncated, style);
    }
}

fn render_entity_list(
    entries: &[super::queries::ListEntry],
    selected: usize,
    scroll: usize,
    search: &str,
    area: Rect,
    buf: &mut Buffer,
) {
    // Search bar at top
    let search_line = if search.is_empty() {
        " (type to search)".to_string()
    } else {
        format!(" Search: {}_", search)
    };
    buf.set_string(area.x, area.y, &search_line, Style::default().fg(Color::Cyan));

    let list_start = area.y + 1;
    let list_height = (area.height as usize).saturating_sub(1);

    if entries.is_empty() {
        buf.set_string(area.x + 1, list_start, "No entries found.", Style::default().fg(Color::DarkGray));
        return;
    }

    for (i, entry) in entries.iter().skip(scroll).enumerate() {
        if i >= list_height {
            break;
        }

        let abs_idx = scroll + i;
        let is_selected = abs_idx == selected;

        let prefix = if is_selected { "> " } else { "  " };
        let text = format!("{}{}", prefix, entry.label);

        let style = if is_selected {
            Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD)
        } else {
            Style::default().fg(Color::White)
        };

        // Use two lines per entry if space allows
        let row = list_start + i as u16;
        if row < area.y + area.height {
            let truncated: String = text.chars().take(area.width as usize).collect();
            buf.set_string(area.x, row, &truncated, style);
        }
    }
}

fn render_entity_detail(
    detail: &super::queries::EntityDetail,
    scroll: usize,
    selected: usize,
    area: Rect,
    buf: &mut Buffer,
) {
    // Reserve 1 column on the right for scrollbar
    let content_width = area.width.saturating_sub(1) as usize;
    let visible_height = area.height as usize;

    for (i, line) in detail.lines.iter().skip(scroll).enumerate() {
        if i >= visible_height {
            break;
        }

        let abs_idx = scroll + i;
        let is_cursor = abs_idx == selected;
        let has_link = line.link.is_some();

        let style = if is_cursor && has_link {
            // Cursor on a link: cyan bg, black text, bold
            Style::default()
                .fg(Color::Black)
                .bg(Color::Cyan)
                .add_modifier(Modifier::BOLD)
        } else if is_cursor {
            // Cursor on non-link: dark gray bg, white text
            Style::default()
                .fg(Color::White)
                .bg(Color::DarkGray)
        } else if has_link {
            // Link (no cursor): cyan text, underlined
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::UNDERLINED)
        } else if line.highlight {
            // Section header: yellow, bold
            Style::default()
                .fg(Color::Yellow)
                .add_modifier(Modifier::BOLD)
        } else {
            // Normal text
            Style::default().fg(Color::White)
        };

        let truncated: String = line.text.chars().take(content_width).collect();
        // Pad to content_width so cursor background covers the full row
        let padded = if is_cursor {
            format!("{:<width$}", truncated, width = content_width)
        } else {
            truncated
        };
        buf.set_string(area.x, area.y + i as u16, &padded, style);
    }

    // Render scrollbar if content exceeds visible height
    if detail.lines.len() > visible_height {
        let scrollbar_area = Rect {
            x: area.x + area.width.saturating_sub(1),
            y: area.y,
            width: 1,
            height: area.height,
        };
        let mut scrollbar_state = ScrollbarState::new(detail.lines.len().saturating_sub(visible_height))
            .position(scroll);
        let scrollbar = Scrollbar::new(ScrollbarOrientation::VerticalRight)
            .begin_symbol(Some("^"))
            .end_symbol(Some("v"))
            .track_symbol(Some("|"))
            .thumb_symbol("#");
        scrollbar.render(scrollbar_area, buf, &mut scrollbar_state);
    }
}
