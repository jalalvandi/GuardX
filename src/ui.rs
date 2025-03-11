use crate::filesystem::FileSystem;
use crate::crypto::{encrypt_file, decrypt_file};
use anyhow::Result;
use crossterm::event::{self, Event, KeyCode, KeyEventKind, MouseEventKind};
use ratatui::{
    prelude::*,
    widgets::{Block, Borders, List, ListItem, Paragraph, Tabs, ListState, BorderType, Table, Row, Cell, Gauge, Clear},
    style::{Style, Color, Modifier},
};
use std::time::{Duration, Instant};
use std::fs;

pub struct App {
    fs: FileSystem,
    selected_dir: ListState,
    selected_file: ListState,
    current_files: Vec<String>,
    key_input: String,
    mode: Mode,
    status: String,
    should_quit: bool,
    last_processed: Instant,
    success_timer: Option<Instant>,
    progress: f64,
    in_progress: bool,
    preview_content: Option<String>,
    history: Vec<(String, Instant, bool)>,
    settings: Settings,
    animation_step: usize,
    info_mode: bool,
}

#[derive(PartialEq)]
pub enum Mode {
    NavigateFolders,
    NavigateFiles,
    EnterKey,
    CreateFolder,
    Preview,
    Settings,
    ConfirmDelete,
}

pub struct Settings {
    theme: Theme,
    key_length: usize,
}

#[derive(PartialEq)]
pub enum Theme {
    Dark,
    Light,
}

impl App {
    pub fn new() -> Result<Self> {
        let fs = FileSystem::new()?;
        let mut selected_dir = ListState::default();
        selected_dir.select(Some(0));
        let mut selected_file = ListState::default();
        selected_file.select(None);
        let current_files = if !fs.dirs.is_empty() { fs.get_files(0) } else { vec![] };
        Ok(App {
            fs,
            selected_dir,
            selected_file,
            current_files,
            key_input: String::new(),
            mode: Mode::NavigateFolders,
            status: "Welcome!".to_string(),
            should_quit: false,
            last_processed: Instant::now(),
            success_timer: None,
            progress: 0.0,
            in_progress: false,
            preview_content: None,
            history: Vec::new(),
            settings: Settings { theme: Theme::Dark, key_length: 32 },
            animation_step: 0,
            info_mode: false,
        })
    }

    fn get_theme_styles(&self) -> (Color, Color, Color, Color) {
        match self.settings.theme {
            Theme::Dark => (Color::Black, Color::White, Color::LightCyan, Color::Gray),
            Theme::Light => (Color::White, Color::Black, Color::Cyan, Color::Gray),
        }
    }

    fn update_current_files(&mut self) {
        if let Some(selected) = self.selected_dir.selected() {
            self.current_files = self.fs.get_files(selected);
            self.selected_file.select(if self.current_files.is_empty() { None } else { Some(0) });
        } else {
            self.current_files.clear();
            self.selected_file.select(None);
        }
    }
}

pub fn run_app<B: Backend>(terminal: &mut Terminal<B>, mut app: App) -> Result<()> {
    const DEBOUNCE_DURATION: Duration = Duration::from_millis(150);

    loop {
        terminal.draw(|f| ui(f, &mut app))?;

        if let Some(start) = app.success_timer {
            if start.elapsed() > Duration::from_secs(2) {
                app.success_timer = None;
                app.status = "Ready".to_string();
            } else {
                app.animation_step = (start.elapsed().as_millis() / 200 % 2) as usize;
            }
        }

        if app.in_progress {
            app.progress += 0.05;
            if app.progress >= 1.0 {
                app.progress = 0.0;
                app.in_progress = false;
            }
        }

        if event::poll(Duration::from_millis(50))? {
            match event::read()? {
                Event::Key(key) if key.kind == KeyEventKind::Press => {
                    let now = Instant::now();
                    if now.duration_since(app.last_processed) >= DEBOUNCE_DURATION {
                        app.last_processed = now;
                        match app.mode {
                            Mode::NavigateFolders => match key.code {
                                KeyCode::Char('q') => app.should_quit = true,
                                KeyCode::Up => {
                                    if let Some(selected) = app.selected_dir.selected() {
                                        app.selected_dir.select(Some(selected.saturating_sub(1)));
                                        app.update_current_files();
                                    }
                                }
                                KeyCode::Down => {
                                    let len = app.fs.dirs.len();
                                    if len > 0 {
                                        app.selected_dir.select(Some((app.selected_dir.selected().unwrap_or(0) + 1).min(len - 1)));
                                        app.update_current_files();
                                    }
                                }
                                KeyCode::Right => { // تغییر به Right برای ورود به فایل‌ها
                                    if !app.current_files.is_empty() {
                                        app.mode = Mode::NavigateFiles;
                                        app.status = "Navigating files (Left to return)".to_string();
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
                                            app.history.push((format!("Encrypt failed: {}", e), Instant::now(), false));
                                            app.in_progress = false;
                                        } else {
                                            app.status = "[OK] Folder encrypted successfully!".to_string();
                                            app.history.push(("Encrypted folder".to_string(), Instant::now(), true));
                                            app.success_timer = Some(Instant::now());
                                            app.in_progress = false;
                                            app.fs.mark_encrypted(selected, true);
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
                                            app.history.push((format!("Decrypt failed: {}", e), Instant::now(), false));
                                            app.in_progress = false;
                                        } else {
                                            app.status = "[OK] Folder decrypted successfully!".to_string();
                                            app.history.push(("Decrypted folder".to_string(), Instant::now(), true));
                                            app.success_timer = Some(Instant::now());
                                            app.in_progress = false;
                                            app.fs.mark_encrypted(selected, false);
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
                                            app.preview_content = fs::read_to_string(&path).ok().or(Some("Unable to read file".to_string()));
                                            app.mode = Mode::Preview;
                                        } else {
                                            app.status = "[!] No files to preview".to_string();
                                        }
                                    }
                                }
                                KeyCode::Char('t') => app.mode = Mode::Settings,
                                KeyCode::Char('r') => app.mode = Mode::ConfirmDelete,
                                KeyCode::Char('i') => app.info_mode = !app.info_mode,
                                KeyCode::Char('l') => {
                                    if let Ok(key) = fs::read_to_string("saved_key.enc") {
                                        app.key_input = key.trim().to_string();
                                        app.status = "[OK] Key loaded successfully!".to_string();
                                        app.success_timer = Some(Instant::now());
                                        app.history.push(("Loaded key".to_string(), Instant::now(), true));
                                    } else {
                                        app.status = "[X] No saved key found".to_string();
                                    }
                                }
                                KeyCode::Char('v') => {
                                    if !app.key_input.is_empty() {
                                        fs::write("saved_key.enc", &app.key_input)?;
                                        app.status = "[OK] Key saved successfully!".to_string();
                                        app.success_timer = Some(Instant::now());
                                        app.history.push(("Saved key".to_string(), Instant::now(), true));
                                    } else {
                                        app.status = "[!] No key to save".to_string();
                                    }
                                }
                                _ => {}
                            },
                            Mode::NavigateFiles => match key.code {
                                KeyCode::Up => {
                                    if let Some(selected) = app.selected_file.selected() {
                                        app.selected_file.select(Some(selected.saturating_sub(1)));
                                    }
                                }
                                KeyCode::Down => {
                                    let len = app.current_files.len();
                                    if len > 0 {
                                        app.selected_file.select(Some((app.selected_file.selected().unwrap_or(0) + 1).min(len - 1)));
                                    }
                                }
                                KeyCode::Left => { // تغییر به Left برای بازگشت به پوشه‌ها
                                    app.mode = Mode::NavigateFolders;
                                    app.status = "Back to folders".to_string();
                                    app.selected_file.select(None);
                                }
                                KeyCode::Char('q') => app.should_quit = true,
                                KeyCode::Char('p') => {
                                    if let Some(dir_idx) = app.selected_dir.selected() {
                                        if let Some(file_idx) = app.selected_file.selected() {
                                            let path = app.fs.dirs[dir_idx].join(&app.current_files[file_idx]);
                                            app.preview_content = fs::read_to_string(&path).ok().or(Some("Unable to read file".to_string()));
                                            app.mode = Mode::Preview;
                                        }
                                    }
                                }
                                _ => {}
                            },
                            Mode::EnterKey => match key.code {
                                KeyCode::Enter => {
                                    app.mode = Mode::NavigateFolders;
                                    app.status = format!("[OK] Key '{}' set successfully!", app.key_input);
                                    app.success_timer = Some(Instant::now());
                                    app.history.push(("Set key".to_string(), Instant::now(), true));
                                }
                                KeyCode::Char(c) => {
                                    app.key_input.push(c);
                                    app.status = format!("[Key] Enter your encryption key: {}", app.key_input);
                                }
                                KeyCode::Backspace => {
                                    app.key_input.pop();
                                    app.status = format!("[Key] Enter your encryption key: {}", app.key_input);
                                }
                                KeyCode::Esc => app.mode = Mode::NavigateFolders,
                                KeyCode::Char('q') => app.should_quit = true,
                                _ => {}
                            },
                            Mode::CreateFolder => match key.code {
                                KeyCode::Enter => {
                                    if let Err(e) = app.fs.create_folder(&app.key_input) {
                                        app.status = format!("[X] Folder creation failed: {}", e);
                                        app.history.push((format!("Create folder failed: {}", e), Instant::now(), false));
                                    } else {
                                        app.status = format!("[OK] Folder '{}' created!", app.key_input);
                                        app.history.push(("Created folder".to_string(), Instant::now(), true));
                                        app.success_timer = Some(Instant::now());
                                        app.update_current_files();
                                    }
                                    app.key_input.clear();
                                    app.mode = Mode::NavigateFolders;
                                }
                                KeyCode::Char(c) => {
                                    app.key_input.push(c);
                                    app.status = format!("[Folder] Enter new folder name: {}", app.key_input);
                                }
                                KeyCode::Backspace => {
                                    app.key_input.pop();
                                    app.status = format!("[Folder] Enter new folder name: {}", app.key_input);
                                }
                                KeyCode::Esc => app.mode = Mode::NavigateFolders,
                                KeyCode::Char('q') => app.should_quit = true,
                                _ => {}
                            },
                            Mode::Preview => match key.code {
                                KeyCode::Esc | KeyCode::Char('q') => {
                                    app.mode = if app.selected_file.selected().is_some() { Mode::NavigateFiles } else { Mode::NavigateFolders };
                                    app.preview_content = None;
                                    app.status = "Back to navigation".to_string();
                                }
                                _ => {}
                            },
                            Mode::Settings => match key.code {
                                KeyCode::Char('1') => app.settings.theme = Theme::Dark,
                                KeyCode::Char('2') => app.settings.theme = Theme::Light,
                                KeyCode::Char('3') => app.settings.key_length = 16,
                                KeyCode::Char('4') => app.settings.key_length = 32,
                                KeyCode::Esc => app.mode = Mode::NavigateFolders,
                                KeyCode::Char('q') => app.should_quit = true,
                                _ => {}
                            },
                            Mode::ConfirmDelete => match key.code {
                                KeyCode::Char('y') => {
                                    if let Some(selected) = app.selected_dir.selected() {
                                        let path = app.fs.dirs[selected].clone();
                                        if let Err(e) = fs::remove_dir_all(&path) {
                                            app.status = format!("[X] Delete failed: {}", e);
                                            app.history.push((format!("Delete failed: {}", e), Instant::now(), false));
                                        } else {
                                            app.fs.dirs.remove(selected);
                                            app.status = "[OK] Folder deleted successfully!".to_string();
                                            app.history.push(("Deleted folder".to_string(), Instant::now(), true));
                                            app.success_timer = Some(Instant::now());
                                            if app.fs.dirs.is_empty() {
                                                app.selected_dir.select(None);
                                            } else {
                                                app.selected_dir.select(Some(selected.min(app.fs.dirs.len() - 1)));
                                            }
                                            app.update_current_files();
                                        }
                                    }
                                    app.mode = Mode::NavigateFolders;
                                }
                                KeyCode::Char('n') | KeyCode::Esc => app.mode = Mode::NavigateFolders,
                                _ => {}
                            },
                        }
                    }
                }
                Event::Mouse(mouse) => {
                    if let MouseEventKind::Down(_) = mouse.kind {
                        let y = mouse.row;
                        if y < 3 { // تب‌ها
                            let x = mouse.column;
                            if x < 10 { app.mode = Mode::NavigateFolders; }
                            else if x < 20 { app.mode = Mode::EnterKey; app.key_input.clear(); app.status = "[Key] Enter your encryption key: ".to_string(); }
                            else if x < 35 { app.mode = Mode::CreateFolder; app.key_input.clear(); app.status = "[Folder] Enter new folder name: ".to_string(); }
                            else { app.mode = Mode::Preview; }
                        } else if y >= 4 && y < main_area_height(&app) + 4 { // لیست پوشه‌ها یا فایل‌ها
                            if app.mode == Mode::NavigateFolders {
                                let new_idx = (y - 4) as usize;
                                if new_idx < app.fs.dirs.len() {
                                    app.selected_dir.select(Some(new_idx));
                                    app.update_current_files();
                                }
                            } else if app.mode == Mode::NavigateFiles {
                                let new_idx = (y - 4) as usize;
                                if new_idx < app.current_files.len() {
                                    app.selected_file.select(Some(new_idx));
                                }
                            }
                        }
                    }
                }
                _ => {}
            }
        }

        if app.should_quit {
            return Ok(());
        }
    }
}

fn main_area_height(app: &App) -> u16 {
    app.fs.dirs.len().max(app.current_files.len()) as u16
}

fn ui(f: &mut Frame, app: &mut App) {
    let (bg, fg, accent, border) = app.get_theme_styles();
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
            .title_style(Style::default().fg(accent).add_modifier(Modifier::BOLD))
            .border_style(Style::default().fg(border)))
        .select(match app.mode {
            Mode::NavigateFolders => 0,
            Mode::EnterKey => 1,
            Mode::CreateFolder => 2,
            Mode::Preview => 3,
            _ => 0,
        })
        .style(Style::default().fg(fg).bg(bg))
        .highlight_style(Style::default().fg(Color::LightYellow).add_modifier(Modifier::BOLD));
    f.render_widget(tabs, chunks[0]);

    let main_chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(30), Constraint::Percentage(70)])
        .split(chunks[1]);

    let dirs: Vec<ListItem> = app.fs.dirs.iter().enumerate()
        .map(|(i, d)| {
            let mark = if app.fs.is_encrypted(i) { "[E]" } else { "" };
            ListItem::new(format!("{} {}", mark, d.display())).style(Style::default().fg(Color::LightGreen))
        })
        .collect();
    let dirs_list = List::new(dirs)
        .block(Block::default()
            .borders(Borders::ALL)
            .border_type(BorderType::Rounded)
            .title("Folders")
            .title_style(Style::default().fg(accent))
            .border_style(Style::default().fg(if app.mode == Mode::NavigateFolders { accent } else { border })))
        .highlight_style(Style::default().fg(Color::LightYellow).bg(Color::DarkGray).add_modifier(Modifier::BOLD))
        .highlight_symbol("> ");
    f.render_stateful_widget(dirs_list, main_chunks[0], &mut app.selected_dir);

    if app.mode == Mode::Preview {
        let preview_text = app.preview_content.as_ref().unwrap_or(&"No content".to_string()).clone();
        let preview_widget = Paragraph::new(preview_text)
            .style(Style::default().fg(fg))
            .block(Block::default()
                .borders(Borders::ALL)
                .border_type(BorderType::Rounded)
                .title("File Preview (Esc to exit)")
                .title_style(Style::default().fg(accent))
                .border_style(Style::default().fg(border)));
        f.render_widget(preview_widget, main_chunks[1]);
    } else if app.info_mode && app.mode != Mode::NavigateFiles {
        let total_dirs = app.fs.dirs.len();
        let encrypted_dirs = app.fs.dirs.iter().enumerate().filter(|(i, _)| app.fs.is_encrypted(*i)).count();
        let total_files: usize = app.fs.dirs.iter().map(|d| fs::read_dir(d).map(|dir| dir.count()).unwrap_or(0)).sum();
        let info_text = format!(
            "Total Folders: {}\nEncrypted Folders: {}\nTotal Files: {}",
            total_dirs, encrypted_dirs, total_files
        );
        let info_widget = Paragraph::new(info_text)
            .style(Style::default().fg(fg))
            .block(Block::default()
                .borders(Borders::ALL)
                .border_type(BorderType::Rounded)
                .title("Dashboard (i to exit)")
                .title_style(Style::default().fg(accent))
                .border_style(Style::default().fg(border)));
        f.render_widget(info_widget, main_chunks[1]);
    } else {
        let rows: Vec<Row> = app.current_files.iter().enumerate().map(|(i, f)| {
            let style = if Some(i) == app.selected_file.selected() && app.mode == Mode::NavigateFiles {
                Style::default().fg(Color::LightYellow).bg(Color::DarkGray).add_modifier(Modifier::BOLD)
            } else {
                Style::default().fg(fg)
            };
            Row::new(vec![Cell::from(f.as_str())]).style(style)
        }).collect();
        let files_table = Table::new(rows, &[Constraint::Percentage(100)])
            .block(Block::default()
                .borders(Borders::ALL)
                .border_type(BorderType::Rounded)
                .title("Files")
                .title_style(Style::default().fg(accent))
                .border_style(Style::default().fg(if app.mode == Mode::NavigateFiles { accent } else { border })));
        f.render_widget(files_table, main_chunks[1]);
    }

    let status_chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(3), Constraint::Length(2)])
        .split(chunks[2]);

    let input_style = if app.status.starts_with("[OK]") {
        let elapsed = app.success_timer.map(|t| t.elapsed().as_secs_f32()).unwrap_or(0.0);
        if elapsed < 1.0 && app.animation_step % 2 == 0 { Style::default().fg(Color::LightGreen).add_modifier(Modifier::BOLD) }
        else { Style::default().fg(Color::Green) }
    } else if app.status.starts_with("[X]") {
        Style::default().fg(Color::Red).add_modifier(Modifier::BOLD)
    } else if app.status.starts_with("[!]") {
        Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD)
    } else {
        Style::default().fg(fg)
    };
    let input_widget = Paragraph::new(app.status.clone())
        .style(input_style)
        .block(Block::default()
            .borders(Borders::ALL)
            .border_type(BorderType::Rounded)
            .border_style(Style::default().fg(border))
            .title("Status/Input\nq: Quit | k: Key | n: New | e: Encrypt | d: Decrypt\np: Preview | t: Settings | r: Remove | i: Info\nl: Load | v: Save | Right/Left: Switch")
            .title_style(Style::default().fg(accent)));
    f.render_widget(input_widget, status_chunks[0]);

    let progress_widget = if app.in_progress {
        Gauge::default()
            .block(Block::default()
                .borders(Borders::ALL)
                .border_type(BorderType::Rounded)
                .border_style(Style::default().fg(border))
                .title("Progress")
                .title_style(Style::default().fg(accent)))
            .gauge_style(Style::default().fg(Color::LightBlue))
            .percent((app.progress * 100.0) as u16)
    } else {
        Gauge::default()
            .block(Block::default()
                .borders(Borders::ALL)
                .border_type(BorderType::Rounded)
                .border_style(Style::default().fg(border)))
            .percent(0)
    };
    f.render_widget(progress_widget, status_chunks[1]);

    if app.mode == Mode::Settings {
        let settings_area = Rect {
            x: f.size().width / 4,
            y: f.size().height / 4,
            width: f.size().width / 2,
            height: f.size().height / 2,
        };
        f.render_widget(Clear, settings_area);
        let settings_text = format!(
            "Settings\n1: Dark Theme\n2: Light Theme\n3: Key Length 16\n4: Key Length 32\nEsc: Exit\nCurrent: {} Theme, Key Length {}",
            if app.settings.theme == Theme::Dark { "Dark" } else { "Light" },
            app.settings.key_length
        );
        let settings_widget = Paragraph::new(settings_text)
            .style(Style::default().fg(fg))
            .block(Block::default()
                .borders(Borders::ALL)
                .border_type(BorderType::Rounded)
                .title("Settings")
                .title_style(Style::default().fg(accent))
                .border_style(Style::default().fg(border)));
        f.render_widget(settings_widget, settings_area);
    }

    if app.mode == Mode::ConfirmDelete {
        let confirm_area = Rect {
            x: f.size().width / 3,
            y: f.size().height / 3,
            width: f.size().width / 3,
            height: 5,
        };
        f.render_widget(Clear, confirm_area);
        let confirm_widget = Paragraph::new("Delete folder? (y/n)")
            .style(Style::default().fg(fg))
            .block(Block::default()
                .borders(Borders::ALL)
                .border_type(BorderType::Rounded)
                .title("Confirm")
                .title_style(Style::default().fg(accent))
                .border_style(Style::default().fg(border)));
        f.render_widget(confirm_widget, confirm_area);
    }

    if app.info_mode {
        let history_area = Rect {
            x: f.size().width - 30,
            y: 4,
            width: 30,
            height: app.history.len() as u16 + 2,
        };
        let history_items: Vec<ListItem> = app.history.iter().map(|(msg, time, success)| {
            let time_str = format!("{:?}", time.elapsed().as_secs());
            ListItem::new(format!("{} - {}s", msg, time_str))
                .style(Style::default().fg(if *success { Color::Green } else { Color::Red }))
        }).collect();
        let history_widget = List::new(history_items)
            .block(Block::default()
                .borders(Borders::ALL)
                .border_type(BorderType::Rounded)
                .title("History")
                .title_style(Style::default().fg(accent))
                .border_style(Style::default().fg(border)));
        f.render_widget(history_widget, history_area);
    }
}