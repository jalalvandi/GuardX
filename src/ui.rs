use crate::filesystem::FileSystem;
use crate::crypto::{encrypt_file, decrypt_file};
use anyhow::Result;
use crossterm::event::{self, Event, KeyCode, KeyEventKind, MouseEventKind};
use ratatui::{
    prelude::*,
    widgets::{Block, Borders, List, ListItem, Paragraph, ListState, BorderType, Table, Row, Cell, Clear, Gauge},
    style::{Style, Color, Modifier},
};
use std::time::{Duration, Instant};
use std::fs;
use std::fs::Metadata;
use std::time::SystemTime;
use chrono::DateTime as ChronoDateTime;
use chrono::Utc;

pub struct App {
    fs: FileSystem,
    selected_dir: ListState,
    selected_file: ListState,
    current_files: Vec<(String, Metadata, bool)>,
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
    ConfirmDeleteFolder,
    ConfirmDeleteFile,
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
        let current_files = if !fs.dirs.is_empty() { Self::load_files(&fs, 0).unwrap_or_default() } else { vec![] };
        Ok(App {
            fs,
            selected_dir,
            selected_file,
            current_files,
            key_input: String::new(),
            mode: Mode::NavigateFolders,
            status: "Welcome to SecureFolder!".to_string(),
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
            Theme::Dark => (Color::Rgb(20, 20, 30), Color::White, Color::Cyan, Color::Gray),
            Theme::Light => (Color::Gray, Color::Black, Color::Blue, Color::DarkGray),
        }
    }

    fn load_files(fs: &FileSystem, dir_idx: usize) -> Result<Vec<(String, Metadata, bool)>> {
        if dir_idx >= fs.dirs.len() { return Ok(vec![]); }
        let dir = &fs.dirs[dir_idx];
        let mut files = Vec::new();
        match fs::read_dir(dir) {
            Ok(entries) => {
                for entry in entries {
                    match entry {
                        Ok(entry) => {
                            let path = entry.path();
                            match entry.metadata() {
                                Ok(metadata) => {
                                    if path.is_file() {
                                        let encrypted = path.extension().map_or(false, |ext| ext == "enc");
                                        files.push((entry.file_name().to_string_lossy().to_string(), metadata, encrypted));
                                    }
                                }
                                Err(_) => {} // خطا رو نادیده می‌گیریم و توی UI مدیریت می‌کنیم
                            }
                        }
                        Err(_) => {} // خطا رو نادیده می‌گیریم
                    }
                }
                Ok(files)
            }
            Err(_) => Ok(vec![]) // به جای ارور، لیست خالی برمی‌گردونیم
        }
    }

    fn update_current_files(&mut self) {
        if let Some(selected) = self.selected_dir.selected() {
            match Self::load_files(&self.fs, selected) {
                Ok(files) => {
                    self.current_files = files;
                    self.selected_file.select(if self.current_files.is_empty() { None } else { Some(0) });
                    if self.current_files.is_empty() && self.fs.get_files(selected).is_err() {
                        self.status = "[!] Access Denied to this folder".to_string();
                    }
                }
                Err(e) => {
                    self.current_files.clear();
                    self.selected_file.select(None);
                    self.status = format!("[!] Access Denied: {}", e);
                }
            }
        } else {
            self.current_files.clear();
            self.selected_file.select(None);
        }
    }
}

pub fn run_app<B: Backend>(terminal: &mut Terminal<B>, mut app: App) -> Result<()> {
    const DEBOUNCE_DURATION: Duration = Duration::from_millis(150);

    loop {
        if let Err(e) = terminal.draw(|f| ui(f, &mut app)) {
            eprintln!("Draw error: {}", e);
            return Err(anyhow::Error::from(e));
        }

        if let Some(start) = app.success_timer {
            if start.elapsed() > Duration::from_secs(2) {
                app.success_timer = None;
                app.status = "Ready".to_string();
            } else {
                app.animation_step = (start.elapsed().as_millis() / 150 % 4) as usize;
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
                                KeyCode::Right => {
                                    if !app.current_files.is_empty() {
                                        app.mode = Mode::NavigateFiles;
                                        app.status = "Navigating files (← to return)".to_string();
                                    }
                                }
                                KeyCode::Char('e') => {
                                    if app.key_input.is_empty() {
                                        app.status = "[!] Enter a key first (k)".to_string();
                                    } else if let Some(selected) = app.selected_dir.selected() {
                                        app.in_progress = true;
                                        app.progress = 0.0;
                                        if let Err(e) = app.fs.encrypt_dir(selected, &app.key_input) {
                                            app.status = format!("[X] Encryption failed: {}", e);
                                            app.history.push((format!("Encrypt failed: {}", e), Instant::now(), false));
                                            app.in_progress = false;
                                        } else {
                                            app.status = "[OK] Folder encrypted!".to_string();
                                            app.history.push(("Encrypted folder".to_string(), Instant::now(), true));
                                            app.success_timer = Some(Instant::now());
                                            app.in_progress = false;
                                            app.fs.mark_encrypted(selected, true);
                                            app.update_current_files();
                                        }
                                    }
                                }
                                KeyCode::Char('d') => {
                                    if app.key_input.is_empty() {
                                        app.status = "[!] Enter a key first (k)".to_string();
                                    } else if let Some(selected) = app.selected_dir.selected() {
                                        app.in_progress = true;
                                        app.progress = 0.0;
                                        if let Err(e) = app.fs.decrypt_dir(selected, &app.key_input) {
                                            app.status = format!("[X] Decryption failed: {}", e);
                                            app.history.push((format!("Decrypt failed: {}", e), Instant::now(), false));
                                            app.in_progress = false;
                                        } else {
                                            app.status = "[OK] Folder decrypted!".to_string();
                                            app.history.push(("Decrypted folder".to_string(), Instant::now(), true));
                                            app.success_timer = Some(Instant::now());
                                            app.in_progress = false;
                                            app.fs.mark_encrypted(selected, false);
                                            app.update_current_files();
                                        }
                                    }
                                }
                                KeyCode::Char('k') => {
                                    app.mode = Mode::EnterKey;
                                    app.key_input.clear();
                                    app.status = "[Key] Enter encryption key: ".to_string();
                                }
                                KeyCode::Char('n') => {
                                    app.mode = Mode::CreateFolder;
                                    app.key_input.clear();
                                    app.status = "[Folder] Enter new folder name: ".to_string();
                                }
                                KeyCode::Char('p') => {
                                    if let Some(selected) = app.selected_dir.selected() {
                                        match app.fs.get_files(selected) {
                                            Ok(files) => {
                                                if let Some(first_file) = files.first() {
                                                    let path = app.fs.dirs[selected].join(first_file);
                                                    app.preview_content = fs::read_to_string(&path).ok().or(Some("Unable to read file".to_string()));
                                                    app.mode = Mode::Preview;
                                                } else {
                                                    app.status = "[!] No files to preview".to_string();
                                                }
                                            }
                                            Err(_) => {
                                                app.status = "[!] Access Denied to this folder".to_string();
                                            }
                                        }
                                    }
                                }
                                KeyCode::Char('t') => app.mode = Mode::Settings,
                                KeyCode::Char('r') => app.mode = Mode::ConfirmDeleteFolder,
                                KeyCode::Char('i') => app.info_mode = !app.info_mode,
                                KeyCode::Char('l') => {
                                    if let Ok(key) = fs::read_to_string("saved_key.enc") {
                                        app.key_input = key.trim().to_string();
                                        app.status = "[OK] Key loaded!".to_string();
                                        app.success_timer = Some(Instant::now());
                                        app.history.push(("Loaded key".to_string(), Instant::now(), true));
                                    } else {
                                        app.status = "[X] No saved key found".to_string();
                                    }
                                }
                                KeyCode::Char('v') => {
                                    if !app.key_input.is_empty() {
                                        fs::write("saved_key.enc", &app.key_input)?;
                                        app.status = "[OK] Key saved!".to_string();
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
                                KeyCode::Left => {
                                    app.mode = Mode::NavigateFolders;
                                    app.status = "Back to folders".to_string();
                                    app.selected_file.select(None);
                                }
                                KeyCode::Char('q') => app.should_quit = true,
                                KeyCode::Char('p') => {
                                    if let Some(dir_idx) = app.selected_dir.selected() {
                                        if let Some(file_idx) = app.selected_file.selected() {
                                            let path = app.fs.dirs[dir_idx].join(&app.current_files[file_idx].0);
                                            app.preview_content = fs::read_to_string(&path).ok().or(Some("Unable to read file".to_string()));
                                            app.mode = Mode::Preview;
                                        }
                                    }
                                }
                                KeyCode::Char('r') => app.mode = Mode::ConfirmDeleteFile,
                                _ => {}
                            },
                            Mode::EnterKey => match key.code {
                                KeyCode::Enter => {
                                    app.mode = Mode::NavigateFolders;
                                    app.status = format!("[OK] Key '{}' set!", app.key_input);
                                    app.success_timer = Some(Instant::now());
                                    app.history.push(("Set key".to_string(), Instant::now(), true));
                                }
                                KeyCode::Char(c) => {
                                    app.key_input.push(c);
                                    app.status = format!("[Key] Enter encryption key: {}", app.key_input);
                                }
                                KeyCode::Backspace => {
                                    app.key_input.pop();
                                    app.status = format!("[Key] Enter encryption key: {}", app.key_input);
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
                            Mode::ConfirmDeleteFolder => match key.code {
                                KeyCode::Char('y') => {
                                    if let Some(selected) = app.selected_dir.selected() {
                                        let path = app.fs.dirs[selected].clone();
                                        if let Err(e) = fs::remove_dir_all(&path) {
                                            app.status = format!("[X] Delete failed: {}", e);
                                            app.history.push((format!("Delete failed: {}", e), Instant::now(), false));
                                        } else {
                                            app.fs.dirs.remove(selected);
                                            app.status = "[OK] Folder deleted!".to_string();
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
                            Mode::ConfirmDeleteFile => match key.code {
                                KeyCode::Char('y') => {
                                    if let Some(dir_idx) = app.selected_dir.selected() {
                                        if let Some(file_idx) = app.selected_file.selected() {
                                            let path = app.fs.dirs[dir_idx].join(&app.current_files[file_idx].0);
                                            if let Err(e) = fs::remove_file(&path) {
                                                app.status = format!("[X] File delete failed: {}", e);
                                                app.history.push((format!("File delete failed: {}", e), Instant::now(), false));
                                            } else {
                                                app.status = "[OK] File deleted!".to_string();
                                                app.history.push(("Deleted file".to_string(), Instant::now(), true));
                                                app.success_timer = Some(Instant::now());
                                                app.update_current_files();
                                            }
                                        }
                                    }
                                    app.mode = Mode::NavigateFiles;
                                }
                                KeyCode::Char('n') | KeyCode::Esc => app.mode = Mode::NavigateFiles,
                                _ => {}
                            },
                        }
                    }
                }
                Event::Mouse(mouse) => {
                    if let MouseEventKind::Down(_) = mouse.kind {
                        let y = mouse.row;
                        if y >= 4 && y < main_area_height(&app) + 4 {
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
    app.fs.dirs.len().max(app.current_files.len()) as u16 + 2
}

fn ui(f: &mut Frame, app: &mut App) {
    let (bg, fg, accent, border) = app.get_theme_styles();

    f.render_widget(Paragraph::new("").style(Style::default().bg(bg)), f.size());

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3),   // نوار وضعیت
            Constraint::Length(2),   // نوار پیشرفت
            Constraint::Min(10),     // بخش اصلی
            Constraint::Length(5),   // راهنما
        ])
        .split(f.size());

    // نوار وضعیت
    let status_style = if app.status.starts_with("[OK]") {
        let anim_colors = [Color::Green, Color::LightGreen, Color::Green, Color::LightGreen];
        Style::default().fg(anim_colors[app.animation_step]).add_modifier(Modifier::BOLD)
    } else if app.status.starts_with("[X]") {
        Style::default().fg(Color::Red).add_modifier(Modifier::BOLD | Modifier::ITALIC)
    } else if app.status.starts_with("[!]") {
        Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD)
    } else {
        Style::default().fg(fg)
    };
    let status_block = Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Double)
        .border_style(Style::default().fg(accent))
        .title(" 🔒 SecureFolder ")
        .title_style(Style::default().fg(accent).add_modifier(Modifier::BOLD));
    let status_widget = Paragraph::new(app.status.clone())
        .style(status_style)
        .block(status_block);
    f.render_widget(status_widget, chunks[0]);

    // نوار پیشرفت
    if app.in_progress {
        let progress_widget = Gauge::default()
            .gauge_style(Style::default().fg(Color::Cyan).bg(bg))
            .percent((app.progress * 100.0) as u16)
            .label("Processing...");
        f.render_widget(progress_widget, chunks[1]);
    }

    // بخش اصلی
    let main_chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(30), Constraint::Percentage(70)])
        .split(chunks[2]);

    // لیست پوشه‌ها
    let dirs: Vec<ListItem> = app.fs.dirs.iter().enumerate()
        .map(|(i, d)| {
            let mark = if app.fs.is_encrypted(i) { "🔐 " } else { "📁 " };
            ListItem::new(format!("{}{}", mark, d.display()))
                .style(Style::default().fg(if app.fs.is_encrypted(i) { Color::LightCyan } else { Color::LightGreen }))
        })
        .collect();
    let dirs_list = List::new(dirs)
        .block(Block::default()
            .borders(Borders::ALL)
            .border_type(BorderType::Rounded)
            .title(" Folders ")
            .title_alignment(Alignment::Center)
            .title_style(Style::default().fg(accent).add_modifier(Modifier::BOLD))
            .border_style(Style::default().fg(if app.mode == Mode::NavigateFolders { accent } else { border })))
        .highlight_style(Style::default().fg(Color::White).bg(Color::Rgb(50, 50, 70)).add_modifier(Modifier::BOLD))
        .highlight_symbol("➤ ");
    f.render_stateful_widget(dirs_list, main_chunks[0], &mut app.selected_dir);

    // بخش سمت راست
    if app.mode == Mode::Preview {
        let preview_text = app.preview_content.as_ref().unwrap_or(&"No content".to_string()).clone();
        let preview_widget = Paragraph::new(preview_text)
            .style(Style::default().fg(fg))
            .block(Block::default()
                .borders(Borders::ALL)
                .border_type(BorderType::Thick)
                .title(" 📄 Preview (Esc to exit) ")
                .title_alignment(Alignment::Center)
                .title_style(Style::default().fg(accent).add_modifier(Modifier::BOLD))
                .border_style(Style::default().fg(border).bg(Color::Rgb(30, 30, 40))));
        f.render_widget(preview_widget, main_chunks[1]);
    } else if app.info_mode && app.mode != Mode::NavigateFiles {
        let total_dirs = app.fs.dirs.len();
        let encrypted_dirs = app.fs.dirs.iter().enumerate().filter(|(i, _)| app.fs.is_encrypted(*i)).count();
        let total_files: usize = app.fs.dirs.iter().map(|d| fs::read_dir(d).map(|dir| dir.count()).unwrap_or(0)).sum();
        let info_text = format!(
            "📂 Total Folders: {}\n🔐 Encrypted: {}\n📄 Total Files: {}",
            total_dirs, encrypted_dirs, total_files
        );
        let info_widget = Paragraph::new(info_text)
            .style(Style::default().fg(fg))
            .block(Block::default()
                .borders(Borders::ALL)
                .border_type(BorderType::Rounded)
                .title(" Dashboard (i to toggle) ")
                .title_alignment(Alignment::Center)
                .title_style(Style::default().fg(accent).add_modifier(Modifier::BOLD))
                .border_style(Style::default().fg(border)));
        f.render_widget(info_widget, main_chunks[1]);
    } else {
        let rows: Vec<Row> = if app.current_files.is_empty() && app.selected_dir.selected().map_or(false, |idx| app.fs.get_files(idx).is_err()) {
            vec![Row::new(vec![Cell::from("⚠ No access to this folder")])
                .style(Style::default().fg(Color::Yellow).add_modifier(Modifier::ITALIC))]
        } else {
            app.current_files.iter().enumerate().map(|(i, (name, meta, encrypted))| {
                let size = format!("{} KB", meta.len() / 1024);
                let created = meta.created()
                    .map(|t| t.duration_since(SystemTime::UNIX_EPOCH).unwrap().as_secs())
                    .map(|s| ChronoDateTime::<Utc>::from_timestamp(s as i64, 0).unwrap().format("%Y-%m-%d").to_string())
                    .unwrap_or("N/A".to_string());
                let status = if *encrypted { "🔒" } else { "✔" };
                let style = if Some(i) == app.selected_file.selected() && app.mode == Mode::NavigateFiles {
                    Style::default().fg(Color::White).bg(Color::Rgb(50, 50, 70)).add_modifier(Modifier::BOLD)
                } else {
                    Style::default().fg(fg)
                };
                Row::new(vec![
                    Cell::from(name.as_str()),
                    Cell::from(size),
                    Cell::from(created),
                    Cell::from(status),
                ]).style(style).height(1)
            }).collect()
        };
        let files_table = Table::new(rows, &[
            Constraint::Percentage(40),
            Constraint::Percentage(20),
            Constraint::Percentage(30),
            Constraint::Percentage(10),
        ])
        .header(Row::new(vec!["Name", "Size", "Created", "Status"])
            .style(Style::default().fg(accent).add_modifier(Modifier::BOLD))
            .bottom_margin(1))
        .block(Block::default()
            .borders(Borders::ALL)
            .border_type(BorderType::Rounded)
            .title(" Files ")
            .title_alignment(Alignment::Center)
            .title_style(Style::default().fg(accent).add_modifier(Modifier::BOLD))
            .border_style(Style::default().fg(if app.mode == Mode::NavigateFiles { accent } else { border })));
        f.render_widget(files_table, main_chunks[1]);
    }

    // نوار راهنما
    let help_text = vec![
        Line::from(vec![
            Span::styled("q", Style::default().fg(accent).add_modifier(Modifier::BOLD)),
            Span::raw(": Quit | "),
            Span::styled("k", Style::default().fg(accent).add_modifier(Modifier::BOLD)),
            Span::raw(": Key | "),
            Span::styled("n", Style::default().fg(accent).add_modifier(Modifier::BOLD)),
            Span::raw(": New Folder"),
        ]),
        Line::from(vec![
            Span::styled("e", Style::default().fg(accent).add_modifier(Modifier::BOLD)),
            Span::raw(": Encrypt | "),
            Span::styled("d", Style::default().fg(accent).add_modifier(Modifier::BOLD)),
            Span::raw(": Decrypt | "),
            Span::styled("p", Style::default().fg(accent).add_modifier(Modifier::BOLD)),
            Span::raw(": Preview"),
        ]),
        Line::from(vec![
            Span::styled("t", Style::default().fg(accent).add_modifier(Modifier::BOLD)),
            Span::raw(": Settings | "),
            Span::styled("r", Style::default().fg(accent).add_modifier(Modifier::BOLD)),
            Span::raw(": Remove | "),
            Span::styled("i", Style::default().fg(accent).add_modifier(Modifier::BOLD)),
            Span::raw(": Info"),
        ]),
    ];
    let help_widget = Paragraph::new(help_text)
        .style(Style::default().fg(fg))
        .block(Block::default()
            .borders(Borders::ALL)
            .border_type(BorderType::Double)
            .title(" Controls ")
            .title_alignment(Alignment::Center)
            .title_style(Style::default().fg(accent).add_modifier(Modifier::BOLD))
            .border_style(Style::default().fg(border)));
    f.render_widget(help_widget, chunks[3]);

    // پنجره تنظیمات
    if app.mode == Mode::Settings {
        let settings_area = centered_rect(50, 50, f.size());
        f.render_widget(Clear, settings_area);
        f.render_widget(Block::default().borders(Borders::ALL).border_style(Style::default().fg(Color::Gray).bg(Color::Rgb(20, 20, 20))), settings_area);
        let settings_text = vec![
            Line::from("⚙ Settings"),
            Line::from(vec![
                Span::styled("1", Style::default().fg(accent).add_modifier(Modifier::BOLD)),
                Span::raw(": Dark Theme")
            ]),
            Line::from(vec![
                Span::styled("2", Style::default().fg(accent).add_modifier(Modifier::BOLD)),
                Span::raw(": Light Theme")
            ]),
            Line::from(vec![
                Span::styled("3", Style::default().fg(accent).add_modifier(Modifier::BOLD)),
                Span::raw(": Key Length 16")
            ]),
            Line::from(vec![
                Span::styled("4", Style::default().fg(accent).add_modifier(Modifier::BOLD)),
                Span::raw(": Key Length 32")
            ]),
            Line::from(vec![
                Span::styled("Esc", Style::default().fg(accent).add_modifier(Modifier::BOLD)),
                Span::raw(": Exit")
            ]),
            Line::from(format!(
                "Current: {} Theme, Key Length {}",
                if app.settings.theme == Theme::Dark { "Dark" } else { "Light" },
                app.settings.key_length
            )),
        ];
        let settings_widget = Paragraph::new(settings_text)
            .style(Style::default().fg(fg))
            .block(Block::default()
                .borders(Borders::ALL)
                .border_type(BorderType::Thick)
                .title(" Settings ")
                .title_alignment(Alignment::Center)
                .title_style(Style::default().fg(accent).add_modifier(Modifier::BOLD))
                .border_style(Style::default().fg(accent)));
        f.render_widget(settings_widget, settings_area);
    }

    // پنجره تأیید حذف پوشه
    if app.mode == Mode::ConfirmDeleteFolder {
        let confirm_area = centered_rect(30, 5, f.size());
        f.render_widget(Clear, confirm_area);
        f.render_widget(Block::default().borders(Borders::ALL).border_style(Style::default().fg(Color::Gray).bg(Color::Rgb(20, 20, 20))), confirm_area);
        let confirm_widget = Paragraph::new("Delete folder? [y/n]")
            .style(Style::default().fg(fg))
            .block(Block::default()
                .borders(Borders::ALL)
                .border_type(BorderType::Rounded)
                .title(" Confirm ")
                .title_alignment(Alignment::Center)
                .title_style(Style::default().fg(Color::Red).add_modifier(Modifier::BOLD))
                .border_style(Style::default().fg(Color::Red)));
        f.render_widget(confirm_widget, confirm_area);
    }

    // پنجره تأیید حذف فایل
    if app.mode == Mode::ConfirmDeleteFile {
        let confirm_area = centered_rect(30, 5, f.size());
        f.render_widget(Clear, confirm_area);
        f.render_widget(Block::default().borders(Borders::ALL).border_style(Style::default().fg(Color::Gray).bg(Color::Rgb(20, 20, 20))), confirm_area);
        let confirm_widget = Paragraph::new("Delete file? [y/n]")
            .style(Style::default().fg(fg))
            .block(Block::default()
                .borders(Borders::ALL)
                .border_type(BorderType::Rounded)
                .title(" Confirm ")
                .title_alignment(Alignment::Center)
                .title_style(Style::default().fg(Color::Red).add_modifier(Modifier::BOLD))
                .border_style(Style::default().fg(Color::Red)));
        f.render_widget(confirm_widget, confirm_area);
    }

    // تاریخچه
    if app.info_mode {
        let history_area = Rect {
            x: f.size().width - 35,
            y: 4,
            width: 35,
            height: (app.history.len() + 2).min(10) as u16,
        };
        let history_items: Vec<ListItem> = app.history.iter().rev().take(8)
            .map(|(msg, time, success)| {
                let time_str = format!("{:?}s", time.elapsed().as_secs());
                ListItem::new(format!("{} ({})", msg, time_str))
                    .style(Style::default().fg(if *success { Color::Green } else { Color::Red }))
            }).collect();
        let history_widget = List::new(history_items)
            .block(Block::default()
                .borders(Borders::ALL)
                .border_type(BorderType::Rounded)
                .title(" History ")
                .title_alignment(Alignment::Center)
                .title_style(Style::default().fg(accent).add_modifier(Modifier::BOLD))
                .border_style(Style::default().fg(border)))
            .highlight_style(Style::default().fg(Color::White).bg(Color::DarkGray));
        f.render_widget(history_widget, history_area);
    }
}

fn centered_rect(percent_x: u16, percent_y: u16, r: Rect) -> Rect {
    let popup_layout = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Percentage((100 - percent_y) / 2),
            Constraint::Percentage(percent_y),
            Constraint::Percentage((100 - percent_y) / 2),
        ])
        .split(r);
    Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage((100 - percent_x) / 2),
            Constraint::Percentage(percent_x),
            Constraint::Percentage((100 - percent_x) / 2),
        ])
        .split(popup_layout[1])[1]
}