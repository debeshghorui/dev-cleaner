use std::path::Path;

use ratatui::layout::{Constraint, Layout, Rect};
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Cell, Clear, Paragraph, Row, Table, Wrap};
use ratatui::Frame;

use crate::app::{App, Phase, Summary};
use crate::clean;
use crate::safety;
use crate::scan;
use crate::size;

pub fn splash(frame: &mut Frame, root: &Path, message: &str) {
    let block = Block::default().title(" devclean ").borders(Borders::ALL);
    let area = frame.area();
    let inner = block.inner(area);
    frame.render_widget(block, area);
    let text = Paragraph::new(vec![
        Line::from(message.to_string()),
        Line::from(""),
        Line::from(root.display().to_string()),
    ]);
    frame.render_widget(text, inner);
}

pub fn draw(frame: &mut Frame, app: &mut App) {
    match app.phase.clone() {
        Phase::Summary(summary) => {
            frame.render_widget(Clear, frame.area());
            draw_summary(frame, &summary);
        }
        phase => {
            draw_browse(frame, app);
            match phase {
                Phase::Confirm => draw_confirm(frame, app),
                Phase::Working => draw_working(frame, app),
                Phase::Browse | Phase::Summary(_) => {}
            }
        }
    }
}

fn draw_browse(frame: &mut Frame, app: &mut App) {
    let warning_rows = if app.show_warnings && !app.warnings.is_empty() {
        (app.warnings.len() as u16 + 2).clamp(3, 8)
    } else {
        0
    };
    let header_rows = if app.filter.is_empty() { 4 } else { 5 };
    let footer_rows = if app.notice.is_empty() { 3 } else { 4 };
    let chunks = if warning_rows == 0 {
        Layout::vertical([
            Constraint::Length(header_rows),
            Constraint::Min(5),
            Constraint::Length(footer_rows),
        ])
        .split(frame.area())
    } else {
        Layout::vertical([
            Constraint::Length(header_rows),
            Constraint::Min(5),
            Constraint::Length(warning_rows),
            Constraint::Length(footer_rows),
        ])
        .split(frame.area())
    };

    frame.render_widget(header(app), chunks[0]);
    draw_table(frame, app, chunks[1]);
    if warning_rows > 0 {
        frame.render_widget(warnings(app), chunks[2]);
        frame.render_widget(footer(app), chunks[3]);
    } else {
        frame.render_widget(footer(app), chunks[2]);
    }
}

fn header(app: &App) -> Paragraph<'static> {
    let title = Line::from(vec![
        Span::styled("devclean", Style::new().add_modifier(Modifier::BOLD)),
        Span::raw("  "),
        Span::raw(safety::os_label()),
        Span::raw("  "),
        Span::raw(app.root.display().to_string()),
    ]);
    let totals = Line::from(format!(
        "{} caches   {} reclaimable   {} selected ({})",
        app.rows.len(),
        size::format_bytes(app.total_bytes()),
        app.checked_count(),
        size::format_bytes(app.checked_bytes())
    ));
    let mut lines = vec![title, totals];
    if !app.filter.is_empty() {
        lines.push(Line::from(format!("Filter: {}", app.filter)));
    }
    Paragraph::new(lines).block(Block::default().borders(Borders::ALL))
}

fn draw_table(frame: &mut Frame, app: &mut App, area: Rect) {
    let visible = app.visible();
    let rows: Vec<Row> = visible
        .iter()
        .map(|&index| {
            let row = &app.rows[index];
            let mark = if row.checked { "[x]" } else { "[ ]" };
            Row::new(vec![
                Cell::from(mark),
                Cell::from(row.find.name.clone()),
                Cell::from(row.find.kind.label()),
                Cell::from(size::format_bytes(row.size)),
                Cell::from(scan::format_modified(row.find.modified)),
                Cell::from(row.find.path.display().to_string()),
            ])
        })
        .collect();

    let table = Table::new(
        rows,
        [
            Constraint::Length(3),
            Constraint::Length(16),
            Constraint::Length(8),
            Constraint::Length(10),
            Constraint::Length(12),
            Constraint::Min(20),
        ],
    )
    .header(
        Row::new(["", "Name", "Kind", "Size", "Modified", "Path"])
            .style(Style::new().add_modifier(Modifier::BOLD)),
    )
    .row_highlight_style(Style::new().add_modifier(Modifier::REVERSED))
    .block(
        Block::default()
            .borders(Borders::ALL)
            .title(" Dry run — nothing is deleted until you confirm "),
    )
    .column_spacing(1);

    if visible.is_empty() {
        let message = if app.rows.is_empty() {
            "No developer caches found under this folder."
        } else {
            "No caches match this filter."
        };
        let paragraph = Paragraph::new(message).block(
            Block::default()
                .borders(Borders::ALL)
                .title(" Dry run — nothing is deleted until you confirm "),
        );
        frame.render_widget(paragraph, area);
        return;
    }

    frame.render_stateful_widget(table, area, &mut app.table_state);
}

fn warnings(app: &App) -> Paragraph<'static> {
    let mut lines: Vec<Line> = app
        .warnings
        .iter()
        .take(6)
        .map(|warning| Line::from(warning.clone()))
        .collect();
    if app.warnings.len() > 6 {
        lines.push(Line::from(format!("… and {} more", app.warnings.len() - 6)));
    }
    Paragraph::new(lines)
        .wrap(Wrap { trim: true })
        .block(Block::default().borders(Borders::ALL).title(" Warnings "))
}

fn footer(app: &App) -> Paragraph<'static> {
    let keys = if app.filter_mode {
        "type to filter    backspace delete    enter apply    esc clear"
    } else {
        "↑↓ move    space toggle    a all    / filter    w warnings    enter clean    q quit"
    };
    let mut lines = Vec::new();
    if !app.notice.is_empty() {
        lines.push(Line::from(Span::styled(
            app.notice.clone(),
            Style::new().add_modifier(Modifier::BOLD),
        )));
    }
    lines.push(Line::from(keys));
    Paragraph::new(lines).block(Block::default().borders(Borders::ALL).title(" Keys "))
}

fn draw_confirm(frame: &mut Frame, app: &App) {
    let count = app.checked_count();
    let noun = if count == 1 { "folder" } else { "folders" };
    let body = vec![
        Line::from(format!(
            "Move {count} {noun} ({}) to Trash?",
            size::format_bytes(app.checked_bytes())
        )),
        Line::from(""),
        Line::from("They stay in the system Trash until you empty it."),
        Line::from("Nothing is permanently deleted."),
        Line::from(""),
        Line::from("enter or y confirm     n cancel"),
    ];
    render_modal(frame, " Confirm ", body);
}

fn draw_working(frame: &mut Frame, app: &App) {
    let body = vec![
        Line::from(app.notice.clone()),
        Line::from(""),
        Line::from("Moving selected caches to Trash…"),
    ];
    render_modal(frame, " Cleaning ", body);
}

fn draw_summary(frame: &mut Frame, summary: &Summary) {
    let noun = if summary.moved.len() == 1 {
        "folder"
    } else {
        "folders"
    };
    let mut lines = vec![
        Line::from(format!(
            "Moved {} {noun} ({}) to Trash.",
            summary.moved.len(),
            size::format_bytes(summary.bytes)
        )),
        Line::from(""),
    ];

    match clean::log_file() {
        Ok(path) if !summary.moved.is_empty() => {
            lines.push(Line::from("Restore from Trash. Moved paths:"));
            lines.push(Line::from(path.display().to_string()));
        }
        _ => {
            lines.push(Line::from("Restore anything moved from the system Trash."));
        }
    }

    if let Some(warning) = &summary.log_warning {
        lines.push(Line::from(""));
        lines.push(Line::from(warning.clone()));
    }

    if !summary.errors.is_empty() {
        lines.push(Line::from(""));
        lines.push(Line::from(format!("Failed ({}):", summary.errors.len())));
        for error in summary.errors.iter().take(5) {
            lines.push(Line::from(format!(
                "{} — {}",
                error.path.display(),
                error.message
            )));
        }
        if summary.errors.len() > 5 {
            lines.push(Line::from(format!(
                "… and {} more",
                summary.errors.len() - 5
            )));
        }
    }

    lines.push(Line::from(""));
    lines.push(Line::from("enter or q close"));
    render_modal(frame, " Done ", lines);
}

fn render_modal(frame: &mut Frame, title: &str, lines: Vec<Line<'static>>) {
    let height = (lines.len() as u16 + 2).clamp(6, frame.area().height.saturating_sub(2));
    let width = 72.min(frame.area().width.saturating_sub(2)).max(20);
    let area = centered(frame.area(), width, height);
    frame.render_widget(Clear, area);
    let paragraph = Paragraph::new(lines).wrap(Wrap { trim: false }).block(
        Block::default()
            .borders(Borders::ALL)
            .title(title.to_string()),
    );
    frame.render_widget(paragraph, area);
}

fn centered(area: Rect, width: u16, height: u16) -> Rect {
    let width = width.min(area.width);
    let height = height.min(area.height);
    let x = area.x + (area.width.saturating_sub(width)) / 2;
    let y = area.y + (area.height.saturating_sub(height)) / 2;
    Rect::new(x, y, width, height)
}
