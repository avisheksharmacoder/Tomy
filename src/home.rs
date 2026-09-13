use ratatui::{
    Frame,
    layout::{Alignment, Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, BorderType, Borders, List, ListItem, Paragraph},
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HomeOption {
    Chat,
    Editor,
    Code,
    Todo,
    Settings,
}

impl HomeOption {
    pub const ALL: [HomeOption; 5] = [
        HomeOption::Chat,
        HomeOption::Editor,
        HomeOption::Code,
        HomeOption::Todo,
        HomeOption::Settings,
    ];

    pub fn title(&self) -> &'static str {
        match self {
            HomeOption::Chat => "💬 Chat",
            HomeOption::Editor => "💻 Code Editor",
            HomeOption::Code => "⚡ Code Harness",
            HomeOption::Todo => "📝 To-do List",
            HomeOption::Settings => "⚙️ Settings",
        }
    }

    pub fn description(&self) -> &'static str {
        match self {
            HomeOption::Chat => {
                "Conversational interface for small LLM models with prompt templating."
            }
            HomeOption::Editor => {
                "Interactive Python code editor with syntax highlighting, 4-space tabs, and JetBrains Mono rendering."
            }
            HomeOption::Code => {
                "Code generation, review, and debugging harness using coding models."
            }
            HomeOption::Todo => {
                "Track development tasks, experiments, and model evaluation checklists."
            }
            HomeOption::Settings => {
                "Configure local inference backends, model paths, and sampling parameters."
            }
        }
    }
}

pub struct HomeScreen {
    pub selected_index: usize,
}

impl HomeScreen {
    pub fn new() -> Self {
        Self { selected_index: 1 } // default to Code Editor to highlight requested feature
    }

    pub fn next(&mut self) {
        if self.selected_index + 1 < HomeOption::ALL.len() {
            self.selected_index += 1;
        } else {
            self.selected_index = 0;
        }
    }

    pub fn previous(&mut self) {
        if self.selected_index > 0 {
            self.selected_index -= 1;
        } else {
            self.selected_index = HomeOption::ALL.len() - 1;
        }
    }

    pub fn selected_option(&self) -> HomeOption {
        HomeOption::ALL[self.selected_index]
    }

    pub fn render(&self, frame: &mut Frame, area: Rect) {
        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .margin(1)
            .constraints([
                Constraint::Length(5), // Title banner
                Constraint::Length(9), // Options list (5 items)
                Constraint::Length(4), // Description box
                Constraint::Min(0),    // Fill
            ])
            .split(area);

        // 1. Header Banner
        let title_spans = vec![
            Span::styled(
                "⚡ TOMY ",
                Style::default()
                    .fg(Color::Cyan)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::styled(
                "— Small LLM Harness & Workspace",
                Style::default().fg(Color::DarkGray),
            ),
        ];
        let sub_title = Span::styled(
            "Modular terminal workspace designed for lightweight on-device AI models",
            Style::default().fg(Color::Gray),
        );
        let title_paragraph = Paragraph::new(vec![
            Line::from(title_spans),
            Line::from(""),
            Line::from(sub_title),
        ])
        .alignment(Alignment::Center)
        .block(
            Block::default()
                .borders(Borders::BOTTOM)
                .border_style(Style::default().fg(Color::DarkGray)),
        );
        frame.render_widget(title_paragraph, chunks[0]);

        // 2. Options List
        let items: Vec<ListItem> = HomeOption::ALL
            .iter()
            .enumerate()
            .map(|(i, option)| {
                let is_selected = i == self.selected_index;
                let (prefix, style) = if is_selected {
                    (
                        "▶  ",
                        Style::default()
                            .fg(Color::Cyan)
                            .add_modifier(Modifier::BOLD),
                    )
                } else {
                    ("   ", Style::default().fg(Color::White))
                };

                ListItem::new(Line::from(vec![
                    Span::styled(prefix, style),
                    Span::styled(option.title(), style),
                ]))
            })
            .collect();

        let menu_block = Block::default()
            .title(" Main Navigation ")
            .title_alignment(Alignment::Left)
            .borders(Borders::ALL)
            .border_type(BorderType::Rounded)
            .border_style(Style::default().fg(Color::Blue));

        let list = List::new(items).block(menu_block);
        frame.render_widget(list, chunks[1]);

        // 3. Description Box
        let selected_option = self.selected_option();
        let desc_text = vec![Line::from(vec![
            Span::styled(
                "Description: ",
                Style::default()
                    .fg(Color::Yellow)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::styled(
                selected_option.description(),
                Style::default().fg(Color::White),
            ),
        ])];
        let desc_box = Paragraph::new(desc_text).block(
            Block::default()
                .title(" Info ")
                .borders(Borders::ALL)
                .border_type(BorderType::Rounded)
                .border_style(Style::default().fg(Color::DarkGray)),
        );
        frame.render_widget(desc_box, chunks[2]);
    }
}
