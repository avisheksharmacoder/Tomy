use ratatui::{
    Frame,
    layout::{Alignment, Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, BorderType, Borders, Clear, List, ListItem, Paragraph},
};

#[derive(Debug, Clone)]
pub struct TodoItem {
    pub text: String,
    pub completed: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TodoInputMode {
    Normal,
    Adding,
}

pub struct TodoScreen {
    pub items: Vec<TodoItem>,
    pub selected_index: usize,
    pub mode: TodoInputMode,
    pub input_buffer: String,
}

impl TodoScreen {
    pub fn new() -> Self {
        Self {
            items: vec![
                TodoItem {
                    text: "Setup local GGUF model loader for SmolLM2-1.7B".to_string(),
                    completed: true,
                },
                TodoItem {
                    text: "Implement prompt formatting harness for ChatML".to_string(),
                    completed: false,
                },
                TodoItem {
                    text: "Test CPU inference latency with batch size 1".to_string(),
                    completed: false,
                },
                TodoItem {
                    text: "Add token generation speed metrics (tokens/sec)".to_string(),
                    completed: false,
                },
            ],
            selected_index: 0,
            mode: TodoInputMode::Normal,
            input_buffer: String::new(),
        }
    }

    pub fn next(&mut self) {
        if !self.items.is_empty() && self.selected_index + 1 < self.items.len() {
            self.selected_index += 1;
        }
    }

    pub fn previous(&mut self) {
        if self.selected_index > 0 {
            self.selected_index -= 1;
        }
    }

    pub fn toggle_done(&mut self) {
        if let Some(item) = self.items.get_mut(self.selected_index) {
            item.completed = !item.completed;
        }
    }

    pub fn start_adding(&mut self) {
        self.mode = TodoInputMode::Adding;
        self.input_buffer.clear();
    }

    pub fn cancel_adding(&mut self) {
        self.mode = TodoInputMode::Normal;
        self.input_buffer.clear();
    }

    pub fn commit_task(&mut self) {
        let trimmed = self.input_buffer.trim();
        if !trimmed.is_empty() {
            self.items.push(TodoItem {
                text: trimmed.to_string(),
                completed: false,
            });
            self.selected_index = self.items.len() - 1;
        }
        self.mode = TodoInputMode::Normal;
        self.input_buffer.clear();
    }

    pub fn delete_selected(&mut self) {
        if !self.items.is_empty() {
            self.items.remove(self.selected_index);
            if self.selected_index >= self.items.len() && !self.items.is_empty() {
                self.selected_index = self.items.len() - 1;
            }
        }
    }

    pub fn handle_input_char(&mut self, c: char) {
        self.input_buffer.push(c);
    }

    pub fn handle_input_backspace(&mut self) {
        self.input_buffer.pop();
    }

    pub fn commands_hint(&self) -> Vec<(&'static str, &'static str)> {
        match self.mode {
            TodoInputMode::Normal => vec![
                ("a", "Add Task"),
                ("Space/Enter", "Toggle Done"),
                ("d/Del", "Delete"),
                ("↑/↓ / k/j", "Navigate"),
                ("Esc", "Main Menu"),
            ],
            TodoInputMode::Adding => vec![
                ("Enter", "Confirm & Save"),
                ("Esc", "Cancel"),
                ("Backspace", "Delete Char"),
            ],
        }
    }

    pub fn render(&self, frame: &mut Frame, area: Rect) {
        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .margin(1)
            .constraints([
                Constraint::Length(3), // Summary bar
                Constraint::Min(5),    // List area
            ])
            .split(area);

        // 1. Stats Summary Bar
        let total = self.items.len();
        let completed = self.items.iter().filter(|i| i.completed).count();
        let pending = total.saturating_sub(completed);

        let stats_line = Line::from(vec![
            Span::styled(
                "Tasks Overview: ",
                Style::default()
                    .fg(Color::Yellow)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::styled(
                format!("Total: {}  |  ", total),
                Style::default().fg(Color::White),
            ),
            Span::styled(
                format!("Done: {}  |  ", completed),
                Style::default()
                    .fg(Color::Green)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::styled(
                format!("Pending: {}", pending),
                Style::default().fg(Color::Red),
            ),
        ]);

        let summary_block = Paragraph::new(stats_line).block(
            Block::default()
                .title(" 📝 To-do List ")
                .title_alignment(Alignment::Left)
                .borders(Borders::ALL)
                .border_type(BorderType::Rounded)
                .border_style(Style::default().fg(Color::Cyan)),
        );
        frame.render_widget(summary_block, chunks[0]);

        // 2. Task Items List
        let list_items: Vec<ListItem> = if self.items.is_empty() {
            vec![ListItem::new(Line::from(Span::styled(
                "  No tasks yet! Press 'a' to add your first task.",
                Style::default().fg(Color::DarkGray),
            )))]
        } else {
            self.items
                .iter()
                .enumerate()
                .map(|(idx, item)| {
                    let is_selected = idx == self.selected_index;
                    let cursor = if is_selected { "▶ " } else { "  " };

                    // Styling rule:
                    // Done = bold green color
                    // Not done = red color
                    let (status_icon, status_style) = if item.completed {
                        (
                            "[✓] ",
                            Style::default()
                                .fg(Color::Green)
                                .add_modifier(Modifier::BOLD),
                        )
                    } else {
                        ("[ ] ", Style::default().fg(Color::Red))
                    };

                    let cursor_style = if is_selected {
                        Style::default()
                            .fg(Color::Cyan)
                            .add_modifier(Modifier::BOLD)
                    } else {
                        Style::default().fg(Color::White)
                    };

                    let text_style = if item.completed {
                        Style::default()
                            .fg(Color::Green)
                            .add_modifier(Modifier::BOLD)
                    } else {
                        Style::default().fg(Color::Red)
                    };

                    ListItem::new(Line::from(vec![
                        Span::styled(cursor, cursor_style),
                        Span::styled(status_icon, status_style),
                        Span::styled(&item.text, text_style),
                    ]))
                })
                .collect()
        };

        let list_block = Block::default()
            .title(" Tasks ")
            .borders(Borders::ALL)
            .border_type(BorderType::Rounded)
            .border_style(Style::default().fg(Color::DarkGray));

        let list_widget = List::new(list_items).block(list_block);
        frame.render_widget(list_widget, chunks[1]);

        // 3. Render Add Task Modal/Overlay if in Adding mode
        if self.mode == TodoInputMode::Adding {
            let popup_area = centered_rect(65, 25, area);
            frame.render_widget(Clear, popup_area);

            let input_paragraph = Paragraph::new(vec![
                Line::from(Span::styled(
                    "Enter task description:",
                    Style::default().fg(Color::Yellow),
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
                        if self.input_buffer.is_empty() {
                            "(type here...)"
                        } else {
                            &self.input_buffer
                        },
                        if self.input_buffer.is_empty() {
                            Style::default().fg(Color::DarkGray)
                        } else {
                            Style::default()
                                .fg(Color::White)
                                .add_modifier(Modifier::BOLD)
                        },
                    ),
                    Span::styled("█", Style::default().fg(Color::Cyan)), // blinking cursor simulation
                ]),
            ])
            .block(
                Block::default()
                    .title(" ➕ Add New Task ")
                    .title_alignment(Alignment::Center)
                    .borders(Borders::ALL)
                    .border_type(BorderType::Double)
                    .border_style(Style::default().fg(Color::Green)),
            );

            frame.render_widget(input_paragraph, popup_area);
        }
    }
}

/// Helper function to create a centered rectangle of given percent width and height
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
    fn test_initial_state() {
        let screen = TodoScreen::new();
        assert!(!screen.items.is_empty());
        assert_eq!(screen.mode, TodoInputMode::Normal);
        assert_eq!(screen.selected_index, 0);
    }

    #[test]
    fn test_toggle_done() {
        let mut screen = TodoScreen::new();
        let initial_status = screen.items[0].completed;
        screen.toggle_done();
        assert_eq!(screen.items[0].completed, !initial_status);
        screen.toggle_done();
        assert_eq!(screen.items[0].completed, initial_status);
    }

    #[test]
    fn test_add_task() {
        let mut screen = TodoScreen::new();
        let count_before = screen.items.len();
        screen.start_adding();
        assert_eq!(screen.mode, TodoInputMode::Adding);

        screen.handle_input_char('N');
        screen.handle_input_char('e');
        screen.handle_input_char('w');
        assert_eq!(screen.input_buffer, "New");

        screen.handle_input_backspace();
        assert_eq!(screen.input_buffer, "Ne");

        screen.handle_input_char('w');
        screen.handle_input_char('!');
        screen.commit_task();

        assert_eq!(screen.mode, TodoInputMode::Normal);
        assert_eq!(screen.items.len(), count_before + 1);
        let last = screen.items.last().unwrap();
        assert_eq!(last.text, "New!");
        assert!(!last.completed);
        assert_eq!(screen.selected_index, screen.items.len() - 1);
    }

    #[test]
    fn test_cancel_add_task() {
        let mut screen = TodoScreen::new();
        let count_before = screen.items.len();
        screen.start_adding();
        screen.handle_input_char('X');
        screen.cancel_adding();
        assert_eq!(screen.mode, TodoInputMode::Normal);
        assert_eq!(screen.items.len(), count_before);
    }

    #[test]
    fn test_delete_task() {
        let mut screen = TodoScreen::new();
        let initial_count = screen.items.len();
        screen.selected_index = 0;
        screen.delete_selected();
        assert_eq!(screen.items.len(), initial_count - 1);
    }

    #[test]
    fn test_navigation_bounds() {
        let mut screen = TodoScreen::new();
        screen.previous(); // at 0, should stay 0
        assert_eq!(screen.selected_index, 0);

        for _ in 0..10 {
            screen.next();
        }
        assert_eq!(screen.selected_index, screen.items.len() - 1);
    }
}
