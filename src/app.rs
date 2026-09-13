use std::path::{Path, PathBuf};
use std::process::Command;

use crossterm::event::{KeyCode, KeyEvent, KeyEventKind, KeyModifiers, MouseEvent, MouseEventKind};
use ratatui::{
    Frame,
    layout::{Alignment, Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, BorderType, Borders, Clear, Paragraph},
};

use crate::chat::ChatScreen;
use crate::code::CodeScreen;
use crate::editor::EditorScreen;
use crate::explorer::{FileExplorerAction, FileExplorerScreen};
use crate::home::{HomeOption, HomeScreen};
use crate::settings::{SettingsAction, SettingsScreen};
use crate::todo::{TodoInputMode, TodoScreen};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CurrentScreen {
    Home,
    Chat,
    EditorPrompt,
    FileExplorer,
    Editor,
    Code,
    Todo,
    Settings,
}

pub struct App {
    pub current_screen: CurrentScreen,
    pub home: HomeScreen,
    pub chat: ChatScreen,
    pub editor: EditorScreen,
    pub explorer: Option<FileExplorerScreen>,
    pub code: CodeScreen,
    pub todo: TodoScreen,
    pub settings: SettingsScreen,
    pub should_quit: bool,
    pub manual_folder_input: String,
    pub is_manual_folder_input: bool,
}

impl App {
    pub fn new() -> Self {
        Self {
            current_screen: CurrentScreen::Home,
            home: HomeScreen::new(),
            chat: ChatScreen::new(),
            editor: EditorScreen::new(),
            explorer: None,
            code: CodeScreen::new(),
            todo: TodoScreen::new(),
            settings: SettingsScreen::new(),
            should_quit: false,
            manual_folder_input: String::new(),
            is_manual_folder_input: false,
        }
    }

    pub fn open_project_folder(&mut self, folder: PathBuf) {
        let explorer = FileExplorerScreen::new(folder.clone());
        self.explorer = Some(explorer);
        self.current_screen = CurrentScreen::FileExplorer;

        // Auto-launch the output runner window if an entrypoint or Python project file is detected
        if let Some(entrypoint) = find_python_entrypoint(&folder) {
            self.spawn_runner_for_file(&entrypoint);
        }
    }

    pub fn spawn_runner_for_file(&mut self, path: &Path) {
        // If runner is already active for this exact file, keep running
        if self.editor.is_runner_active()
            && self.editor.current_runner_path.as_deref() == Some(path)
        {
            return;
        }

        self.cleanup_runner();

        let current_exe = std::env::current_exe().unwrap_or_else(|_| PathBuf::from("tomy"));
        let path_str = path.to_string_lossy().to_string();
        let file_name = path
            .file_name()
            .map(|n| n.to_string_lossy().to_string())
            .unwrap_or_else(|| "Script".to_string());
        let title_arg = format!("--title=Tomy Output Runner - {}", file_name);

        let child = Command::new("gnome-terminal")
            .args([
                &title_arg,
                "--wait",
                "--",
                current_exe.to_str().unwrap_or("tomy"),
                "runner",
                &path_str,
            ])
            .spawn()
            .or_else(|_| {
                Command::new("gnome-terminal")
                    .args([
                        &title_arg,
                        "--",
                        current_exe.to_str().unwrap_or("tomy"),
                        "runner",
                        &path_str,
                    ])
                    .spawn()
            })
            .or_else(|_| {
                Command::new("x-terminal-emulator")
                    .args([
                        "-e",
                        &format!("{} runner {}", current_exe.display(), path_str),
                    ])
                    .spawn()
            })
            .ok();

        self.editor.runner_child = child;
        self.editor.current_runner_path = Some(path.to_path_buf());
    }

    pub fn select_folder_with_zenity_or_prompt(&mut self) {
        // Try launching zenity --file-selection --directory
        let output = Command::new("zenity")
            .args([
                "--file-selection",
                "--directory",
                "--title=Select Project Root Folder",
            ])
            .output();

        if let Ok(res) = output
            && res.status.success()
        {
            let path_str = String::from_utf8_lossy(&res.stdout).trim().to_string();
            if !path_str.is_empty() {
                let path = PathBuf::from(path_str);
                if path.is_dir() {
                    self.open_project_folder(path);
                    return;
                }
            }
        }

        // If zenity was cancelled or unavailable, fall back to in-terminal prompt
        self.is_manual_folder_input = true;
        self.manual_folder_input = std::env::current_dir()
            .map(|p| p.to_string_lossy().to_string())
            .unwrap_or_default();
    }

    pub fn launch_scratch_and_runner(&mut self) {
        let temp_file = std::env::temp_dir().join("tomy_scratch.py");
        if !temp_file.exists() {
            let template = r#"# Tomy Scratch Script Runner
# Edit this code, press Ctrl+S or Ctrl+R to execute.
# Output will display live in this terminal window.

import time

def main():
    print("⚡ Tomy On-Device Execution Harness")
    print(f"Timestamp: {time.strftime('%Y-%m-%d %H:%M:%S')}")
    
    squares = [x ** 2 for x in range(1, 11)]
    print(f"Computed squares: {squares}")
    print("Execution complete.")

if __name__ == '__main__':
    main()
"#;
            let _ = std::fs::write(&temp_file, template);
        }

        self.editor.open_file(&temp_file);
        self.editor.opened_from_explorer = false;
        self.spawn_runner_for_file(&temp_file);
        self.current_screen = CurrentScreen::Editor;
    }

    pub fn handle_key(&mut self, key: KeyEvent) {
        // Only process key press events (avoid double triggers on platforms with key release)
        if key.kind != KeyEventKind::Press {
            return;
        }

        // Global shortcut: Ctrl+C quits immediately (except when in Runner)
        if key.modifiers.contains(KeyModifiers::CONTROL) && key.code == KeyCode::Char('c') {
            self.cleanup_runner();
            self.should_quit = true;
            return;
        }

        match self.current_screen {
            CurrentScreen::Home => match key.code {
                KeyCode::Up | KeyCode::Char('k') => self.home.previous(),
                KeyCode::Down | KeyCode::Char('j') => self.home.next(),
                KeyCode::Enter => match self.home.selected_option() {
                    HomeOption::Chat => self.current_screen = CurrentScreen::Chat,
                    HomeOption::Editor => self.current_screen = CurrentScreen::EditorPrompt,
                    HomeOption::Code => self.current_screen = CurrentScreen::Code,
                    HomeOption::Todo => self.current_screen = CurrentScreen::Todo,
                    HomeOption::Settings => self.current_screen = CurrentScreen::Settings,
                },
                KeyCode::Char('q') => {
                    self.cleanup_runner();
                    self.should_quit = true;
                }
                _ => {}
            },

            CurrentScreen::EditorPrompt => {
                if self.is_manual_folder_input {
                    match key.code {
                        KeyCode::Enter => {
                            let trimmed = self.manual_folder_input.trim();
                            if !trimmed.is_empty() {
                                let path = PathBuf::from(trimmed);
                                if path.is_dir() {
                                    self.open_project_folder(path);
                                    self.is_manual_folder_input = false;
                                    self.manual_folder_input.clear();
                                    return;
                                }
                            }
                            self.is_manual_folder_input = false;
                            self.manual_folder_input.clear();
                        }
                        KeyCode::Esc => {
                            self.is_manual_folder_input = false;
                            self.manual_folder_input.clear();
                        }
                        KeyCode::Backspace => {
                            self.manual_folder_input.pop();
                        }
                        KeyCode::Char(c) => {
                            self.manual_folder_input.push(c);
                        }
                        _ => {}
                    }
                } else {
                    match key.code {
                        KeyCode::Char('1') | KeyCode::Char('f') | KeyCode::Char('F') => {
                            self.select_folder_with_zenity_or_prompt();
                        }
                        KeyCode::Char('2') | KeyCode::Char('s') | KeyCode::Char('S') => {
                            self.launch_scratch_and_runner();
                        }
                        KeyCode::Esc => {
                            self.current_screen = CurrentScreen::Home;
                        }
                        _ => {}
                    }
                }
            }

            CurrentScreen::FileExplorer => {
                if let Some(ref mut explorer) = self.explorer {
                    match explorer.handle_key(key) {
                        FileExplorerAction::OpenFile(path) => {
                            self.editor.open_file(&path);
                            self.editor.opened_from_explorer = true;
                            let is_py = path
                                .extension()
                                .is_some_and(|ext| ext == "py" || ext == "pyw");
                            if is_py {
                                self.spawn_runner_for_file(&path);
                            }
                            self.current_screen = CurrentScreen::Editor;
                        }
                        FileExplorerAction::BackToHome => {
                            self.cleanup_runner();
                            self.current_screen = CurrentScreen::Home;
                        }
                        FileExplorerAction::None => {}
                    }
                }
            }

            CurrentScreen::Editor => {
                // Intercept Ctrl+R to ensure runner window is spawned and running
                if key.modifiers.contains(KeyModifiers::CONTROL)
                    && (key.code == KeyCode::Char('r') || key.code == KeyCode::Char('R'))
                {
                    self.editor.save_file();
                    self.editor.run_toast_ticks = 30;
                    if !self.editor.is_runner_active()
                        && let Some(ref path) = self.editor.current_file_path.clone()
                    {
                        self.spawn_runner_for_file(path);
                    }
                    return;
                }

                match key.code {
                    KeyCode::Esc => {
                        if self.editor.opened_from_explorer {
                            self.current_screen = CurrentScreen::FileExplorer;
                        } else {
                            self.cleanup_runner();
                            self.current_screen = CurrentScreen::Home;
                        }
                    }
                    _ => self.editor.handle_key(key),
                }
            }

            CurrentScreen::Chat => {
                if self.chat.confirming_clear {
                    match key.code {
                        KeyCode::Char('y') | KeyCode::Char('Y') => self.chat.confirm_clear(true),
                        KeyCode::Char('n') | KeyCode::Char('N') | KeyCode::Esc => {
                            self.chat.confirm_clear(false)
                        }
                        _ => {}
                    }
                } else {
                    match key.code {
                        KeyCode::Esc => self.current_screen = CurrentScreen::Home,
                        KeyCode::Enter => self.chat.send_prompt(),
                        KeyCode::Backspace => self.chat.handle_backspace(),
                        KeyCode::Up => self.chat.scroll_up(),
                        KeyCode::Down => self.chat.scroll_down(),
                        KeyCode::Char(c) => self.chat.handle_char(c),
                        _ => {}
                    }
                }
            }

            CurrentScreen::Todo => match self.todo.mode {
                TodoInputMode::Adding => match key.code {
                    KeyCode::Enter => self.todo.commit_task(),
                    KeyCode::Esc => self.todo.cancel_adding(),
                    KeyCode::Backspace => self.todo.handle_input_backspace(),
                    KeyCode::Char(c) => self.todo.handle_input_char(c),
                    _ => {}
                },
                TodoInputMode::Normal => match key.code {
                    KeyCode::Esc => self.current_screen = CurrentScreen::Home,
                    KeyCode::Up | KeyCode::Char('k') => self.todo.previous(),
                    KeyCode::Down | KeyCode::Char('j') => self.todo.next(),
                    KeyCode::Char(' ') | KeyCode::Enter => self.todo.toggle_done(),
                    KeyCode::Char('a') | KeyCode::Char('n') => self.todo.start_adding(),
                    KeyCode::Char('d') | KeyCode::Char('x') | KeyCode::Delete => {
                        self.todo.delete_selected()
                    }
                    _ => {}
                },
            },

            CurrentScreen::Code => {
                if key.code == KeyCode::Esc {
                    self.current_screen = CurrentScreen::Home;
                }
            }

            CurrentScreen::Settings => match self.settings.handle_key(key) {
                SettingsAction::BackToHome => {
                    self.current_screen = CurrentScreen::Home;
                }
                SettingsAction::None => {}
            },
        }
    }

    pub fn handle_mouse(&mut self, mouse: MouseEvent) {
        if self.current_screen == CurrentScreen::Chat {
            match mouse.kind {
                MouseEventKind::ScrollUp => self.chat.scroll_up(),
                MouseEventKind::ScrollDown => self.chat.scroll_down(),
                _ => {}
            }
        } else if self.current_screen == CurrentScreen::Editor {
            self.editor.handle_mouse(mouse);
        }
    }

    pub fn cleanup_runner(&mut self) {
        if let Some(mut child) = self.editor.runner_child.take() {
            let _ = child.kill();
            let _ = child.wait();
        }
        self.editor.current_runner_path = None;
    }

    fn current_command_hints(&self) -> Vec<(&'static str, &'static str)> {
        match self.current_screen {
            CurrentScreen::Home => vec![
                ("↑/↓ / k/j", "Navigate"),
                ("Enter", "Select Screen"),
                ("q", "Quit"),
            ],
            CurrentScreen::EditorPrompt => vec![
                ("1", "Open Project Folder"),
                ("2", "Scratch & Runner"),
                ("Esc", "Main Menu"),
            ],
            CurrentScreen::FileExplorer => self
                .explorer
                .as_ref()
                .map(|e| e.commands_hint())
                .unwrap_or_default(),
            CurrentScreen::Chat => self.chat.commands_hint(),
            CurrentScreen::Editor => self.editor.commands_hint(),
            CurrentScreen::Todo => self.todo.commands_hint(),
            CurrentScreen::Code => self.code.commands_hint(),
            CurrentScreen::Settings => self.settings.commands_hint(),
        }
    }

    pub fn render(&mut self, frame: &mut Frame) {
        let full_area = frame.area();

        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Min(0),    // Main content area
                Constraint::Length(3), // Bottom command bar
            ])
            .split(full_area);

        // 1. Render active screen in upper area
        match self.current_screen {
            CurrentScreen::Home => self.home.render(frame, chunks[0]),
            CurrentScreen::EditorPrompt => {
                // Render home backdrop, then centered prompt modal
                self.home.render(frame, chunks[0]);
                self.render_editor_prompt(frame, chunks[0]);
            }
            CurrentScreen::FileExplorer => {
                if let Some(ref mut explorer) = self.explorer {
                    explorer.render(frame, chunks[0]);
                }
            }
            CurrentScreen::Chat => self.chat.render(frame, chunks[0]),
            CurrentScreen::Editor => self.editor.render(frame, chunks[0]),
            CurrentScreen::Todo => self.todo.render(frame, chunks[0]),
            CurrentScreen::Code => self.code.render(frame, chunks[0]),
            CurrentScreen::Settings => self.settings.render(frame, chunks[0]),
        }

        // 2. Render bottom commands bar
        let hints = self.current_command_hints();
        let mut spans = Vec::new();

        for (idx, (key, desc)) in hints.iter().enumerate() {
            if idx > 0 {
                spans.push(Span::styled(
                    "   │   ",
                    Style::default().fg(Color::DarkGray),
                ));
            }
            spans.push(Span::styled("[", Style::default().fg(Color::DarkGray)));
            spans.push(Span::styled(
                *key,
                Style::default()
                    .fg(Color::Yellow)
                    .add_modifier(Modifier::BOLD),
            ));
            spans.push(Span::styled("] ", Style::default().fg(Color::DarkGray)));
            spans.push(Span::styled(*desc, Style::default().fg(Color::White)));
        }

        let command_bar = Paragraph::new(Line::from(spans)).block(
            Block::default()
                .title(" Commands ")
                .borders(Borders::ALL)
                .border_type(BorderType::Rounded)
                .border_style(Style::default().fg(Color::DarkGray)),
        );

        frame.render_widget(command_bar, chunks[1]);
    }

    fn render_editor_prompt(&self, frame: &mut Frame, area: Rect) {
        let popup_area = centered_rect(66, 36, area);
        frame.render_widget(Clear, popup_area);

        let lines = if self.is_manual_folder_input {
            vec![
                Line::from(Span::styled(
                    "📁 Enter Project Folder Path:",
                    Style::default()
                        .fg(Color::Yellow)
                        .add_modifier(Modifier::BOLD),
                )),
                Line::from(""),
                Line::from(vec![
                    Span::styled(
                        "> ",
                        Style::default()
                            .fg(Color::Cyan)
                            .add_modifier(Modifier::BOLD),
                    ),
                    Span::styled(
                        &self.manual_folder_input,
                        Style::default()
                            .fg(Color::White)
                            .add_modifier(Modifier::BOLD),
                    ),
                    Span::styled("█", Style::default().fg(Color::Cyan)),
                ]),
                Line::from(""),
                Line::from(vec![
                    Span::styled(
                        "[Enter] ",
                        Style::default()
                            .fg(Color::Green)
                            .add_modifier(Modifier::BOLD),
                    ),
                    Span::styled("Open Folder    ", Style::default().fg(Color::White)),
                    Span::styled(
                        "[Esc] ",
                        Style::default().fg(Color::Red).add_modifier(Modifier::BOLD),
                    ),
                    Span::styled("Cancel", Style::default().fg(Color::White)),
                ]),
            ]
        } else {
            vec![
                Line::from(Span::styled(
                    "⚡ Choose Code Workspace Mode",
                    Style::default()
                        .fg(Color::Cyan)
                        .add_modifier(Modifier::BOLD),
                )),
                Line::from(""),
                Line::from(vec![
                    Span::styled(
                        "[1] ",
                        Style::default()
                            .fg(Color::Yellow)
                            .add_modifier(Modifier::BOLD),
                    ),
                    Span::styled(
                        "📁 Open Project Source Folder",
                        Style::default()
                            .fg(Color::White)
                            .add_modifier(Modifier::BOLD),
                    ),
                ]),
                Line::from(Span::styled(
                    "    Select a project directory using Linux file explorer.",
                    Style::default().fg(Color::Gray),
                )),
                Line::from(Span::styled(
                    "    Browse files in 2-column mode (Blue folders, Orange files, Purple nested files).",
                    Style::default().fg(Color::DarkGray),
                )),
                Line::from(""),
                Line::from(vec![
                    Span::styled(
                        "[2] ",
                        Style::default()
                            .fg(Color::Yellow)
                            .add_modifier(Modifier::BOLD),
                    ),
                    Span::styled(
                        "🧪 Scratch Script & Dual-Terminal Output Runner",
                        Style::default()
                            .fg(Color::White)
                            .add_modifier(Modifier::BOLD),
                    ),
                ]),
                Line::from(Span::styled(
                    "    Edit scratch code with a secondary Ratatui window streaming stdout/stderr.",
                    Style::default().fg(Color::Gray),
                )),
                Line::from(Span::styled(
                    "    Run with Ctrl+R and copy output to clipboard with Ctrl+C.",
                    Style::default().fg(Color::DarkGray),
                )),
                Line::from(""),
                Line::from(vec![
                    Span::styled(
                        "[Esc] ",
                        Style::default().fg(Color::Red).add_modifier(Modifier::BOLD),
                    ),
                    Span::styled(
                        "Cancel & Return to Main Menu",
                        Style::default().fg(Color::White),
                    ),
                ]),
            ]
        };

        let block = Block::default()
            .title(" Code Workspace Setup ")
            .title_alignment(Alignment::Center)
            .borders(Borders::ALL)
            .border_type(BorderType::Double)
            .border_style(Style::default().fg(Color::Blue));

        let p = Paragraph::new(lines).block(block);
        frame.render_widget(p, popup_area);
    }
}

/// Helper function to create a centered popup rect
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

/// Helper function to detect a runnable Python entrypoint in a project folder
pub fn find_python_entrypoint(folder: &Path) -> Option<PathBuf> {
    let standard_candidates = [
        "main.py",
        "app.py",
        "run.py",
        "__main__.py",
        "index.py",
        "server.py",
        "cli.py",
    ];
    for name in &standard_candidates {
        let p = folder.join(name);
        if p.is_file() {
            return Some(p);
        }
    }

    // Check common subdirectories like src/ or app/
    for sub in &["src", "app"] {
        for name in &["main.py", "app.py", "__main__.py", "server.py", "run.py"] {
            let p = folder.join(sub).join(name);
            if p.is_file() {
                return Some(p);
            }
        }
    }

    // Otherwise, check for any .py file directly in the folder (sorted for determinism)
    if let Ok(entries) = std::fs::read_dir(folder) {
        let mut py_files: Vec<PathBuf> = entries
            .filter_map(|res| res.ok())
            .map(|e| e.path())
            .filter(|p| p.is_file() && p.extension().is_some_and(|ext| ext == "py" || ext == "pyw"))
            .collect();
        py_files.sort();
        if let Some(first) = py_files.into_iter().next() {
            return Some(first);
        }
    }

    None
}

#[cfg(test)]
mod tests {
    use super::*;

    fn press_key(app: &mut App, code: KeyCode) {
        app.handle_key(KeyEvent::new(code, KeyModifiers::NONE));
    }

    #[test]
    fn test_home_navigation_and_enter() {
        let mut app = App::new();
        assert_eq!(app.current_screen, CurrentScreen::Home);

        // Home defaults to Code Editor (index 1)
        assert_eq!(app.home.selected_option(), HomeOption::Editor);

        // Press Enter -> enters EditorPrompt
        press_key(&mut app, KeyCode::Enter);
        assert_eq!(app.current_screen, CurrentScreen::EditorPrompt);

        // Press Esc -> returns to Home
        press_key(&mut app, KeyCode::Esc);
        assert_eq!(app.current_screen, CurrentScreen::Home);
    }

    #[test]
    fn test_editor_tab_and_typing_via_app() {
        let mut app = App::new();
        press_key(&mut app, KeyCode::Enter); // Home -> EditorPrompt
        assert_eq!(app.current_screen, CurrentScreen::EditorPrompt);

        // Select option 2: Scratch & Runner
        press_key(&mut app, KeyCode::Char('2'));
        assert_eq!(app.current_screen, CurrentScreen::Editor);

        // Press Tab (inserts 4 spaces)
        press_key(&mut app, KeyCode::Tab);
        assert_eq!(app.editor.cursor_col, 4);

        // Esc returns to Home
        press_key(&mut app, KeyCode::Esc);
        assert_eq!(app.current_screen, CurrentScreen::Home);
    }

    #[test]
    fn test_open_project_folder_navigation() {
        let mut app = App::new();
        let temp_dir = std::env::temp_dir().join("tomy_app_test_dir");
        let _ = std::fs::remove_dir_all(&temp_dir);
        std::fs::create_dir_all(&temp_dir).unwrap();
        std::fs::write(temp_dir.join("sample.py"), "print(123)").unwrap();

        app.open_project_folder(temp_dir.clone());
        assert_eq!(app.current_screen, CurrentScreen::FileExplorer);
        assert!(app.explorer.is_some());

        // Press Enter on sample.py -> opens in Editor
        press_key(&mut app, KeyCode::Enter);
        assert_eq!(app.current_screen, CurrentScreen::Editor);
        assert!(app.editor.opened_from_explorer);

        // Press Esc -> returns to FileExplorer
        press_key(&mut app, KeyCode::Esc);
        assert_eq!(app.current_screen, CurrentScreen::FileExplorer);

        // Press Esc in FileExplorer -> returns to Home
        press_key(&mut app, KeyCode::Esc);
        assert_eq!(app.current_screen, CurrentScreen::Home);

        let _ = std::fs::remove_dir_all(temp_dir);
    }

    #[test]
    fn test_chat_interaction_via_app() {
        let mut app = App::new();
        app.home.selected_index = 0; // Chat
        press_key(&mut app, KeyCode::Enter);
        assert_eq!(app.current_screen, CurrentScreen::Chat);

        // Type "Hi"
        press_key(&mut app, KeyCode::Char('H'));
        press_key(&mut app, KeyCode::Char('i'));
        assert_eq!(app.chat.input_buffer, "Hi");

        // Send with Enter
        press_key(&mut app, KeyCode::Enter);
        assert_eq!(app.chat.input_buffer, "");
        assert_eq!(
            app.chat.messages.last().unwrap().role,
            crate::chat::MessageRole::Assistant
        );

        // Esc returns to Home
        press_key(&mut app, KeyCode::Esc);
        assert_eq!(app.current_screen, CurrentScreen::Home);
    }

    #[test]
    fn test_chat_clear_flow_via_app() {
        let mut app = App::new();
        app.current_screen = CurrentScreen::Chat;

        // Type "/clear"
        for c in "/clear".chars() {
            press_key(&mut app, KeyCode::Char(c));
        }
        press_key(&mut app, KeyCode::Enter);
        assert!(app.chat.confirming_clear);

        // Cancel with 'n'
        press_key(&mut app, KeyCode::Char('n'));
        assert!(!app.chat.confirming_clear);
        assert!(!app.chat.messages.is_empty());

        // Type "/clear" again
        for c in "/clear".chars() {
            press_key(&mut app, KeyCode::Char(c));
        }
        press_key(&mut app, KeyCode::Enter);
        assert!(app.chat.confirming_clear);

        // Confirm with 'y'
        press_key(&mut app, KeyCode::Char('y'));
        assert!(!app.chat.confirming_clear);
        assert!(app.chat.messages.is_empty());
    }

    #[test]
    fn test_screens_esc_return() {
        let mut app = App::new();

        // Navigate to Chat (index 0)
        app.home.selected_index = 0;
        assert_eq!(app.home.selected_option(), HomeOption::Chat);
        press_key(&mut app, KeyCode::Enter);
        assert_eq!(app.current_screen, CurrentScreen::Chat);
        press_key(&mut app, KeyCode::Esc);
        assert_eq!(app.current_screen, CurrentScreen::Home);

        // Navigate to Editor (index 1) -> opens EditorPrompt
        app.home.selected_index = 1;
        assert_eq!(app.home.selected_option(), HomeOption::Editor);
        press_key(&mut app, KeyCode::Enter);
        assert_eq!(app.current_screen, CurrentScreen::EditorPrompt);
        press_key(&mut app, KeyCode::Esc);
        assert_eq!(app.current_screen, CurrentScreen::Home);

        // Navigate to Code (index 2)
        app.home.selected_index = 2;
        assert_eq!(app.home.selected_option(), HomeOption::Code);
        press_key(&mut app, KeyCode::Enter);
        assert_eq!(app.current_screen, CurrentScreen::Code);
        press_key(&mut app, KeyCode::Esc);
        assert_eq!(app.current_screen, CurrentScreen::Home);

        // Navigate to Todo (index 3)
        app.home.selected_index = 3;
        assert_eq!(app.home.selected_option(), HomeOption::Todo);
        press_key(&mut app, KeyCode::Enter);
        assert_eq!(app.current_screen, CurrentScreen::Todo);
        press_key(&mut app, KeyCode::Esc);
        assert_eq!(app.current_screen, CurrentScreen::Home);

        // Navigate to Settings (index 4)
        app.home.selected_index = 4;
        assert_eq!(app.home.selected_option(), HomeOption::Settings);
        press_key(&mut app, KeyCode::Enter);
        assert_eq!(app.current_screen, CurrentScreen::Settings);
        press_key(&mut app, KeyCode::Esc);
        assert_eq!(app.current_screen, CurrentScreen::Home);
    }

    #[test]
    fn test_quit_shortcut() {
        let mut app = App::new();
        assert!(!app.should_quit);
        press_key(&mut app, KeyCode::Char('q'));
        assert!(app.should_quit);
    }

    #[test]
    fn test_find_python_entrypoint() {
        let temp_dir = std::env::temp_dir().join("tomy_entrypoint_test_dir");
        let _ = std::fs::remove_dir_all(&temp_dir);
        std::fs::create_dir_all(&temp_dir).unwrap();

        // Empty dir -> None
        assert_eq!(find_python_entrypoint(&temp_dir), None);

        // Fallback to any python file
        let z_py = temp_dir.join("zebra.py");
        std::fs::write(&z_py, "print('zebra')").unwrap();
        assert_eq!(find_python_entrypoint(&temp_dir), Some(z_py));

        // app.py has higher priority than arbitrary python file
        let app_py = temp_dir.join("app.py");
        std::fs::write(&app_py, "print('app')").unwrap();
        assert_eq!(find_python_entrypoint(&temp_dir), Some(app_py));

        // main.py has highest priority
        let main_py = temp_dir.join("main.py");
        std::fs::write(&main_py, "print('main')").unwrap();
        assert_eq!(find_python_entrypoint(&temp_dir), Some(main_py));

        let _ = std::fs::remove_dir_all(temp_dir);
    }

    #[test]
    fn test_open_project_folder_auto_spawns_runner() {
        let mut app = App::new();
        let temp_dir = std::env::temp_dir().join("tomy_runner_folder_test");
        let _ = std::fs::remove_dir_all(&temp_dir);
        std::fs::create_dir_all(&temp_dir).unwrap();
        let main_py = temp_dir.join("main.py");
        std::fs::write(&main_py, "print('autostart')").unwrap();

        app.open_project_folder(temp_dir.clone());
        assert_eq!(app.current_screen, CurrentScreen::FileExplorer);
        assert_eq!(app.editor.current_runner_path, Some(main_py.clone()));

        // Pressing Enter on file in explorer keeps or updates runner path
        press_key(&mut app, KeyCode::Enter);
        assert_eq!(app.current_screen, CurrentScreen::Editor);
        assert_eq!(app.editor.current_runner_path, Some(main_py));

        app.cleanup_runner();
        assert_eq!(app.editor.current_runner_path, None);

        let _ = std::fs::remove_dir_all(temp_dir);
    }

    #[test]
    fn test_editor_ctrl_r_runner_trigger() {
        let mut app = App::new();
        let temp_file = std::env::temp_dir().join("tomy_ctrl_r_test.py");
        std::fs::write(&temp_file, "val = 1").unwrap();

        app.editor.open_file(&temp_file);
        app.current_screen = CurrentScreen::Editor;
        app.editor.lines[0] = "val = 42".to_string();
        app.editor.is_modified = true;

        // Press Ctrl+R
        let key = KeyEvent::new(KeyCode::Char('r'), KeyModifiers::CONTROL);
        app.handle_key(key);

        // Saved to disk, toast active, and runner path tracked
        assert!(!app.editor.is_modified);
        assert!(app.editor.run_toast_ticks > 0);
        assert_eq!(app.editor.current_runner_path, Some(temp_file.clone()));

        let disk_content = std::fs::read_to_string(&temp_file).unwrap();
        assert_eq!(disk_content.trim(), "val = 42");

        app.cleanup_runner();
        let _ = std::fs::remove_file(temp_file);
    }
}
