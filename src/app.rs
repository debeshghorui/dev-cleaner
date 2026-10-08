use std::io::{self, stdout};
use std::path::{Path, PathBuf};
use std::time::Duration;

use crossterm::event::{self, Event, KeyCode, KeyEvent, KeyEventKind, KeyModifiers};
use crossterm::execute;
use crossterm::terminal::{
    disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen,
};
use ratatui::backend::CrosstermBackend;
use ratatui::Terminal;
use rayon::prelude::*;

use crate::clean::{self, TrashError};
use crate::scan::{self, Find, Scan};
use crate::size;
use crate::ui;

pub struct RowItem {
    pub find: Find,
    pub size: u64,
    pub checked: bool,
}

#[derive(Clone)]
pub struct Summary {
    pub moved: Vec<PathBuf>,
    pub bytes: u64,
    pub errors: Vec<TrashError>,
    pub log_warning: Option<String>,
}

#[derive(Clone)]
pub enum Phase {
    Browse,
    Confirm,
    Working,
    Summary(Summary),
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum PhaseKind {
    Browse,
    Confirm,
    Working,
    Summary,
}

pub enum Signal {
    Continue,
    Quit,
    Clean,
}

pub struct App {
    pub root: PathBuf,
    pub rows: Vec<RowItem>,
    pub warnings: Vec<String>,
    pub cursor: usize,
    pub filter: String,
    pub filter_mode: bool,
    pub show_warnings: bool,
    pub phase: Phase,
    pub notice: String,
    pub table_state: ratatui::widgets::TableState,
}

impl App {
    fn from_scan(scan: Scan, sizes: Vec<u64>) -> Self {
        let rows = scan
            .finds
            .into_iter()
            .zip(sizes)
            .map(|(find, size)| RowItem {
                find,
                size,
                checked: false,
            })
            .collect();
        let mut app = Self {
            root: scan.root,
            rows,
            warnings: scan.warnings,
            cursor: 0,
            filter: String::new(),
            filter_mode: false,
            show_warnings: false,
            phase: Phase::Browse,
            notice: String::new(),
            table_state: ratatui::widgets::TableState::default(),
        };
        app.sync_table();
        app
    }

    pub fn visible(&self) -> Vec<usize> {
        let query = self.filter.to_ascii_lowercase();
        self.rows
            .iter()
            .enumerate()
            .filter_map(|(index, row)| {
                if query.is_empty() || row_matches(row, &query) {
                    Some(index)
                } else {
                    None
                }
            })
            .collect()
    }

    pub fn checked_count(&self) -> usize {
        self.rows.iter().filter(|row| row.checked).count()
    }

    pub fn checked_bytes(&self) -> u64 {
        self.rows
            .iter()
            .filter(|row| row.checked)
            .map(|row| row.size)
            .sum()
    }

    pub fn total_bytes(&self) -> u64 {
        self.rows.iter().map(|row| row.size).sum()
    }

    fn phase_kind(&self) -> PhaseKind {
        match self.phase {
            Phase::Browse => PhaseKind::Browse,
            Phase::Confirm => PhaseKind::Confirm,
            Phase::Working => PhaseKind::Working,
            Phase::Summary(_) => PhaseKind::Summary,
        }
    }

    fn handle_key(&mut self, key: KeyEvent) -> Signal {
        if key.modifiers.contains(KeyModifiers::CONTROL) && key.code == KeyCode::Char('c') {
            return Signal::Quit;
        }

        match self.phase_kind() {
            PhaseKind::Working => Signal::Continue,
            PhaseKind::Summary => match key.code {
                KeyCode::Enter | KeyCode::Esc | KeyCode::Char('q') => Signal::Quit,
                _ => Signal::Continue,
            },
            PhaseKind::Confirm => match key.code {
                KeyCode::Enter | KeyCode::Char('y') | KeyCode::Char('Y') => Signal::Clean,
                KeyCode::Esc | KeyCode::Char('n') | KeyCode::Char('N') | KeyCode::Char('q') => {
                    self.phase = Phase::Browse;
                    self.notice.clear();
                    Signal::Continue
                }
                _ => Signal::Continue,
            },
            PhaseKind::Browse if self.filter_mode => self.handle_filter(key),
            PhaseKind::Browse => self.handle_browse(key),
        }
    }

    fn handle_browse(&mut self, key: KeyEvent) -> Signal {
        match key.code {
            KeyCode::Char('q') | KeyCode::Esc => Signal::Quit,
            KeyCode::Up | KeyCode::Char('k') => {
                self.move_cursor(-1);
                Signal::Continue
            }
            KeyCode::Down | KeyCode::Char('j') => {
                self.move_cursor(1);
                Signal::Continue
            }
            KeyCode::PageUp => {
                self.move_cursor(-10);
                Signal::Continue
            }
            KeyCode::PageDown => {
                self.move_cursor(10);
                Signal::Continue
            }
            KeyCode::Char(' ') => {
                self.toggle_current();
                Signal::Continue
            }
            KeyCode::Char('a') | KeyCode::Char('A') => {
                self.toggle_all_visible();
                Signal::Continue
            }
            KeyCode::Char('/') => {
                self.filter_mode = true;
                self.notice.clear();
                Signal::Continue
            }
            KeyCode::Char('w') | KeyCode::Char('W') => {
                self.show_warnings = !self.show_warnings && !self.warnings.is_empty();
                Signal::Continue
            }
            KeyCode::Enter => self.begin_confirm(),
            _ => Signal::Continue,
        }
    }

    fn handle_filter(&mut self, key: KeyEvent) -> Signal {
        match key.code {
            KeyCode::Esc => {
                self.filter.clear();
                self.filter_mode = false;
                self.sync_table();
                Signal::Continue
            }
            KeyCode::Enter => {
                self.filter_mode = false;
                self.sync_table();
                Signal::Continue
            }
            KeyCode::Backspace => {
                self.filter.pop();
                self.sync_table();
                Signal::Continue
            }
            KeyCode::Char(ch) if !key.modifiers.contains(KeyModifiers::CONTROL) => {
                self.filter.push(ch);
                self.sync_table();
                Signal::Continue
            }
            _ => Signal::Continue,
        }
    }

    fn begin_confirm(&mut self) -> Signal {
        if self.checked_count() == 0 {
            self.notice = "Select at least one folder with space, or press a for all.".to_string();
            return Signal::Continue;
        }
        self.notice.clear();
        self.phase = Phase::Confirm;
        Signal::Continue
    }

    fn move_cursor(&mut self, delta: isize) {
        let len = self.visible().len();
        if len == 0 {
            return;
        }
        let next = self.cursor as isize + delta;
        self.cursor = next.clamp(0, len as isize - 1) as usize;
        self.sync_table();
    }

    fn toggle_current(&mut self) {
        let Some(&index) = self.visible().get(self.cursor) else {
            return;
        };
        self.rows[index].checked = !self.rows[index].checked;
        self.notice.clear();
    }

    fn toggle_all_visible(&mut self) {
        let visible = self.visible();
        if visible.is_empty() {
            return;
        }
        let check = visible.iter().any(|&index| !self.rows[index].checked);
        for index in visible {
            self.rows[index].checked = check;
        }
        self.notice.clear();
    }

    fn sync_table(&mut self) {
        let len = self.visible().len();
        if len == 0 {
            self.cursor = 0;
            self.table_state.select(None);
            return;
        }
        if self.cursor >= len {
            self.cursor = len - 1;
        }
        self.table_state.select(Some(self.cursor));
    }
}

fn row_matches(row: &RowItem, query: &str) -> bool {
    row.find.name.to_ascii_lowercase().contains(query)
        || row.find.kind.label().contains(query)
        || row
            .find
            .path
            .to_string_lossy()
            .to_ascii_lowercase()
            .contains(query)
}

pub fn run(root: PathBuf) -> io::Result<()> {
    enable_raw_mode()?;
    let _guard = TerminalGuard;
    execute!(stdout(), EnterAlternateScreen, crossterm::cursor::Hide)?;
    let mut terminal = Terminal::new(CrosstermBackend::new(stdout()))?;
    session(&mut terminal, &root)
}

fn session(terminal: &mut Terminal<CrosstermBackend<io::Stdout>>, root: &Path) -> io::Result<()> {
    terminal.draw(|frame| ui::splash(frame, root, "Scanning directories…"))?;
    let scanned = scan::scan(root);
    let measuring = format!("Measuring {} cache folders…", scanned.finds.len());
    terminal.draw(|frame| ui::splash(frame, root, &measuring))?;
    let sizes: Vec<u64> = scanned
        .finds
        .par_iter()
        .map(|find| size::folder_size(&find.path))
        .collect();
    let mut app = App::from_scan(scanned, sizes);

    loop {
        terminal.draw(|frame| ui::draw(frame, &mut app))?;
        if !event::poll(Duration::from_millis(200))? {
            continue;
        }
        let Event::Key(key) = event::read()? else {
            continue;
        };
        if key.kind == KeyEventKind::Release {
            continue;
        }
        match app.handle_key(key) {
            Signal::Continue => {}
            Signal::Quit => break,
            Signal::Clean => clean_selected(terminal, &mut app)?,
        }
    }
    Ok(())
}

fn clean_selected(
    terminal: &mut Terminal<CrosstermBackend<io::Stdout>>,
    app: &mut App,
) -> io::Result<()> {
    let jobs: Vec<(PathBuf, u64)> = app
        .rows
        .iter()
        .filter(|row| row.checked)
        .map(|row| (row.find.path.clone(), row.size))
        .collect();
    let total = jobs.len();
    let mut moved = Vec::new();
    let mut bytes = 0u64;
    let mut errors = Vec::new();
    let mut log_warning = None;

    app.phase = Phase::Working;
    for (index, (path, size)) in jobs.into_iter().enumerate() {
        app.notice = format!("Moving {}/{}  {}", index + 1, total, path.display());
        terminal.draw(|frame| ui::draw(frame, app))?;
        match clean::trash_one(&path, &app.root, size) {
            Ok(trashed) => {
                bytes += trashed.bytes;
                if log_warning.is_none() {
                    log_warning = trashed.log_warning;
                }
                moved.push(trashed.path);
            }
            Err(err) => errors.push(err),
        }
    }

    app.notice.clear();
    app.phase = Phase::Summary(Summary {
        moved,
        bytes,
        errors,
        log_warning,
    });
    Ok(())
}

struct TerminalGuard;

impl Drop for TerminalGuard {
    fn drop(&mut self) {
        let _ = disable_raw_mode();
        let _ = execute!(stdout(), LeaveAlternateScreen, crossterm::cursor::Show);
    }
}
