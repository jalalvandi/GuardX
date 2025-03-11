use crate::filesystem::FileSystem;
use anyhow::Result;
use crossterm::event::{self, Event, KeyCode, KeyEventKind};
use ratatui::{
    prelude::*,
    widgets::{Block, Borders, List, ListItem, Paragraph, Tabs, ListState, BorderType, Table, Row, Cell, Gauge},
    style::{Style, Color, Modifier},
};
use std::time::{Duration, Instant};

pub struct App {
    fs: FileSystem,
    selected_dir: ListState,
    key_input: String,
    mode: Mode,
    status: String,
    should_quit: bool,
    last_processed: Instant,
    success_timer: Option<Instant>,
    progress: f64,
    in_progress: bool,
    preview_content: Option<String>,
}

#[derive(PartialEq)]
pub enum Mode {
    Navigate,
    EnterKey,
    CreateFolder,
    Preview,
}

impl App {
    pub fn new() -> Result<Self> {
        let fs = FileSystem::new()?;
        let mut selected_dir = ListState::default();
        selected_dir.select(Some(0));
        Ok(App {
            fs,
            selected_dir,
            key_input: String::new(),
            mode: Mode::Navigate,
            status: "Welcome! Press 'k' to enter key, 'n' to create folder, 'p' to preview".to_string(),
            should_quit: false,
            last_processed: Instant::now(),
            success_timer: None,
            progress: 0.0,
            in_progress: false,
            preview_content: None,
        })
    }
}

pub fn run_app<B: Backend>(terminal: &mut Terminal<B>, mut app: App) -> Result<()> {
    const DEBOUNCE_DURATION: Duration = Duration::from_millis(150);

    loop {
        terminal.draw(|f| ui(f, &mut app))?;

        if let Some(start) = app.success_timer {
            if start.elapsed() > Duration::from_secs(2) {
                app.success_timer = None;
            }
        }

        if app.in_progress {
            app.progress += 0.1;
            if app.progress >= 1.0 {
                app.progress = 0.0;
                app.in_progress = false;
            }
        }

        if event::poll(Duration::from_millis(50))? {
            if let Event::Key(key) = event::read()? {
                if key.kind == KeyEventKind::Press {
                    let now = Instant::now();
                    if now.duration_since(app.last_processed) >= DEBOUNCE_DURATION {
                        app.last_processed = now;
                        match app.mode {
                            Mode::Navigate => match key.code {
                                KeyCode::Char('q') => app.should_quit = true,
                                KeyCode::Up => {
                                    if let Some(selected) = app.selected_dir.selected() {
                                        app.selected_dir.select(Some(selected.saturating_sub(1)));
                                    }
                                }
                                KeyCode::Down => {
                                    if let Some(selected) = app.selected_dir.selected() {
                                        if selected < app.fs.dirs.len() - 1 {
                                            app.selected_dir.select(Some(selected + 1));
                                        }
                                    }
                                }
                                KeyCode::Char('e') => {
                                    if app.key_input.is_empty() {
                                        app.status = "[!] Please enter a key first (press 'k')".to_string();
                                    } else if let Some(selected) = app.selected_dir.selected() {
                                        app.in_progress = true;
                                        app.progress = 0.0;
                                        if let Err(e) = app.fs.encrypt_dir(selected, &app.key_input) {
                                            app.status = format!("[X] Encryption failed: {}", e);
                                            app.in_progress = false;
                                        } else {
                                            app.status = "[OK] Folder encrypted successfully!".to_string();
                                            app.success_timer = Some(Instant::now());
                                            app.in_progress = false;
                                        }
                                    }
                                }
                                KeyCode::Char('d') => {
                                    if app.key_input.is_empty() {
                                        app.status = "[!] Please enter a key first (press 'k')".to_string();
                                    } else if let Some(selected) = app.selected_dir.selected() {
                                        app.in_progress = true;
                                        app.progress = 0.0;
                                        if let Err(e) = app.fs.decrypt_dir(selected, &app.key_input) {
                                            app.status = format!("[X] Decryption failed: {}", e);
                                            app.in_progress = false;
                                        } else {
                                            app.status = "[OK] Folder decrypted successfully!".to_string();
                                            app.success_timer = Some(Instant::now());
                                            app.in_progress = false;
                                        }
                                    }
                                }
                                KeyCode::Char('k') => {
                                    app.mode = Mode::EnterKey;
                                    app.key_input.clear();
                                    app.status = "[Key] Enter your encryption key: ".to_string();
                                }
                                KeyCode::Char('n') => {
                                    app.mode = Mode::CreateFolder;
                                    app.key_input.clear();
                                    app.status = "[Folder] Enter new folder name: ".to_string();
                                }
                                KeyCode::Char('p') => {
                                    if let Some(selected) = app.selected_dir.selected() {
                                        let files = app.fs.get_files(selected);
                                        if let Some(first_file) = files.first() {
                                            let path = app.fs.dirs[selected].join(first_file);
                                            app.preview_content = std::fs::read_to_string(&path)
                                                .ok()
                                                .or(Some("Unable to read file".to_string()));
                                            app.mode = Mode::Preview;
                                        } else {
                                            app.status = "[!] No files to preview".to_string();
                                        }
                                    }
                                }
                                _ => {}
                            },
                            Mode::EnterKey => match key.code {
                                KeyCode::Enter => {
                                    app.mode = Mode::Navigate;
                                    app.status = format!("[OK] Key '{}' set successfully!", app.key_input);
                                    app.success_timer = Some(Instant::now());
                                }
                                KeyCode::Char(c) => {
                                    app.key_input.push(c);
                                    app.status = format!("[Key] Enter your encryption key: {}", app.key_input);
                                }
                                KeyCode::Backspace => {
                                    app.key_input.pop();
                                    app.status = format!("[Key] Enter your encryption key: {}", app.key_input);
                                }
                                KeyCode::Esc => app.mode = Mode::Navigate,
                                KeyCode::Char('q') => app.should_quit = true,
                                _ => {}
                            },
                            Mode::CreateFolder => match key.code {
                                KeyCode::Enter => {
                                    if let Err(e) = app.fs.create_folder(&app.key_input) {
                                        app.status = format!("[X] Folder creation failed: {}", e);
                                    } else {
                                        app.status = format!("[OK] Folder '{}' created!", app.key_input);
                                        app.success_timer = Some(Instant::now());
                                    }
                                    app.key_input.clear();
                                    app.mode = Mode::Navigate;
                                }
                                KeyCode::Char(c) => {
                                    app.key_input.push(c);
                                    app.status = format!("[Folder] Enter new folder name: {}", app.key_input);
                                }
                                KeyCode::Backspace => {
                                    app.key_input.pop();
                                    app.status = format!("[Folder] Enter new folder name: {}", app.key_input);
                                }
                                KeyCode::Esc => app.mode = Mode::Navigate,
                                KeyCode::Char('q') => app.should_quit = true,
                                _ => {}
                            },
                            Mode::Preview => match key.code {
                                KeyCode::Esc | KeyCode::Char('q') => {
                                    app.mode = Mode::Navigate;
                                    app.preview_content = None;
                                    app.status = "Back to navigation".to_string();
                                }
                                _ => {}
                            },
                        }
                    }
                }
            }
        }

        if app.should_quit {
            return Ok(());
        }
    }
}

fn ui(f: &mut Frame, app: &mut App) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3),
            Constraint::Min(0),
            Constraint::Length(5),
        ])
        .split(f.size());

    let titles = vec!["[Folders]", "[Key]", "[New Folder]", "[Preview]"];
    let tabs = Tabs::new(titles)
        .block(Block::default()
            .borders(Borders::ALL)
            .border_type(BorderType::Rounded)
            .title("Secure Folder")
            .title_style(Style::default().fg(Color::LightCyan).add_modifier(Modifier::BOLD))
            .border_style(Style::default().fg(Color::Gray)))
        .select(match app.mode {
            Mode::Navigate => 0,
            Mode::EnterKey => 1,
            Mode::CreateFolder => 2,
            Mode::Preview => 3,
        })
        .style(Style::default().fg(Color::White))
        .highlight_style(Style::default().fg(Color::LightYellow).add_modifier(Modifier::BOLD));
    f.render_widget(tabs, chunks[0]);

    let main_chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(30), Constraint::Percentage(70)])
        .split(chunks[1]);

    let dirs: Vec<ListItem> = app
        .fs
        .dirs
        .iter()
        .map(|d| ListItem::new(d.display().to_string()).style(Style::default().fg(Color::LightGreen)))
        .collect();
    let dirs_list = List::new(dirs)
        .block(Block::default()
            .borders(Borders::ALL)
            .border_type(BorderType::Rounded)
            .title("Folders")
            .title_style(Style::default().fg(Color::LightCyan))
            .border_style(Style::default().fg(Color::Gray)))
        .highlight_style(Style::default().fg(Color::LightYellow).bg(Color::DarkGray).add_modifier(Modifier::BOLD))
        .highlight_symbol("> ");
    f.render_stateful_widget(dirs_list, main_chunks[0], &mut app.selected_dir);

    if app.mode == Mode::Preview {
        let preview_text = app.preview_content.as_ref().unwrap_or(&"No content".to_string()).clone();
        let preview_widget = Paragraph::new(preview_text)
            .style(Style::default().fg(Color::White))
            .block(Block::default()
                .borders(Borders::ALL)
                .border_type(BorderType::Rounded)
                .title("File Preview (Esc to exit)")
                .title_style(Style::default().fg(Color::LightCyan))
                .border_style(Style::default().fg(Color::Gray)));
        f.render_widget(preview_widget, main_chunks[1]);
    } else {
        let files = if let Some(selected) = app.selected_dir.selected() {
            app.fs.get_files(selected)
        } else {
            vec![]
        };
        let rows: Vec<Row> = files.iter().map(|f| Row::new(vec![Cell::from(f.as_str())])).collect();
        let files_table = Table::new(rows, &[Constraint::Percentage(100)])
            .block(Block::default()
                .borders(Borders::ALL)
                .border_type(BorderType::Rounded)
                .title("Files")
                .title_style(Style::default().fg(Color::LightCyan))
                .border_style(Style::default().fg(Color::Gray)))
            .style(Style::default().fg(Color::White));
        f.render_widget(files_table, main_chunks[1]);
    }

    let status_chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(3), Constraint::Length(2)])
        .split(chunks[2]);

    let input_style = if app.status.starts_with("[OK]") {
        let elapsed = app.success_timer.map(|t| t.elapsed().as_secs_f32()).unwrap_or(0.0);
        if elapsed < 1.0 { Style::default().fg(Color::LightGreen).add_modifier(Modifier::BOLD) }
        else { Style::default().fg(Color::Green) }
    } else if app.status.starts_with("[X]") {
        Style::default().fg(Color::Red).add_modifier(Modifier::BOLD)
    } else if app.status.starts_with("[!]") {
        Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD)
    } else {
        Style::default().fg(Color::White)
    };
    let input_widget = Paragraph::new(app.status.clone())
        .style(input_style)
        .block(Block::default()
            .borders(Borders::ALL)
            .border_type(BorderType::Rounded)
            .border_style(Style::default().fg(Color::Gray))
            .title("Status/Input | q: Quit | Up/Down: Navigate | k: Key | n: New Folder | e: Encrypt | d: Decrypt | p: Preview")
            .title_style(Style::default().fg(Color::LightCyan)));
    f.render_widget(input_widget, status_chunks[0]);

    let progress_widget = if app.in_progress {
        Gauge::default()
            .block(Block::default()
                .borders(Borders::ALL)
                .border_type(BorderType::Rounded)
                .border_style(Style::default().fg(Color::Gray))
                .title("Progress")
                .title_style(Style::default().fg(Color::LightCyan)))
            .gauge_style(Style::default().fg(Color::LightBlue))
            .percent((app.progress * 100.0) as u16)
    } else {
        Gauge::default()
            .block(Block::default()
                .borders(Borders::ALL)
                .border_type(BorderType::Rounded)
                .border_style(Style::default().fg(Color::Gray)))
            .percent(0)
    };
    f.render_widget(progress_widget, status_chunks[1]);
}