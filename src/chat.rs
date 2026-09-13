use ratatui::{
    Frame,
    layout::{Alignment, Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, BorderType, Borders, Clear, Paragraph},
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MessageRole {
    User,
    Assistant,
}

#[derive(Debug, Clone)]
pub struct ChatMessage {
    pub role: MessageRole,
    pub content: String,
}

pub struct ChatScreen {
    pub messages: Vec<ChatMessage>,
    pub input_buffer: String,
    pub scroll_offset: u16,
    pub max_scroll: u16,
    pub confirming_clear: bool,
}

impl ChatScreen {
    pub fn new() -> Self {
        Self {
            messages: vec![
                ChatMessage {
                    role: MessageRole::User,
                    content: "What small LLMs are ideal for local terminal harnesses?".to_string(),
                },
                ChatMessage {
                    role: MessageRole::Assistant,
                    content: "SmolLM2 (360M, 1.7B), Qwen2.5-Coder (0.5B, 1.5B, 3B), and Llama 3.2 (1B, 3B) are outstanding choices! Quantized to 4-bit GGUF, they run comfortably with under 2GB of RAM and produce fast responses even on low-power CPUs.".to_string(),
                },
            ],
            input_buffer: String::new(),
            scroll_offset: 0,
            max_scroll: 0,
            confirming_clear: false,
        }
    }

    pub fn handle_char(&mut self, c: char) {
        if !self.confirming_clear {
            self.input_buffer.push(c);
        }
    }

    pub fn handle_backspace(&mut self) {
        if !self.confirming_clear {
            self.input_buffer.pop();
        }
    }

    pub fn send_prompt(&mut self) {
        if self.confirming_clear {
            return;
        }

        let trimmed = self.input_buffer.trim();
        if trimmed.eq_ignore_ascii_case("/clear") {
            self.confirming_clear = true;
            return;
        }

        if trimmed.is_empty() {
            return;
        }

        let user_prompt = trimmed.to_string();
        self.messages.push(ChatMessage {
            role: MessageRole::User,
            content: user_prompt.clone(),
        });

        let assistant_response = generate_mock_response(&user_prompt);
        self.messages.push(ChatMessage {
            role: MessageRole::Assistant,
            content: assistant_response,
        });

        self.input_buffer.clear();
        self.scroll_to_bottom();
    }

    pub fn confirm_clear(&mut self, confirm: bool) {
        if confirm {
            self.messages.clear();
            self.scroll_offset = 0;
            self.max_scroll = 0;
        }
        self.input_buffer.clear();
        self.confirming_clear = false;
    }

    pub fn scroll_up(&mut self) {
        self.scroll_offset = self.scroll_offset.saturating_sub(2);
    }

    pub fn scroll_down(&mut self) {
        if self.scroll_offset + 2 <= self.max_scroll {
            self.scroll_offset += 2;
        } else {
            self.scroll_offset = self.max_scroll;
        }
    }

    pub fn scroll_to_bottom(&mut self) {
        self.scroll_offset = self.max_scroll;
    }

    pub fn commands_hint(&self) -> Vec<(&'static str, &'static str)> {
        if self.confirming_clear {
            vec![("y", "Confirm Clear"), ("n / Esc", "Cancel")]
        } else {
            vec![
                ("Enter", "Send Prompt"),
                ("↑/↓ / Mouse", "Scroll"),
                ("/clear", "Reset History"),
                ("Esc", "Back to Home"),
            ]
        }
    }

    pub fn render(&mut self, frame: &mut Frame, area: Rect) {
        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .margin(1)
            .constraints([
                Constraint::Min(6),    // Chat messages container
                Constraint::Length(3), // Blue-bordered input bar
            ])
            .split(area);

        let container_area = chunks[0];
        let input_area = chunks[1];

        // 1. Build conversation lines with square 1px borders (Blue for User, Green for Assistant)
        let inner_width = (container_area.width.saturating_sub(6)) as usize;
        let mut lines: Vec<Line> = Vec::new();

        if self.messages.is_empty() {
            lines.push(Line::from(""));
            lines.push(Line::from(Span::styled(
                "  No messages in this session. Type a prompt below and press Enter to chat!",
                Style::default().fg(Color::DarkGray),
            )));
        } else {
            for (idx, msg) in self.messages.iter().enumerate() {
                if idx > 0 {
                    lines.push(Line::from("")); // spacing between message bubbles
                }

                let (border_color, title) = match msg.role {
                    MessageRole::User => (Color::Blue, " You "),
                    MessageRole::Assistant => (Color::Green, " Tomy (SmolLM2) "),
                };

                let border_style = Style::default().fg(border_color);
                let title_style = Style::default()
                    .fg(border_color)
                    .add_modifier(Modifier::BOLD);

                // Top border with title
                let title_len = title.chars().count();
                let remaining_width = inner_width.saturating_sub(title_len + 3);
                let mut top_spans = vec![
                    Span::styled("┌", border_style),
                    Span::styled(title, title_style),
                ];
                top_spans.push(Span::styled("─".repeat(remaining_width), border_style));
                top_spans.push(Span::styled("┐", border_style));
                lines.push(Line::from(top_spans));

                // Wrapped message content lines
                let wrapped_content_lines = wrap_text(&msg.content, inner_width.saturating_sub(4));
                for content_line in wrapped_content_lines {
                    let padding_len = inner_width
                        .saturating_sub(4)
                        .saturating_sub(content_line.chars().count());
                    lines.push(Line::from(vec![
                        Span::styled("│  ", border_style),
                        Span::styled(content_line, Style::default().fg(Color::White)),
                        Span::styled(" ".repeat(padding_len), Style::default()),
                        Span::styled("  │", border_style),
                    ]));
                }

                // Bottom border
                lines.push(Line::from(vec![
                    Span::styled("└", border_style),
                    Span::styled("─".repeat(inner_width.saturating_sub(2)), border_style),
                    Span::styled("┘", border_style),
                ]));
            }
        }

        let total_lines = lines.len() as u16;
        let visible_height = container_area.height.saturating_sub(2);
        self.max_scroll = total_lines.saturating_sub(visible_height);

        // Keep scroll offset bounded
        if self.scroll_offset > self.max_scroll {
            self.scroll_offset = self.max_scroll;
        }

        // Render Conversation Container Block
        let container_title = format!(
            " 💬 Conversation Container (History: {} messages) ",
            self.messages.len()
        );
        let container_block = Block::default()
            .title(container_title)
            .borders(Borders::ALL)
            .border_type(BorderType::Rounded)
            .border_style(Style::default().fg(Color::DarkGray));

        let conversation_paragraph = Paragraph::new(lines)
            .block(container_block)
            .scroll((self.scroll_offset, 0));

        frame.render_widget(conversation_paragraph, container_area);

        // 2. Render Text Input with 1px Blue Border
        let input_content = if self.input_buffer.is_empty() {
            vec![
                Span::styled(
                    "> ",
                    Style::default()
                        .fg(Color::Blue)
                        .add_modifier(Modifier::BOLD),
                ),
                Span::styled(
                    "Type a prompt and press Enter... (type '/clear' to wipe)",
                    Style::default().fg(Color::DarkGray),
                ),
                Span::styled("█", Style::default().fg(Color::Blue)),
            ]
        } else {
            vec![
                Span::styled(
                    "> ",
                    Style::default()
                        .fg(Color::Blue)
                        .add_modifier(Modifier::BOLD),
                ),
                Span::styled(&self.input_buffer, Style::default().fg(Color::White)),
                Span::styled("█", Style::default().fg(Color::Cyan)),
            ]
        };

        let input_block = Block::default()
            .title(" Chat Input (Press Enter to Send) ")
            .borders(Borders::ALL)
            .border_type(BorderType::Plain)
            .border_style(Style::default().fg(Color::Blue));

        let input_paragraph = Paragraph::new(Line::from(input_content)).block(input_block);
        frame.render_widget(input_paragraph, input_area);

        // 3. Render Confirmation Modal if /clear was submitted
        if self.confirming_clear {
            let popup_area = centered_rect(55, 25, area);
            frame.render_widget(Clear, popup_area);

            let modal_text = vec![
                Line::from(Span::styled(
                    "⚠️  Clear Chat Session?",
                    Style::default()
                        .fg(Color::Yellow)
                        .add_modifier(Modifier::BOLD),
                )),
                Line::from(""),
                Line::from(Span::styled(
                    "This will delete all messages in this conversation.",
                    Style::default().fg(Color::White),
                )),
                Line::from(""),
                Line::from(vec![
                    Span::styled(
                        "[y] ",
                        Style::default()
                            .fg(Color::Green)
                            .add_modifier(Modifier::BOLD),
                    ),
                    Span::styled("Confirm Deletion     ", Style::default().fg(Color::White)),
                    Span::styled(
                        "[n / Esc] ",
                        Style::default().fg(Color::Red).add_modifier(Modifier::BOLD),
                    ),
                    Span::styled("Cancel", Style::default().fg(Color::White)),
                ]),
            ];

            let modal_block = Block::default()
                .title(" Confirm Reset ")
                .title_alignment(Alignment::Center)
                .borders(Borders::ALL)
                .border_type(BorderType::Double)
                .border_style(Style::default().fg(Color::Yellow));

            let modal_paragraph = Paragraph::new(modal_text)
                .alignment(Alignment::Center)
                .block(modal_block);

            frame.render_widget(modal_paragraph, popup_area);
        }
    }
}

/// Helper function to wrap text neatly within inner width
fn wrap_text(text: &str, max_width: usize) -> Vec<String> {
    let mut lines = Vec::new();
    let max_len = if max_width < 10 { 10 } else { max_width };

    for paragraph in text.split('\n') {
        let mut current_line = String::new();
        for word in paragraph.split_whitespace() {
            if current_line.is_empty() {
                current_line.push_str(word);
            } else if current_line.chars().count() + 1 + word.chars().count() <= max_len {
                current_line.push(' ');
                current_line.push_str(word);
            } else {
                lines.push(current_line);
                current_line = word.to_string();
            }
        }
        if !current_line.is_empty() {
            lines.push(current_line);
        }
    }

    if lines.is_empty() {
        lines.push(String::new());
    }

    lines
}

/// Simulated context-aware responses suited for small LLM harness testing
fn generate_mock_response(prompt: &str) -> String {
    let lower = prompt.to_lowercase();
    if lower.contains("small") || lower.contains("model") || lower.contains("llm") {
        "Small LLMs like SmolLM2-1.7B, Qwen2.5-Coder-1.5B, and Phi-3.5-mini achieve incredible token throughput on CPUs with 4-bit quantization, giving you instant responses with zero data leaving your machine.".to_string()
    } else if lower.contains("rust") || lower.contains("code") {
        "For Rust development, small coding models excel at generating type-safe implementations, boilerplate structs, regex parsers, and unit tests directly in your terminal.".to_string()
    } else if lower.contains("speed") || lower.contains("benchmark") || lower.contains("fast") {
        "On modern CPUs with AVX2 or ARM NEON, quantized 1.5B models typically reach 35 to 65 tokens per second in single-user interactive mode.".to_string()
    } else {
        format!(
            "Evaluation response for prompt: \"{}\". (Harness simulation active; real-time streaming inference will connect in upcoming milestones).",
            prompt
        )
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_chat_initial_state() {
        let chat = ChatScreen::new();
        assert!(!chat.messages.is_empty());
        assert_eq!(chat.input_buffer, "");
        assert!(!chat.confirming_clear);
    }

    #[test]
    fn test_input_char_and_backspace() {
        let mut chat = ChatScreen::new();
        chat.handle_char('H');
        chat.handle_char('i');
        assert_eq!(chat.input_buffer, "Hi");
        chat.handle_backspace();
        assert_eq!(chat.input_buffer, "H");
    }

    #[test]
    fn test_send_prompt_creates_user_and_assistant_bubbles() {
        let mut chat = ChatScreen::new();
        let initial_count = chat.messages.len();
        chat.input_buffer = "How fast are small models?".to_string();
        chat.send_prompt();

        assert_eq!(chat.messages.len(), initial_count + 2);
        let user_msg = &chat.messages[chat.messages.len() - 2];
        let assistant_msg = &chat.messages[chat.messages.len() - 1];

        assert_eq!(user_msg.role, MessageRole::User);
        assert_eq!(user_msg.content, "How fast are small models?");
        assert_eq!(assistant_msg.role, MessageRole::Assistant);
        assert!(!assistant_msg.content.is_empty());
        assert!(chat.input_buffer.is_empty());
    }

    #[test]
    fn test_clear_command_triggers_confirmation() {
        let mut chat = ChatScreen::new();
        chat.input_buffer = "/clear".to_string();
        chat.send_prompt();

        assert!(chat.confirming_clear);
        assert!(!chat.messages.is_empty()); // not cleared yet

        // Confirm clear
        chat.confirm_clear(true);
        assert!(!chat.confirming_clear);
        assert!(chat.messages.is_empty());
    }

    #[test]
    fn test_cancel_clear_preserves_messages() {
        let mut chat = ChatScreen::new();
        let count = chat.messages.len();
        chat.input_buffer = "/clear".to_string();
        chat.send_prompt();

        assert!(chat.confirming_clear);
        chat.confirm_clear(false);
        assert!(!chat.confirming_clear);
        assert_eq!(chat.messages.len(), count);
    }

    #[test]
    fn test_empty_prompt_ignored() {
        let mut chat = ChatScreen::new();
        let count = chat.messages.len();
        chat.input_buffer = "   ".to_string();
        chat.send_prompt();
        assert_eq!(chat.messages.len(), count);
    }
}
