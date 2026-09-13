use std::error::Error;
use std::io::{self, Write};
use std::path::PathBuf;
use std::process::{Command, Stdio};
use std::time::{Duration, Instant, SystemTime};

use crossterm::{
    event::{self, Event, KeyCode, KeyEvent, KeyEventKind, KeyModifiers},
    execute,
    terminal::{EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode},
};
use ratatui::{
    Frame, Terminal,
    backend::CrosstermBackend,
    layout::{Constraint, Direction, Layout},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, BorderType, Borders, Paragraph},
};

#[derive(Debug, Clone)]
pub struct OutputLine {
    pub text: String,
    pub is_stderr: bool,
}

pub struct RunnerApp {
    pub script_path: PathBuf,
    pub output_lines: Vec<OutputLine>,
    pub scroll_offset: u16,
    pub max_scroll: u16,
    pub last_modified: Option<SystemTime>,
    pub last_duration_ms: f64,
    pub last_exit_code: Option<i32>,
    pub is_running: bool,
    pub copied_toast_ticks: u8,
    pub should_quit: bool,
    pub output_tokens: usize,
}

impl RunnerApp {
    pub fn new(script_path: PathBuf) -> Self {
        let last_mod = std::fs::metadata(&script_path)
            .and_then(|m| m.modified())
            .ok();

        let mut app = Self {
            script_path,
            output_lines: Vec::new(),
            scroll_offset: 0,
            max_scroll: 0,
            last_modified: last_mod,
            last_duration_ms: 0.0,
            last_exit_code: None,
            is_running: false,
            copied_toast_ticks: 0,
            should_quit: false,
            output_tokens: 0,
        };

        // Execute immediately on startup
        app.run_script();
        app
    }

    pub fn run_script(&mut self) {
        self.is_running = true;
        self.output_lines.clear();

        let start = Instant::now();
        let mut cmd = Command::new("python3");
        if let Some(parent) = self.script_path.parent() {
            cmd.current_dir(parent);
        }
        let result = cmd
            .arg(&self.script_path)
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .output();

        self.last_duration_ms = start.elapsed().as_secs_f64() * 1000.0;
        self.is_running = false;

        match result {
            Ok(output) => {
                self.last_exit_code = output.status.code();
                let stdout_str = String::from_utf8_lossy(&output.stdout);
                for line in stdout_str.lines() {
                    self.output_lines.push(OutputLine {
                        text: line.to_string(),
                        is_stderr: false,
                    });
                }

                let stderr_str = String::from_utf8_lossy(&output.stderr);
                for line in stderr_str.lines() {
                    self.output_lines.push(OutputLine {
                        text: line.to_string(),
                        is_stderr: true,
                    });
                }

                if self.output_lines.is_empty() {
                    self.output_lines.push(OutputLine {
                        text: "(Process completed successfully with empty output)".to_string(),
                        is_stderr: false,
                    });
                }
            }
            Err(e) => {
                self.last_exit_code = Some(-1);
                self.output_lines.push(OutputLine {
                    text: format!("Failed to execute python3: {}", e),
                    is_stderr: true,
                });
            }
        }

        let all_output = self
            .output_lines
            .iter()
            .map(|l| l.text.as_str())
            .collect::<Vec<_>>()
            .join("\n");
        self.output_tokens = crate::token_counter::count_tokens_str(&all_output);

        self.scroll_offset = 0;
    }

    pub fn check_file_update(&mut self) {
        if let Ok(mod_time) = std::fs::metadata(&self.script_path).and_then(|m| m.modified()) {
            if let Some(prev) = self.last_modified {
                if mod_time > prev {
                    self.last_modified = Some(mod_time);
                    self.run_script();
                }
            } else {
                self.last_modified = Some(mod_time);
            }
        }
    }

    pub fn copy_output_to_clipboard(&mut self) {
        let full_text: String = self
            .output_lines
            .iter()
            .map(|l| l.text.as_str())
            .collect::<Vec<&str>>()
            .join("\n");

        copy_text_to_clipboard(&full_text);
        self.copied_toast_ticks = 40; // ~2 seconds notification
    }

    pub fn handle_key(&mut self, key: KeyEvent) {
        if key.kind != KeyEventKind::Press {
            return;
        }

        // Ctrl+C copies output to clipboard (as requested!)
        if key.modifiers.contains(KeyModifiers::CONTROL) && key.code == KeyCode::Char('c') {
            self.copy_output_to_clipboard();
            return;
        }

        match key.code {
            KeyCode::Esc | KeyCode::Char('q') => {
                self.should_quit = true;
            }
            KeyCode::Char('r') => {
                self.run_script();
            }
            KeyCode::Up | KeyCode::Char('k') => {
                self.scroll_offset = self.scroll_offset.saturating_sub(1);
            }
            KeyCode::Down | KeyCode::Char('j') => {
                if self.scroll_offset < self.max_scroll {
                    self.scroll_offset += 1;
                }
            }
            KeyCode::PageUp => {
                self.scroll_offset = self.scroll_offset.saturating_sub(15);
            }
            KeyCode::PageDown => {
                self.scroll_offset = (self.scroll_offset + 15).min(self.max_scroll);
            }
            _ => {}
        }
    }

    pub fn render(&mut self, frame: &mut Frame) {
        let area = frame.area();

        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .margin(1)
            .constraints([
                Constraint::Length(3), // Header banner
                Constraint::Min(6),    // Output terminal canvas
                Constraint::Length(3), // Command bar
            ])
            .split(area);

        // 1. Header Banner
        let filename = self
            .script_path
            .file_name()
            .and_then(|s| s.to_str())
            .unwrap_or("script.py");

        let (status_span, border_color) = if self.is_running {
            (
                Span::styled(
                    " ● Executing...",
                    Style::default()
                        .fg(Color::Yellow)
                        .add_modifier(Modifier::BOLD),
                ),
                Color::Yellow,
            )
        } else if let Some(code) = self.last_exit_code {
            if code == 0 {
                (
                    Span::styled(
                        format!(" ✓ Exit: 0 ({:.1} ms)", self.last_duration_ms),
                        Style::default()
                            .fg(Color::Green)
                            .add_modifier(Modifier::BOLD),
                    ),
                    Color::Green,
                )
            } else {
                (
                    Span::styled(
                        format!(" ✗ Exit: {} ({:.1} ms)", code, self.last_duration_ms),
                        Style::default().fg(Color::Red).add_modifier(Modifier::BOLD),
                    ),
                    Color::Red,
                )
            }
        } else {
            (
                Span::styled(" Ready", Style::default().fg(Color::DarkGray)),
                Color::DarkGray,
            )
        };

        let header_line = Line::from(vec![
            Span::styled(
                "⚡ TOMY RUNNER ",
                Style::default()
                    .fg(Color::Cyan)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::styled("— Watching: ", Style::default().fg(Color::DarkGray)),
            Span::styled(
                filename,
                Style::default()
                    .fg(Color::White)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::styled("   │   Status: ", Style::default().fg(Color::DarkGray)),
            status_span,
            Span::styled("   │   Tokens: ", Style::default().fg(Color::DarkGray)),
            Span::styled(
                crate::token_counter::format_token_count(self.output_tokens),
                Style::default()
                    .fg(Color::Cyan)
                    .add_modifier(Modifier::BOLD),
            ),
        ]);

        let header_block = Block::default()
            .title(" Process Execution ")
            .borders(Borders::ALL)
            .border_type(BorderType::Rounded)
            .border_style(Style::default().fg(border_color));

        frame.render_widget(Paragraph::new(header_line).block(header_block), chunks[0]);

        // 2. Output Lines Canvas
        let canvas_area = chunks[1];
        let visible_height = canvas_area.height.saturating_sub(2);
        self.max_scroll = (self.output_lines.len() as u16).saturating_sub(visible_height);

        let lines: Vec<Line> = self
            .output_lines
            .iter()
            .map(|item| {
                if item.is_stderr {
                    Line::from(Span::styled(
                        &item.text,
                        Style::default().fg(Color::LightRed),
                    ))
                } else {
                    Line::from(Span::styled(&item.text, Style::default().fg(Color::White)))
                }
            })
            .collect();

        let title = format!(
            " Output ({} lines │ {} tokens) ",
            self.output_lines.len(),
            crate::token_counter::format_token_count(self.output_tokens)
        );
        let output_block = Block::default()
            .title(title)
            .borders(Borders::ALL)
            .border_type(BorderType::Rounded)
            .border_style(Style::default().fg(Color::DarkGray));

        let output_paragraph = Paragraph::new(lines)
            .block(output_block)
            .scroll((self.scroll_offset, 0));

        frame.render_widget(output_paragraph, canvas_area);

        // 3. Command Bar & Toast
        let mut spans = vec![
            Span::styled("[", Style::default().fg(Color::DarkGray)),
            Span::styled(
                "Ctrl+C",
                Style::default()
                    .fg(Color::Yellow)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::styled("] ", Style::default().fg(Color::DarkGray)),
            Span::styled("Copy Output", Style::default().fg(Color::White)),
            Span::styled("   │   ", Style::default().fg(Color::DarkGray)),
            Span::styled("[", Style::default().fg(Color::DarkGray)),
            Span::styled(
                "r",
                Style::default()
                    .fg(Color::Yellow)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::styled("] ", Style::default().fg(Color::DarkGray)),
            Span::styled("Re-run Script", Style::default().fg(Color::White)),
            Span::styled("   │   ", Style::default().fg(Color::DarkGray)),
            Span::styled("[", Style::default().fg(Color::DarkGray)),
            Span::styled(
                "↑/↓",
                Style::default()
                    .fg(Color::Yellow)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::styled("] ", Style::default().fg(Color::DarkGray)),
            Span::styled("Scroll", Style::default().fg(Color::White)),
            Span::styled("   │   ", Style::default().fg(Color::DarkGray)),
            Span::styled("[", Style::default().fg(Color::DarkGray)),
            Span::styled(
                "q/Esc",
                Style::default()
                    .fg(Color::Yellow)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::styled("] ", Style::default().fg(Color::DarkGray)),
            Span::styled("Close Window", Style::default().fg(Color::White)),
        ];

        if self.copied_toast_ticks > 0 {
            self.copied_toast_ticks = self.copied_toast_ticks.saturating_sub(1);
            spans.push(Span::styled(
                "    ✔ COPIED TO CLIPBOARD! ",
                Style::default()
                    .fg(Color::Black)
                    .bg(Color::Green)
                    .add_modifier(Modifier::BOLD),
            ));
        }

        let cmd_block = Block::default()
            .title(" Actions ")
            .borders(Borders::ALL)
            .border_type(BorderType::Rounded)
            .border_style(Style::default().fg(Color::Blue));

        frame.render_widget(
            Paragraph::new(Line::from(spans)).block(cmd_block),
            chunks[2],
        );
    }
}

/// Helper function to copy text to system clipboard via wl-copy, xclip, or xsel
pub fn copy_text_to_clipboard(text: &str) {
    // 1. Try wl-copy (Wayland standard)
    if let Ok(mut child) = Command::new("wl-copy").stdin(Stdio::piped()).spawn() {
        if let Some(mut stdin) = child.stdin.take() {
            let _ = stdin.write_all(text.as_bytes());
        }
        if child.wait().is_ok_and(|status| status.success()) {
            return;
        }
    }

    // 2. Try xclip (X11 standard)
    if let Ok(mut child) = Command::new("xclip")
        .args(["-selection", "clipboard"])
        .stdin(Stdio::piped())
        .spawn()
    {
        if let Some(mut stdin) = child.stdin.take() {
            let _ = stdin.write_all(text.as_bytes());
        }
        if child.wait().is_ok_and(|status| status.success()) {
            return;
        }
    }

    // 3. Try xsel (X11 alternative)
    if let Ok(mut child) = Command::new("xsel")
        .args(["--clipboard", "--input"])
        .stdin(Stdio::piped())
        .spawn()
    {
        if let Some(mut stdin) = child.stdin.take() {
            let _ = stdin.write_all(text.as_bytes());
        }
        let _ = child.wait();
    }
}

/// Standalone entry point when Tomy is run with `tomy runner <file_path>`
pub fn run_runner(script_path_str: &str) -> Result<(), Box<dyn Error>> {
    let script_path = PathBuf::from(script_path_str);

    // Setup terminal
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;
    terminal.clear()?;

    let mut app = RunnerApp::new(script_path);

    while !app.should_quit {
        terminal.draw(|frame| app.render(frame))?;

        if event::poll(Duration::from_millis(100))?
            && let Event::Key(key) = event::read()?
        {
            app.handle_key(key);
        }

        // Check if file was modified on disk by editor
        app.check_file_update();
    }

    disable_raw_mode()?;
    execute!(terminal.backend_mut(), LeaveAlternateScreen)?;
    terminal.show_cursor()?;

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_runner_initialization() {
        let temp_file = std::env::temp_dir().join("tomy_runner_test.py");
        std::fs::write(&temp_file, "print('Hello from Tomy Runner!')").unwrap();

        let runner = RunnerApp::new(temp_file.clone());
        assert_eq!(runner.last_exit_code, Some(0));
        assert!(
            runner
                .output_lines
                .iter()
                .any(|l| l.text.contains("Hello from Tomy Runner!"))
        );

        let _ = std::fs::remove_file(temp_file);
    }

    #[test]
    fn test_runner_clipboard_action() {
        let temp_file = std::env::temp_dir().join("tomy_runner_clip.py");
        std::fs::write(&temp_file, "print('Clipboard Output Test')").unwrap();

        let mut runner = RunnerApp::new(temp_file.clone());
        runner.copy_output_to_clipboard();
        assert_eq!(runner.copied_toast_ticks, 40);

        let _ = std::fs::remove_file(temp_file);
    }
}
