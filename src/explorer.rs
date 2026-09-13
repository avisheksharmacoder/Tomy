use std::collections::HashSet;
use std::fs;
use std::path::{Path, PathBuf};

use crossterm::event::{KeyCode, KeyEvent, KeyEventKind, KeyModifiers};
use ratatui::{
    Frame,
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, BorderType, Borders, Clear, Paragraph},
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FileExplorerAction {
    None,
    OpenFile(PathBuf),
    BackToHome,
}

#[derive(Debug, Clone)]
pub struct TreeEntry {
    pub path: PathBuf,
    pub name: String,
    pub is_dir: bool,
    pub depth: usize,
    pub is_expanded: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ExplorerModal {
    None,
    NewFile {
        input: String,
        error_msg: Option<String>,
    },
    DeleteConfirm {
        target_path: PathBuf,
        target_name: String,
        is_dir: bool,
        step: DeleteConfirmStep,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DeleteConfirmStep {
    First,
    SecondFolderWarning,
}

pub struct FileExplorerScreen {
    pub root_path: PathBuf,
    pub entries: Vec<TreeEntry>,
    pub selected_index: usize,
    pub scroll_offset: usize,
    pub preview_lines: Vec<String>,
    pub preview_file_path: Option<PathBuf>,
    pub preview_tokens: Option<usize>,
    pub newly_created_files: HashSet<PathBuf>,
    pub modal: ExplorerModal,
}

impl FileExplorerScreen {
    pub fn new(root_path: PathBuf) -> Self {
        let mut screen = Self {
            root_path: root_path.clone(),
            entries: Vec::new(),
            selected_index: 0,
            scroll_offset: 0,
            preview_lines: Vec::new(),
            preview_file_path: None,
            preview_tokens: None,
            newly_created_files: HashSet::new(),
            modal: ExplorerModal::None,
        };

        screen.load_directory(&root_path, 0, 0);
        screen.update_preview();
        screen
    }

    /// Loads items from `dir_path` and inserts them into `self.entries` at `insert_idx`
    fn load_directory(&mut self, dir_path: &Path, depth: usize, insert_idx: usize) {
        let mut dir_items: Vec<TreeEntry> = Vec::new();

        if let Ok(read_dir) = fs::read_dir(dir_path) {
            for entry_res in read_dir.flatten() {
                let path = entry_res.path();
                let name = entry_res.file_name().to_string_lossy().to_string();

                // Skip hidden files/folders (e.g. .git, .DS_Store)
                if name.starts_with('.') {
                    continue;
                }

                let is_dir = path.is_dir();
                dir_items.push(TreeEntry {
                    path,
                    name,
                    is_dir,
                    depth,
                    is_expanded: false,
                });
            }
        }

        // Sort: directories first (alphabetical), then files (alphabetical)
        dir_items.sort_by(|a, b| {
            if a.is_dir == b.is_dir {
                a.name.to_lowercase().cmp(&b.name.to_lowercase())
            } else if a.is_dir {
                std::cmp::Ordering::Less
            } else {
                std::cmp::Ordering::Greater
            }
        });

        for item in dir_items.into_iter().rev() {
            self.entries.insert(insert_idx, item);
        }
    }

    /// Reloads the directory tree preserving expanded folders
    pub fn reload_tree(&mut self) {
        let expanded: HashSet<PathBuf> = self
            .entries
            .iter()
            .filter(|e| e.is_dir && e.is_expanded)
            .map(|e| e.path.clone())
            .collect();

        self.entries.clear();
        self.load_tree_recursive(&self.root_path.clone(), 0, &expanded);

        if self.selected_index >= self.entries.len() {
            self.selected_index = self.entries.len().saturating_sub(1);
        }
        self.preview_file_path = None;
        self.update_preview();
    }

    fn load_tree_recursive(&mut self, dir_path: &Path, depth: usize, expanded: &HashSet<PathBuf>) {
        let mut dir_items: Vec<TreeEntry> = Vec::new();

        if let Ok(read_dir) = fs::read_dir(dir_path) {
            for entry_res in read_dir.flatten() {
                let path = entry_res.path();
                let name = entry_res.file_name().to_string_lossy().to_string();

                if name.starts_with('.') {
                    continue;
                }

                let is_dir = path.is_dir();
                let is_expanded = is_dir && expanded.contains(&path);
                dir_items.push(TreeEntry {
                    path,
                    name,
                    is_dir,
                    depth,
                    is_expanded,
                });
            }
        }

        dir_items.sort_by(|a, b| {
            if a.is_dir == b.is_dir {
                a.name.to_lowercase().cmp(&b.name.to_lowercase())
            } else if a.is_dir {
                std::cmp::Ordering::Less
            } else {
                std::cmp::Ordering::Greater
            }
        });

        for item in dir_items {
            let is_dir = item.is_dir;
            let is_expanded = item.is_expanded;
            let path = item.path.clone();
            self.entries.push(item);
            if is_dir && is_expanded {
                self.load_tree_recursive(&path, depth + 1, expanded);
            }
        }
    }

    pub fn selected_entry(&self) -> Option<&TreeEntry> {
        self.entries.get(self.selected_index)
    }

    pub fn previous(&mut self) {
        if self.selected_index > 0 {
            self.selected_index -= 1;
            self.update_preview();
        }
    }

    pub fn next(&mut self) {
        if !self.entries.is_empty() && self.selected_index + 1 < self.entries.len() {
            self.selected_index += 1;
            self.update_preview();
        }
    }

    pub fn toggle_or_open(&mut self) -> FileExplorerAction {
        if self.entries.is_empty() {
            return FileExplorerAction::None;
        }

        let entry = self.entries[self.selected_index].clone();

        if entry.is_dir {
            if entry.is_expanded {
                // Collapse: remove all consecutive children with depth > entry.depth
                self.entries[self.selected_index].is_expanded = false;
                let target_depth = entry.depth;
                let mut remove_count = 0;

                for idx in (self.selected_index + 1)..self.entries.len() {
                    if self.entries[idx].depth > target_depth {
                        remove_count += 1;
                    } else {
                        break;
                    }
                }

                for _ in 0..remove_count {
                    self.entries.remove(self.selected_index + 1);
                }
            } else {
                // Expand: load children and insert below
                self.entries[self.selected_index].is_expanded = true;
                let next_depth = entry.depth + 1;
                let insert_at = self.selected_index + 1;
                self.load_directory(&entry.path, next_depth, insert_at);
            }
            self.update_preview();
            FileExplorerAction::None
        } else {
            // File selected: launch code editor with this file
            FileExplorerAction::OpenFile(entry.path)
        }
    }

    pub fn update_preview(&mut self) {
        let file_to_preview = if let Some(entry) = self.selected_entry() {
            if !entry.is_dir {
                Some(entry.path.clone())
            } else {
                None
            }
        } else {
            None
        };

        if let Some(path) = file_to_preview {
            if self.preview_file_path.as_ref() == Some(&path) {
                return;
            }
            self.preview_file_path = Some(path.clone());
            self.preview_lines = match fs::read_to_string(&path) {
                Ok(content) => content.lines().take(50).map(String::from).collect(),
                Err(_) => vec!["(Binary or unreadable file content)".to_string()],
            };
            self.preview_tokens = crate::token_counter::count_tokens_file(&path).ok();
        } else {
            self.preview_lines.clear();
            self.preview_file_path = None;
            self.preview_tokens = None;
        }
    }

    pub fn handle_key(&mut self, key: KeyEvent) -> FileExplorerAction {
        if key.kind != KeyEventKind::Press {
            return FileExplorerAction::None;
        }

        // 1. Handle active modals keystrokes
        match self.modal {
            ExplorerModal::NewFile {
                ref mut input,
                ref mut error_msg,
            } => {
                match key.code {
                    KeyCode::Esc => {
                        self.modal = ExplorerModal::None;
                        return FileExplorerAction::None;
                    }
                    KeyCode::Backspace => {
                        input.pop();
                        *error_msg = None;
                        return FileExplorerAction::None;
                    }
                    KeyCode::Char(c) if !key.modifiers.contains(KeyModifiers::CONTROL) => {
                        input.push(c);
                        *error_msg = None;
                        return FileExplorerAction::None;
                    }
                    KeyCode::Enter => {
                        // Handled below outside modal borrow
                    }
                    _ => return FileExplorerAction::None,
                }
            }

            ExplorerModal::DeleteConfirm {
                is_dir,
                ref mut step,
                ..
            } => {
                match key.code {
                    KeyCode::Esc | KeyCode::Char('n') | KeyCode::Char('N') => {
                        self.modal = ExplorerModal::None;
                        return FileExplorerAction::None;
                    }
                    KeyCode::Char('y') | KeyCode::Char('Y') => {
                        if is_dir && *step == DeleteConfirmStep::First {
                            // Advance to second warning confirmation
                            *step = DeleteConfirmStep::SecondFolderWarning;
                            return FileExplorerAction::None;
                        }
                        // Handled below outside modal borrow
                    }
                    _ => return FileExplorerAction::None,
                }
            }

            ExplorerModal::None => {}
        }

        // If Enter was pressed in NewFile modal, execute file creation
        if matches!(self.modal, ExplorerModal::NewFile { .. }) && key.code == KeyCode::Enter {
            return self.confirm_new_file();
        }

        // If 'y' or 'Y' was pressed in DeleteConfirm, execute deletion
        if matches!(self.modal, ExplorerModal::DeleteConfirm { .. })
            && (key.code == KeyCode::Char('y') || key.code == KeyCode::Char('Y'))
        {
            return self.confirm_delete();
        }

        // Global key actions when no modal is open
        if key.modifiers.contains(KeyModifiers::CONTROL)
            && (key.code == KeyCode::Char('n') || key.code == KeyCode::Char('N'))
        {
            self.modal = ExplorerModal::NewFile {
                input: String::new(),
                error_msg: None,
            };
            return FileExplorerAction::None;
        }

        match key.code {
            KeyCode::Esc => FileExplorerAction::BackToHome,
            KeyCode::Up | KeyCode::Char('k') => {
                self.previous();
                FileExplorerAction::None
            }
            KeyCode::Down | KeyCode::Char('j') => {
                self.next();
                FileExplorerAction::None
            }
            KeyCode::Enter => self.toggle_or_open(),
            KeyCode::Delete => {
                if let Some(entry) = self.selected_entry() {
                    self.modal = ExplorerModal::DeleteConfirm {
                        target_path: entry.path.clone(),
                        target_name: entry.name.clone(),
                        is_dir: entry.is_dir,
                        step: DeleteConfirmStep::First,
                    };
                }
                FileExplorerAction::None
            }
            _ => FileExplorerAction::None,
        }
    }

    fn confirm_new_file(&mut self) -> FileExplorerAction {
        let input_text = match &self.modal {
            ExplorerModal::NewFile { input, .. } => input.trim().to_string(),
            _ => return FileExplorerAction::None,
        };

        if input_text.is_empty() {
            if let ExplorerModal::NewFile {
                ref mut error_msg, ..
            } = self.modal
            {
                *error_msg = Some("File name cannot be empty".to_string());
            }
            return FileExplorerAction::None;
        }

        // Determine target folder based on selection
        let target_dir = if let Some(entry) = self.selected_entry() {
            if entry.is_dir {
                entry.path.clone()
            } else if let Some(parent) = entry.path.parent() {
                parent.to_path_buf()
            } else {
                self.root_path.clone()
            }
        } else {
            self.root_path.clone()
        };

        let new_file_path = target_dir.join(&input_text);
        if new_file_path.exists() {
            if let ExplorerModal::NewFile {
                ref mut error_msg, ..
            } = self.modal
            {
                *error_msg = Some("A file or folder with this name already exists".to_string());
            }
            return FileExplorerAction::None;
        }

        if let Some(Err(e)) = new_file_path.parent().map(fs::create_dir_all) {
            if let ExplorerModal::NewFile {
                ref mut error_msg, ..
            } = self.modal
            {
                *error_msg = Some(format!("Failed to create directories: {e}"));
            }
            return FileExplorerAction::None;
        }

        match fs::File::create(&new_file_path) {
            Ok(_) => {
                let canonical = new_file_path
                    .canonicalize()
                    .unwrap_or_else(|_| new_file_path.clone());
                self.newly_created_files.insert(canonical.clone());
                self.newly_created_files.insert(new_file_path.clone());

                // Ensure target directory is expanded in view
                for entry in self.entries.iter_mut() {
                    if entry.is_dir && new_file_path.starts_with(&entry.path) {
                        entry.is_expanded = true;
                    }
                }

                self.reload_tree();

                if let Some(pos) = self
                    .entries
                    .iter()
                    .position(|e| e.path == new_file_path || e.path == canonical)
                {
                    self.selected_index = pos;
                }

                self.modal = ExplorerModal::None;
                FileExplorerAction::OpenFile(new_file_path)
            }
            Err(e) => {
                if let ExplorerModal::NewFile {
                    ref mut error_msg, ..
                } = self.modal
                {
                    *error_msg = Some(format!("Error creating file: {e}"));
                }
                FileExplorerAction::None
            }
        }
    }

    fn confirm_delete(&mut self) -> FileExplorerAction {
        let (target, is_dir) = match &self.modal {
            ExplorerModal::DeleteConfirm {
                target_path,
                is_dir,
                ..
            } => (target_path.clone(), *is_dir),
            _ => return FileExplorerAction::None,
        };

        if is_dir {
            let _ = fs::remove_dir_all(&target);
        } else {
            let _ = fs::remove_file(&target);
        }

        self.newly_created_files.remove(&target);
        self.reload_tree();
        self.modal = ExplorerModal::None;
        FileExplorerAction::None
    }

    pub fn commands_hint(&self) -> Vec<(&'static str, &'static str)> {
        vec![
            ("↑/↓ / k/j", "Navigate"),
            ("Enter", "Expand / Open File"),
            ("Ctrl+N", "New File"),
            ("Del", "Delete"),
            ("Esc", "Back to Menu"),
        ]
    }

    pub fn render(&mut self, frame: &mut Frame, area: Rect) {
        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .margin(1)
            .constraints([
                Constraint::Length(3), // Top Path Banner
                Constraint::Min(6),    // 2-Column Main Workspace
            ])
            .split(area);

        // 1. Top Path Banner
        let root_str = self.root_path.to_string_lossy();
        let header_paragraph = Paragraph::new(Line::from(vec![
            Span::styled(
                "📁 Project Explorer: ",
                Style::default()
                    .fg(Color::Cyan)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::styled(root_str, Style::default().fg(Color::White)),
            Span::styled(
                "  │  [Ctrl+N] New File  │  [Del] Delete  │  [Enter] Open  │  [Esc] Menu",
                Style::default().fg(Color::DarkGray),
            ),
        ]))
        .block(
            Block::default()
                .title(" Workspace Explorer ")
                .borders(Borders::ALL)
                .border_type(BorderType::Rounded)
                .border_style(Style::default().fg(Color::Blue)),
        );
        frame.render_widget(header_paragraph, chunks[0]);

        // 2. 2-Column Layout (Left: 45% Tree, Right: 55% Preview & Details)
        let col_chunks = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([
                Constraint::Percentage(45), // File tree
                Constraint::Percentage(55), // Preview pane
            ])
            .split(chunks[1]);

        let tree_area = col_chunks[0];
        let preview_area = col_chunks[1];

        // Ensure selected index is visible in tree scroll window
        let visible_tree_rows = tree_area.height.saturating_sub(2) as usize;
        if self.selected_index < self.scroll_offset {
            self.scroll_offset = self.selected_index;
        } else if self.selected_index >= self.scroll_offset + visible_tree_rows
            && visible_tree_rows > 0
        {
            self.scroll_offset = self.selected_index - visible_tree_rows + 1;
        }

        // Render Tree Lines with Strict Color Hierarchy:
        // - Folders: Blue (Color::Cyan / Color::Blue)
        // - Top-level Files (depth == 0): Orange (Color::Rgb(255, 140, 0))
        // - Files within expanded folders (depth > 0): Purple (Color::Magenta / Color::Rgb(180, 100, 255))
        // - Newly Created Files: 50% Blue Background (Color::Rgb(30, 65, 125))
        let mut tree_lines: Vec<Line> = Vec::new();

        if self.entries.is_empty() {
            tree_lines.push(Line::from(Span::styled(
                "  (Directory is empty)",
                Style::default().fg(Color::DarkGray),
            )));
        } else {
            for (idx, item) in self
                .entries
                .iter()
                .enumerate()
                .skip(self.scroll_offset)
                .take(visible_tree_rows)
            {
                let is_selected = idx == self.selected_index;
                let is_newly_created = self.newly_created_files.contains(&item.path)
                    || item
                        .path
                        .canonicalize()
                        .map(|p| self.newly_created_files.contains(&p))
                        .unwrap_or(false);

                let cursor = if is_selected { "▶ " } else { "  " };
                let cursor_style = if is_selected {
                    Style::default()
                        .fg(Color::Yellow)
                        .add_modifier(Modifier::BOLD)
                } else {
                    Style::default().fg(Color::DarkGray)
                };

                // Indentation prefix
                let indent_spans = "  ".repeat(item.depth);

                let (icon, item_style) = if item.is_dir {
                    let folder_icon = if item.is_expanded {
                        "▼ 📁 "
                    } else {
                        "▶ 📁 "
                    };
                    let style = Style::default()
                        .fg(Color::Cyan)
                        .add_modifier(if is_selected {
                            Modifier::BOLD
                        } else {
                            Modifier::empty()
                        });
                    (folder_icon, style)
                } else if item.depth == 0 {
                    let style =
                        Style::default()
                            .fg(Color::Rgb(255, 140, 0))
                            .add_modifier(if is_selected {
                                Modifier::BOLD
                            } else {
                                Modifier::empty()
                            });
                    ("  📄 ", style)
                } else {
                    let style = Style::default()
                        .fg(Color::Magenta)
                        .add_modifier(if is_selected {
                            Modifier::BOLD
                        } else {
                            Modifier::empty()
                        });
                    ("  📄 ", style)
                };

                let mut row_spans = vec![
                    Span::styled(cursor, cursor_style),
                    Span::raw(indent_spans),
                    Span::styled(icon, item_style),
                    Span::styled(&item.name, item_style),
                ];

                if is_newly_created {
                    row_spans.push(Span::styled(
                        " [NEW]",
                        Style::default()
                            .fg(Color::Rgb(140, 205, 255))
                            .add_modifier(Modifier::BOLD),
                    ));
                }

                // Apply background: 50% blue background for newly created files
                if is_newly_created {
                    let bg_color = if is_selected {
                        Color::Rgb(45, 95, 175)
                    } else {
                        Color::Rgb(25, 60, 120)
                    };
                    for span in row_spans.iter_mut() {
                        span.style = span.style.bg(bg_color);
                    }
                } else if is_selected {
                    for span in row_spans.iter_mut() {
                        span.style = span.style.bg(Color::Rgb(25, 30, 45));
                    }
                }

                tree_lines.push(Line::from(row_spans));
            }
        }

        let tree_block = Block::default()
            .title(" 📁 Project Tree ")
            .borders(Borders::ALL)
            .border_type(BorderType::Rounded)
            .border_style(Style::default().fg(Color::Blue));

        frame.render_widget(Paragraph::new(tree_lines).block(tree_block), tree_area);

        // Render Right Column: File Preview / Details Pane
        let mut preview_spans: Vec<Line> = Vec::new();

        if let Some(entry) = self.selected_entry() {
            if entry.is_dir {
                preview_spans.push(Line::from(vec![
                    Span::styled(
                        "Directory: ",
                        Style::default()
                            .fg(Color::Yellow)
                            .add_modifier(Modifier::BOLD),
                    ),
                    Span::styled(
                        &entry.name,
                        Style::default()
                            .fg(Color::Cyan)
                            .add_modifier(Modifier::BOLD),
                    ),
                ]));
                preview_spans.push(Line::from(vec![
                    Span::styled("Path: ", Style::default().fg(Color::DarkGray)),
                    Span::styled(
                        entry.path.to_string_lossy(),
                        Style::default().fg(Color::White),
                    ),
                ]));
                preview_spans.push(Line::from(""));
                preview_spans.push(Line::from(Span::styled(
                    if entry.is_expanded {
                        "Status: Expanded (Press Enter to collapse)"
                    } else {
                        "Status: Collapsed (Press Enter to expand folder)"
                    },
                    Style::default().fg(Color::Gray),
                )));
            } else {
                let file_size_str = if let Ok(meta) = fs::metadata(&entry.path) {
                    let bytes = meta.len();
                    if bytes < 1024 {
                        format!("{} B", bytes)
                    } else {
                        format!("{:.1} KB", bytes as f64 / 1024.0)
                    }
                } else {
                    "Unknown".to_string()
                };

                let token_str = match self.preview_tokens {
                    Some(tokens) => crate::token_counter::format_token_count(tokens),
                    None => "-".to_string(),
                };

                // Exact row displaying both Size and Tokens side-by-side
                preview_spans.push(Line::from(vec![
                    Span::styled(
                        "File: ",
                        Style::default()
                            .fg(Color::Yellow)
                            .add_modifier(Modifier::BOLD),
                    ),
                    Span::styled(
                        &entry.name,
                        Style::default()
                            .fg(Color::Rgb(255, 140, 0))
                            .add_modifier(Modifier::BOLD),
                    ),
                    Span::styled("  │  Size: ", Style::default().fg(Color::DarkGray)),
                    Span::styled(file_size_str, Style::default().fg(Color::White)),
                    Span::styled("  │  Tokens: ", Style::default().fg(Color::DarkGray)),
                    Span::styled(
                        token_str,
                        Style::default()
                            .fg(Color::Cyan)
                            .add_modifier(Modifier::BOLD),
                    ),
                ]));
                preview_spans.push(Line::from(vec![
                    Span::styled("Path: ", Style::default().fg(Color::DarkGray)),
                    Span::styled(
                        entry.path.to_string_lossy(),
                        Style::default().fg(Color::Gray),
                    ),
                ]));
                preview_spans.push(Line::from(Span::styled(
                    "───────────────────────────────────────────────────────",
                    Style::default().fg(Color::DarkGray),
                )));

                // Preview top lines
                for (idx, line_str) in self
                    .preview_lines
                    .iter()
                    .take(preview_area.height.saturating_sub(6) as usize)
                    .enumerate()
                {
                    preview_spans.push(Line::from(vec![
                        Span::styled(
                            format!("{:>3} │ ", idx + 1),
                            Style::default().fg(Color::DarkGray),
                        ),
                        Span::styled(line_str, Style::default().fg(Color::White)),
                    ]));
                }
            }
        } else {
            preview_spans.push(Line::from(Span::styled(
                "Select a file or folder from the left pane to view details.",
                Style::default().fg(Color::DarkGray),
            )));
        }

        let preview_block = Block::default()
            .title(" 📄 File Inspector & Preview ")
            .borders(Borders::ALL)
            .border_type(BorderType::Rounded)
            .border_style(Style::default().fg(Color::DarkGray));

        frame.render_widget(
            Paragraph::new(preview_spans).block(preview_block),
            preview_area,
        );

        // 3. Render Modal Dialogs (if active)
        match self.modal {
            ExplorerModal::NewFile {
                ref input,
                ref error_msg,
            } => {
                let modal_area = centered_rect(54, 7, area);
                frame.render_widget(Clear, modal_area);

                let mut lines = vec![
                    Line::from(vec![Span::styled(
                        "Enter file name or path: ",
                        Style::default().fg(Color::White),
                    )]),
                    Line::from(vec![
                        Span::styled(
                            "▶ ",
                            Style::default()
                                .fg(Color::Yellow)
                                .add_modifier(Modifier::BOLD),
                        ),
                        Span::styled(
                            input,
                            Style::default()
                                .fg(Color::Cyan)
                                .add_modifier(Modifier::BOLD),
                        ),
                        Span::styled("█", Style::default().fg(Color::White)),
                    ]),
                    Line::from(""),
                    Line::from(vec![
                        Span::styled(
                            "[Enter] ",
                            Style::default()
                                .fg(Color::Green)
                                .add_modifier(Modifier::BOLD),
                        ),
                        Span::styled("Create & Open    ", Style::default().fg(Color::DarkGray)),
                        Span::styled(
                            "[Esc] ",
                            Style::default().fg(Color::Red).add_modifier(Modifier::BOLD),
                        ),
                        Span::styled("Cancel", Style::default().fg(Color::DarkGray)),
                    ]),
                ];

                if let Some(err) = error_msg {
                    lines.push(Line::from(Span::styled(
                        format!("⚠ {}", err),
                        Style::default().fg(Color::Red).add_modifier(Modifier::BOLD),
                    )));
                }

                let block = Block::default()
                    .title(" 📄 Create New File (Ctrl+N) ")
                    .borders(Borders::ALL)
                    .border_type(BorderType::Rounded)
                    .border_style(Style::default().fg(Color::Cyan));

                frame.render_widget(Paragraph::new(lines).block(block), modal_area);
            }

            ExplorerModal::DeleteConfirm {
                ref target_name,
                is_dir,
                step,
                ..
            } => {
                let modal_area = centered_rect(65, 8, area);
                frame.render_widget(Clear, modal_area);

                let (title, border_color, question_line) = match step {
                    DeleteConfirmStep::First => {
                        if is_dir {
                            (
                                " 📁 Delete Folder ",
                                Color::Yellow,
                                Line::from(vec![
                                    Span::styled(
                                        "Are you sure you want to delete folder '",
                                        Style::default().fg(Color::White),
                                    ),
                                    Span::styled(
                                        target_name,
                                        Style::default()
                                            .fg(Color::Yellow)
                                            .add_modifier(Modifier::BOLD),
                                    ),
                                    Span::styled("'? [y/N]", Style::default().fg(Color::White)),
                                ]),
                            )
                        } else {
                            (
                                " 🗑️ Delete File ",
                                Color::Yellow,
                                Line::from(vec![
                                    Span::styled(
                                        "Are you sure you want to delete file '",
                                        Style::default().fg(Color::White),
                                    ),
                                    Span::styled(
                                        target_name,
                                        Style::default()
                                            .fg(Color::Yellow)
                                            .add_modifier(Modifier::BOLD),
                                    ),
                                    Span::styled("'? [y/N]", Style::default().fg(Color::White)),
                                ]),
                            )
                        }
                    }
                    DeleteConfirmStep::SecondFolderWarning => (
                        " ⚠️ Critical: Folder Deletion Confirmation ",
                        Color::Red,
                        Line::from(vec![Span::styled(
                            "This folder contains important files, are you absolutely sure to delete it? [y/N]",
                            Style::default().fg(Color::Red).add_modifier(Modifier::BOLD),
                        )]),
                    ),
                };

                let lines = vec![
                    Line::from(""),
                    question_line,
                    Line::from(""),
                    Line::from(vec![
                        Span::styled(
                            "[y] ",
                            Style::default()
                                .fg(Color::Green)
                                .add_modifier(Modifier::BOLD),
                        ),
                        Span::styled("Confirm Deletion    ", Style::default().fg(Color::DarkGray)),
                        Span::styled(
                            "[n / Esc] ",
                            Style::default().fg(Color::Red).add_modifier(Modifier::BOLD),
                        ),
                        Span::styled("Cancel", Style::default().fg(Color::DarkGray)),
                    ]),
                ];

                let block = Block::default()
                    .title(title)
                    .borders(Borders::ALL)
                    .border_type(BorderType::Rounded)
                    .border_style(Style::default().fg(border_color));

                frame.render_widget(Paragraph::new(lines).block(block), modal_area);
            }

            ExplorerModal::None => {}
        }
    }
}

/// Helper to center a rectangular popup dialog on screen
fn centered_rect(percent_x: u16, height_lines: u16, r: Rect) -> Rect {
    let popup_layout = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length((r.height.saturating_sub(height_lines)) / 2),
            Constraint::Length(height_lines),
            Constraint::Min(0),
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
    fn test_file_explorer_load_and_colors() {
        let temp_dir = std::env::temp_dir().join("tomy_explorer_test");
        let _ = fs::remove_dir_all(&temp_dir);
        fs::create_dir_all(&temp_dir).unwrap();

        // Create a subfolder and top-level file
        let sub_folder = temp_dir.join("models");
        fs::create_dir(&sub_folder).unwrap();

        let top_file = temp_dir.join("main.py");
        fs::write(&top_file, "print('top level')").unwrap();

        let nested_file = sub_folder.join("engine.py");
        fs::write(&nested_file, "print('nested')").unwrap();

        let mut explorer = FileExplorerScreen::new(temp_dir.clone());
        assert!(!explorer.entries.is_empty());

        // Verify folder is blue (is_dir == true)
        let folder_item = explorer
            .entries
            .iter()
            .find(|e| e.name == "models")
            .unwrap();
        assert!(folder_item.is_dir);

        // Verify top-level file is orange (is_dir == false && depth == 0)
        let file_item = explorer
            .entries
            .iter()
            .find(|e| e.name == "main.py")
            .unwrap();
        assert!(!file_item.is_dir);
        assert_eq!(file_item.depth, 0);

        // Expand folder
        explorer.selected_index = explorer
            .entries
            .iter()
            .position(|e| e.name == "models")
            .unwrap();
        let action = explorer.toggle_or_open();
        assert_eq!(action, FileExplorerAction::None);

        // Verify nested file is purple (is_dir == false && depth > 0)
        let nested_item = explorer
            .entries
            .iter()
            .find(|e| e.name == "engine.py")
            .unwrap();
        assert!(!nested_item.is_dir);
        assert_eq!(nested_item.depth, 1);

        // Select nested file and press Enter -> returns OpenFile action
        explorer.selected_index = explorer
            .entries
            .iter()
            .position(|e| e.name == "engine.py")
            .unwrap();
        let open_action = explorer.toggle_or_open();
        assert_eq!(open_action, FileExplorerAction::OpenFile(nested_file));

        let _ = fs::remove_dir_all(temp_dir);
    }

    #[test]
    fn test_create_file_ctrl_n_and_blue_marking() {
        let temp_dir = std::env::temp_dir().join("tomy_explorer_new_file_test");
        let _ = fs::remove_dir_all(&temp_dir);
        fs::create_dir_all(&temp_dir).unwrap();

        let mut explorer = FileExplorerScreen::new(temp_dir.clone());

        // Press Ctrl+N to open modal
        let action = explorer.handle_key(KeyEvent::new(KeyCode::Char('n'), KeyModifiers::CONTROL));
        assert_eq!(action, FileExplorerAction::None);
        assert!(matches!(explorer.modal, ExplorerModal::NewFile { .. }));

        // Type "agent.py"
        for c in "agent.py".chars() {
            explorer.handle_key(KeyEvent::new(KeyCode::Char(c), KeyModifiers::NONE));
        }

        // Press Enter to create
        let action = explorer.handle_key(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE));
        let expected_path = temp_dir.join("agent.py");
        assert_eq!(action, FileExplorerAction::OpenFile(expected_path.clone()));
        assert!(expected_path.exists());

        // Check that newly created file is marked in newly_created_files
        assert!(explorer.newly_created_files.contains(&expected_path));

        let _ = fs::remove_dir_all(temp_dir);
    }

    #[test]
    fn test_folder_deletion_double_confirmation() {
        let temp_dir = std::env::temp_dir().join("tomy_explorer_delete_test");
        let _ = fs::remove_dir_all(&temp_dir);
        fs::create_dir_all(&temp_dir).unwrap();

        let folder_to_del = temp_dir.join("important_data");
        fs::create_dir(&folder_to_del).unwrap();
        fs::write(folder_to_del.join("weights.bin"), b"12345").unwrap();

        let mut explorer = FileExplorerScreen::new(temp_dir.clone());
        explorer.selected_index = explorer
            .entries
            .iter()
            .position(|e| e.name == "important_data")
            .unwrap();

        // Press Delete key
        explorer.handle_key(KeyEvent::new(KeyCode::Delete, KeyModifiers::NONE));
        assert!(matches!(
            explorer.modal,
            ExplorerModal::DeleteConfirm {
                step: DeleteConfirmStep::First,
                is_dir: true,
                ..
            }
        ));

        // Press 'y' -> Advances to step 2
        explorer.handle_key(KeyEvent::new(KeyCode::Char('y'), KeyModifiers::NONE));
        assert!(matches!(
            explorer.modal,
            ExplorerModal::DeleteConfirm {
                step: DeleteConfirmStep::SecondFolderWarning,
                is_dir: true,
                ..
            }
        ));
        assert!(folder_to_del.exists()); // Not deleted yet!

        // Press 'y' again -> Now deleted
        explorer.handle_key(KeyEvent::new(KeyCode::Char('y'), KeyModifiers::NONE));
        assert_eq!(explorer.modal, ExplorerModal::None);
        assert!(!folder_to_del.exists());

        let _ = fs::remove_dir_all(temp_dir);
    }

    #[test]
    fn test_explorer_preview_tokens() {
        let temp_dir = std::env::temp_dir().join("tomy_explorer_tokens_test");
        let _ = fs::remove_dir_all(&temp_dir);
        fs::create_dir_all(&temp_dir).unwrap();

        let top_file = temp_dir.join("main.py");
        fs::write(&top_file, "def calculate(a, b):\n    return a + b\n").unwrap();

        let mut explorer = FileExplorerScreen::new(temp_dir.clone());
        explorer.selected_index = explorer
            .entries
            .iter()
            .position(|e| e.name == "main.py")
            .unwrap();
        explorer.preview_file_path = None;
        explorer.update_preview();

        assert!(explorer.preview_tokens.is_some());
        assert!(explorer.preview_tokens.unwrap() > 0);

        let _ = fs::remove_dir_all(temp_dir);
    }
}
