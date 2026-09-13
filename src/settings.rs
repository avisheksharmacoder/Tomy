use crossterm::event::{KeyCode, KeyEvent, KeyEventKind};
use ratatui::{
    Frame,
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, BorderType, Borders, Paragraph},
};

use crate::token_counter::{TokenizerEncoding, get_active_encoding, set_active_encoding};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SettingsAction {
    None,
    BackToHome,
}

pub struct SettingsScreen {
    pub selected_index: usize,
    pub status_message: Option<String>,
}

impl SettingsScreen {
    pub fn new() -> Self {
        Self {
            selected_index: 0,
            status_message: None,
        }
    }

    pub fn handle_key(&mut self, key: KeyEvent) -> SettingsAction {
        if key.kind != KeyEventKind::Press {
            return SettingsAction::None;
        }

        match key.code {
            KeyCode::Esc => SettingsAction::BackToHome,
            KeyCode::Up | KeyCode::Char('k') => {
                if self.selected_index > 0 {
                    self.selected_index -= 1;
                }
                SettingsAction::None
            }
            KeyCode::Down | KeyCode::Char('j') => {
                if self.selected_index < 6 {
                    self.selected_index += 1;
                }
                SettingsAction::None
            }
            KeyCode::Enter | KeyCode::Char(' ') | KeyCode::Left | KeyCode::Right => {
                if self.selected_index == 0 {
                    // Toggle BPE encoding standard
                    let current = get_active_encoding();
                    let next = match current {
                        TokenizerEncoding::Cl100kBase => TokenizerEncoding::O200kBase,
                        TokenizerEncoding::O200kBase => TokenizerEncoding::Cl100kBase,
                    };
                    set_active_encoding(next);
                    self.status_message = Some(format!(
                        "✔ Switched active BPE tokenizer to {}",
                        next.name()
                    ));
                }
                SettingsAction::None
            }
            _ => SettingsAction::None,
        }
    }

    pub fn commands_hint(&self) -> Vec<(&'static str, &'static str)> {
        vec![
            ("↑/↓ / k/j", "Navigate"),
            ("Space / Enter", "Toggle Setting"),
            ("Esc", "Back to Main Menu"),
        ]
    }

    pub fn render(&self, frame: &mut Frame, area: Rect) {
        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .margin(1)
            .constraints([
                Constraint::Length(3), // Banner
                Constraint::Min(6),    // Settings list
            ])
            .split(area);

        // 1. Status Banner
        let header = Paragraph::new(Line::from(vec![
            Span::styled(
                "⚙️ Settings ",
                Style::default()
                    .fg(Color::Yellow)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::styled(
                "— Small Model Harness & Tokenizer Configuration",
                Style::default().fg(Color::White),
            ),
            Span::styled(
                "   │   [Space/Enter] Toggle  │  [Esc] Back",
                Style::default().fg(Color::DarkGray),
            ),
        ]))
        .block(
            Block::default()
                .title(" Configuration ")
                .borders(Borders::ALL)
                .border_type(BorderType::Rounded)
                .border_style(Style::default().fg(Color::Blue)),
        );
        frame.render_widget(header, chunks[0]);

        // 2. Interactive Settings Items
        let active_encoding = get_active_encoding();
        let mut settings_lines = Vec::new();

        // Item 0: Tokenizer BPE Standard
        let is_sel_0 = self.selected_index == 0;
        let cursor_0 = if is_sel_0 { "▶ " } else { "  " };
        let style_0 = if is_sel_0 {
            Style::default()
                .fg(Color::Yellow)
                .add_modifier(Modifier::BOLD)
        } else {
            Style::default().fg(Color::Cyan)
        };

        settings_lines.push(Line::from(vec![
            Span::styled(cursor_0, style_0),
            Span::styled("• Active BPE Tokenizer:    ", style_0),
            Span::styled(
                format!("[ {} ]", active_encoding.display_label()),
                Style::default()
                    .fg(Color::Green)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::styled(
                "  ◄ (Press Space / Enter to toggle)",
                Style::default().fg(Color::DarkGray),
            ),
        ]));

        // Radio sub-indicators for Tokenizer BPE
        let cl100k_mark = if active_encoding == TokenizerEncoding::Cl100kBase {
            " (●) "
        } else {
            " ( ) "
        };
        let o200k_mark = if active_encoding == TokenizerEncoding::O200kBase {
            " (●) "
        } else {
            " ( ) "
        };

        settings_lines.push(Line::from(vec![
            Span::raw("      "),
            Span::styled(
                cl100k_mark,
                if active_encoding == TokenizerEncoding::Cl100kBase {
                    Style::default()
                        .fg(Color::Green)
                        .add_modifier(Modifier::BOLD)
                } else {
                    Style::default().fg(Color::DarkGray)
                },
            ),
            Span::styled(
                "cl100k_base  — GPT-4 / Claude / modern standard",
                Style::default().fg(Color::White),
            ),
        ]));
        settings_lines.push(Line::from(vec![
            Span::raw("      "),
            Span::styled(
                o200k_mark,
                if active_encoding == TokenizerEncoding::O200kBase {
                    Style::default()
                        .fg(Color::Green)
                        .add_modifier(Modifier::BOLD)
                } else {
                    Style::default().fg(Color::DarkGray)
                },
            ),
            Span::styled(
                "o200k_base   — GPT-4o standard",
                Style::default().fg(Color::White),
            ),
        ]));
        settings_lines.push(Line::from(""));

        // Items 1-6: Model Harness Configuration
        let other_items = [
            (
                "• Backend Provider:        ",
                "Local GGUF / llama-server (Mock)",
            ),
            (
                "• Model File:              ",
                "models/smollm2-1.7b-instruct-q4_k_m.gguf",
            ),
            ("• Context Window Size:     ", "2048 tokens"),
            ("• Temperature:             ", "0.7"),
            ("• Top-p:                   ", "0.9"),
            ("• CPU Threads:             ", "4"),
        ];

        for (i, (label, val)) in other_items.iter().enumerate() {
            let idx = i + 1;
            let is_sel = self.selected_index == idx;
            let cursor = if is_sel { "▶ " } else { "  " };
            let style = if is_sel {
                Style::default()
                    .fg(Color::Yellow)
                    .add_modifier(Modifier::BOLD)
            } else {
                Style::default().fg(Color::Cyan)
            };

            settings_lines.push(Line::from(vec![
                Span::styled(cursor, style),
                Span::styled(*label, style),
                Span::styled(*val, Style::default().fg(Color::White)),
            ]));
        }

        settings_lines.push(Line::from(""));

        if let Some(ref msg) = self.status_message {
            settings_lines.push(Line::from(Span::styled(
                format!("  {}", msg),
                Style::default()
                    .fg(Color::Green)
                    .add_modifier(Modifier::BOLD),
            )));
            settings_lines.push(Line::from(""));
        }

        settings_lines.push(Line::from(Span::styled(
            "  Config file: ~/.config/tomy/config.json (Tiktoken counter runs live across Explorer, Editor & Runner)",
            Style::default().fg(Color::DarkGray),
        )));

        let settings_box = Paragraph::new(settings_lines).block(
            Block::default()
                .title(" Model & Tokenizer Parameters ")
                .borders(Borders::ALL)
                .border_type(BorderType::Rounded)
                .border_style(Style::default().fg(Color::DarkGray)),
        );
        frame.render_widget(settings_box, chunks[1]);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_settings_toggle_bpe_encoding() {
        let mut settings = SettingsScreen::new();
        assert_eq!(settings.selected_index, 0);

        // Ensure baseline
        set_active_encoding(TokenizerEncoding::Cl100kBase);
        assert_eq!(get_active_encoding(), TokenizerEncoding::Cl100kBase);

        // Press Enter or Space to toggle
        let action = settings.handle_key(KeyEvent::new(
            KeyCode::Enter,
            crossterm::event::KeyModifiers::NONE,
        ));
        assert_eq!(action, SettingsAction::None);
        assert_eq!(get_active_encoding(), TokenizerEncoding::O200kBase);
        assert!(
            settings
                .status_message
                .as_ref()
                .unwrap()
                .contains("o200k_base")
        );

        // Toggle back
        settings.handle_key(KeyEvent::new(
            KeyCode::Char(' '),
            crossterm::event::KeyModifiers::NONE,
        ));
        assert_eq!(get_active_encoding(), TokenizerEncoding::Cl100kBase);

        // Test Esc key -> BackToHome
        let action = settings.handle_key(KeyEvent::new(
            KeyCode::Esc,
            crossterm::event::KeyModifiers::NONE,
        ));
        assert_eq!(action, SettingsAction::BackToHome);
    }
}
