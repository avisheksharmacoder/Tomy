use ratatui::{
    Frame,
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, BorderType, Borders, Paragraph},
};

pub struct CodeScreen;

impl CodeScreen {
    pub fn new() -> Self {
        Self
    }

    pub fn commands_hint(&self) -> Vec<(&'static str, &'static str)> {
        vec![("Esc", "Back to Main Menu")]
    }

    pub fn render(&self, frame: &mut Frame, area: Rect) {
        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .margin(1)
            .constraints([
                Constraint::Length(3), // Banner
                Constraint::Min(6),    // Code preview buffer
                Constraint::Length(3), // Command/Instruction prompt
            ])
            .split(area);

        // 1. Status Banner
        let header = Paragraph::new(Line::from(vec![
            Span::styled(
                "Model: ",
                Style::default()
                    .fg(Color::Yellow)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::styled(
                "Qwen2.5-Coder-1.5B (Mock Preview)",
                Style::default().fg(Color::Cyan),
            ),
            Span::styled("  |  Task: ", Style::default().fg(Color::DarkGray)),
            Span::styled("Rust Code Assistant", Style::default().fg(Color::Green)),
        ]))
        .block(
            Block::default()
                .title(" 💻 Code Harness ")
                .borders(Borders::ALL)
                .border_type(BorderType::Rounded)
                .border_style(Style::default().fg(Color::Blue)),
        );
        frame.render_widget(header, chunks[0]);

        // 2. Mock Code Buffer
        let code_lines = vec![
            Line::from(vec![Span::styled(
                "// Generated snippet for small model inference loop",
                Style::default().fg(Color::DarkGray),
            )]),
            Line::from(vec![
                Span::styled(
                    "pub fn ",
                    Style::default()
                        .fg(Color::Magenta)
                        .add_modifier(Modifier::BOLD),
                ),
                Span::styled("infer_step", Style::default().fg(Color::Blue)),
                Span::styled("(prompt: &", Style::default().fg(Color::White)),
                Span::styled("str", Style::default().fg(Color::Yellow)),
                Span::styled(") -> Result<", Style::default().fg(Color::White)),
                Span::styled("String", Style::default().fg(Color::Yellow)),
                Span::styled(", ", Style::default().fg(Color::White)),
                Span::styled("Box<dyn Error>", Style::default().fg(Color::Red)),
                Span::styled("> {", Style::default().fg(Color::White)),
            ]),
            Line::from(vec![
                Span::styled("    println!(", Style::default().fg(Color::White)),
                Span::styled(
                    "\"Executing prompt on small LLM harness: {}\"",
                    Style::default().fg(Color::Green),
                ),
                Span::styled(", prompt);", Style::default().fg(Color::White)),
            ]),
            Line::from(vec![
                Span::styled("    Ok(", Style::default().fg(Color::White)),
                Span::styled(
                    "\"Ready for evaluation.\"",
                    Style::default().fg(Color::Green),
                ),
                Span::styled(".into())", Style::default().fg(Color::White)),
            ]),
            Line::from(vec![Span::styled("}", Style::default().fg(Color::White))]),
        ];

        let code_box = Paragraph::new(code_lines).block(
            Block::default()
                .title(" Code Output / Buffer ")
                .borders(Borders::ALL)
                .border_type(BorderType::Rounded)
                .border_style(Style::default().fg(Color::DarkGray)),
        );
        frame.render_widget(code_box, chunks[1]);

        // 3. Prompt info
        let prompt_box = Paragraph::new(Line::from(Span::styled(
            "(Code harness is in preview mode. Press Esc to return to Home)",
            Style::default().fg(Color::DarkGray),
        )))
        .block(
            Block::default()
                .title(" Instruction ")
                .borders(Borders::ALL)
                .border_type(BorderType::Rounded)
                .border_style(Style::default().fg(Color::DarkGray)),
        );
        frame.render_widget(prompt_box, chunks[2]);
    }
}
