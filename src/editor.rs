use std::path::{Path, PathBuf};

use crossterm::event::{KeyCode, KeyEvent, KeyModifiers, MouseEvent, MouseEventKind};
use ratatui::{
    Frame,
    layout::{Alignment, Constraint, Direction, Layout, Position, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, BorderType, Borders, Clear, Paragraph},
};

use crate::completion::CompletionState;
use crate::ruff_service::FileSymbolIndex;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MultilineStringState {
    None,
    DoubleTriple, // """
    SingleTriple, // '''
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TextPosition {
    pub row: usize,
    pub col: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SelectionRange {
    pub start: TextPosition,
    pub end: TextPosition,
}

impl SelectionRange {
    pub fn normalized(pos1: TextPosition, pos2: TextPosition) -> Self {
        if (pos1.row, pos1.col) <= (pos2.row, pos2.col) {
            Self {
                start: pos1,
                end: pos2,
            }
        } else {
            Self {
                start: pos2,
                end: pos1,
            }
        }
    }

    pub fn is_empty(&self) -> bool {
        self.start == self.end
    }
}

pub struct EditorScreen {
    pub lines: Vec<String>,
    pub cursor_row: usize,
    pub cursor_col: usize,
    pub preferred_col: usize,
    pub scroll_row: usize,
    pub scroll_col: usize,
    pub is_modified: bool,
    pub font_size: u16,
    pub font_size_hud_ticks: u8,
    pub current_file_path: Option<PathBuf>,
    pub opened_from_explorer: bool,
    pub runner_child: Option<std::process::Child>,
    pub current_runner_path: Option<PathBuf>,
    pub save_toast_ticks: u8,
    pub run_toast_ticks: u8,
    pub symbol_index: FileSymbolIndex,
    pub completion: CompletionState,
    pub last_ruff_message: Option<String>,
    pub token_count: usize,
    pub selection: Option<SelectionRange>,
    pub mouse_selecting: bool,
    pub last_code_area: Option<Rect>,
    pub copy_toast_ticks: u8,
}

impl EditorScreen {
    pub fn new() -> Self {
        let initial_code = get_default_python_code();
        let lines: Vec<String> = initial_code.lines().map(String::from).collect();
        let symbol_index = FileSymbolIndex::extract(&lines);
        let mut screen = Self {
            lines: if lines.is_empty() {
                vec![String::new()]
            } else {
                lines
            },
            cursor_row: 0,
            cursor_col: 0,
            preferred_col: 0,
            scroll_row: 0,
            scroll_col: 0,
            is_modified: false,
            font_size: 14,
            font_size_hud_ticks: 0,
            current_file_path: None,
            opened_from_explorer: false,
            runner_child: None,
            current_runner_path: None,
            save_toast_ticks: 0,
            run_toast_ticks: 0,
            symbol_index,
            completion: CompletionState::new(),
            last_ruff_message: None,
            token_count: 0,
            selection: None,
            mouse_selecting: false,
            last_code_area: None,
            copy_toast_ticks: 0,
        };
        screen.update_token_count();
        screen
    }

    pub fn update_token_count(&mut self) {
        let content = self.lines.join("\n");
        self.token_count = crate::token_counter::count_tokens_str(&content);
    }

    pub fn screen_coords_to_text_pos(&self, column: u16, row: u16) -> Option<TextPosition> {
        let code_area = self.last_code_area?;
        if row <= code_area.y || row >= code_area.bottom().saturating_sub(1) {
            return None;
        }
        if column <= code_area.x || column >= code_area.right().saturating_sub(1) {
            return None;
        }

        let total_lines = self.lines.len();
        let gutter_digits = format!("{}", total_lines).len().max(3);
        let gutter_total_width = gutter_digits + 3; // " 123 │ "

        let content_start_x = code_area.x + 1 + gutter_total_width as u16;
        let screen_y = (row - (code_area.y + 1)) as usize;
        let line_row = self.scroll_row + screen_y;

        if line_row >= self.lines.len() {
            let last_r = self.lines.len().saturating_sub(1);
            let last_c = self
                .lines
                .get(last_r)
                .map(|l| l.chars().count())
                .unwrap_or(0);
            return Some(TextPosition {
                row: last_r,
                col: last_c,
            });
        }

        let line_len = self.lines[line_row].chars().count();
        let line_col = if column < content_start_x {
            0
        } else {
            let screen_x = (column - content_start_x) as usize;
            (self.scroll_col + screen_x).min(line_len)
        };

        Some(TextPosition {
            row: line_row,
            col: line_col,
        })
    }

    pub fn get_selected_text(&self) -> Option<String> {
        let sel = self.selection.as_ref()?;
        if sel.is_empty() {
            return None;
        }
        let norm = SelectionRange::normalized(sel.start, sel.end);
        let mut result = Vec::new();

        for row in norm.start.row..=norm.end.row.min(self.lines.len().saturating_sub(1)) {
            let line = &self.lines[row];
            let chars: Vec<char> = line.chars().collect();
            let start_col = if row == norm.start.row {
                norm.start.col.min(chars.len())
            } else {
                0
            };
            let end_col = if row == norm.end.row {
                norm.end.col.min(chars.len())
            } else {
                chars.len()
            };

            if start_col <= end_col {
                let chunk: String = chars[start_col..end_col].iter().collect();
                result.push(chunk);
            }
        }

        Some(result.join("\n"))
    }

    pub fn delete_selection(&mut self) -> bool {
        let sel = match self.selection.take() {
            Some(s) if !s.is_empty() => SelectionRange::normalized(s.start, s.end),
            _ => {
                self.selection = None;
                return false;
            }
        };

        let start_row = sel.start.row.min(self.lines.len().saturating_sub(1));
        let end_row = sel.end.row.min(self.lines.len().saturating_sub(1));

        if start_row == end_row {
            let line = &mut self.lines[start_row];
            let start_byte = char_to_byte_index(line, sel.start.col);
            let end_byte = char_to_byte_index(line, sel.end.col);
            if start_byte < end_byte && end_byte <= line.len() {
                line.drain(start_byte..end_byte);
            }
        } else {
            let first_line_keep: String = {
                let line = &self.lines[start_row];
                let start_byte = char_to_byte_index(line, sel.start.col);
                line[..start_byte].to_string()
            };
            let last_line_keep: String = {
                let line = &self.lines[end_row];
                let end_byte = char_to_byte_index(line, sel.end.col);
                line[end_byte..].to_string()
            };

            self.lines[start_row] = format!("{}{}", first_line_keep, last_line_keep);

            if end_row > start_row {
                self.lines.drain((start_row + 1)..=end_row);
            }
        }

        if self.lines.is_empty() {
            self.lines.push(String::new());
        }

        self.cursor_row = start_row;
        self.cursor_col = sel.start.col.min(self.lines[start_row].chars().count());
        self.preferred_col = self.cursor_col;
        self.is_modified = true;
        self.completion.dismiss();
        true
    }

    pub fn select_all(&mut self) {
        if self.lines.is_empty() {
            return;
        }
        let last_row = self.lines.len() - 1;
        let last_col = self.lines[last_row].chars().count();
        self.selection = Some(SelectionRange {
            start: TextPosition { row: 0, col: 0 },
            end: TextPosition {
                row: last_row,
                col: last_col,
            },
        });
        self.cursor_row = last_row;
        self.cursor_col = last_col;
        self.preferred_col = last_col;
    }

    pub fn copy_selection_or_all(&mut self) {
        let text = if let Some(selected) = self.get_selected_text() {
            if !selected.is_empty() {
                selected
            } else {
                self.lines.join("\n")
            }
        } else {
            self.lines.join("\n")
        };

        crate::runner::copy_text_to_clipboard(&text);
        self.copy_toast_ticks = 30;
    }

    pub fn move_word_left(&mut self) {
        self.selection = None;
        if self.cursor_col > 0 {
            if self.cursor_row >= self.lines.len() {
                return;
            }
            let line = &self.lines[self.cursor_row];
            let chars: Vec<char> = line.chars().collect();
            let mut col = self.cursor_col.min(chars.len());

            while col > 0 && chars[col - 1].is_whitespace() {
                col -= 1;
            }

            if col > 0 {
                let is_word = chars[col - 1].is_alphanumeric() || chars[col - 1] == '_';
                if is_word {
                    while col > 0 && (chars[col - 1].is_alphanumeric() || chars[col - 1] == '_') {
                        col -= 1;
                    }
                } else {
                    while col > 0
                        && !chars[col - 1].is_whitespace()
                        && !(chars[col - 1].is_alphanumeric() || chars[col - 1] == '_')
                    {
                        col -= 1;
                    }
                }
            }

            self.cursor_col = col;
            self.preferred_col = col;
        } else if self.cursor_row > 0 {
            self.cursor_row -= 1;
            self.cursor_col = self.lines[self.cursor_row].chars().count();
            self.preferred_col = self.cursor_col;
        }
        self.completion.dismiss();
    }

    pub fn move_word_right(&mut self) {
        self.selection = None;
        if self.cursor_row >= self.lines.len() {
            return;
        }
        let line = &self.lines[self.cursor_row];
        let chars: Vec<char> = line.chars().collect();
        let mut col = self.cursor_col;

        if col < chars.len() {
            let is_word = chars[col].is_alphanumeric() || chars[col] == '_';
            let is_ws = chars[col].is_whitespace();

            if is_ws {
                while col < chars.len() && chars[col].is_whitespace() {
                    col += 1;
                }
            } else if is_word {
                while col < chars.len() && (chars[col].is_alphanumeric() || chars[col] == '_') {
                    col += 1;
                }
                while col < chars.len() && chars[col].is_whitespace() {
                    col += 1;
                }
            } else {
                while col < chars.len()
                    && !chars[col].is_whitespace()
                    && !(chars[col].is_alphanumeric() || chars[col] == '_')
                {
                    col += 1;
                }
                while col < chars.len() && chars[col].is_whitespace() {
                    col += 1;
                }
            }

            self.cursor_col = col;
            self.preferred_col = col;
        } else if self.cursor_row + 1 < self.lines.len() {
            self.cursor_row += 1;
            self.cursor_col = 0;
            self.preferred_col = 0;
        }
        self.completion.dismiss();
    }

    pub fn delete_word_backward(&mut self) {
        if self.delete_selection() {
            return;
        }

        if self.cursor_col > 0 {
            if self.cursor_row >= self.lines.len() {
                return;
            }
            let line = &mut self.lines[self.cursor_row];
            let chars: Vec<char> = line.chars().collect();
            let end_col = self.cursor_col.min(chars.len());
            let mut col = end_col;

            while col > 0 && chars[col - 1].is_whitespace() {
                col -= 1;
            }

            if col > 0 {
                let is_word = chars[col - 1].is_alphanumeric() || chars[col - 1] == '_';
                if is_word {
                    while col > 0 && (chars[col - 1].is_alphanumeric() || chars[col - 1] == '_') {
                        col -= 1;
                    }
                } else {
                    while col > 0
                        && !chars[col - 1].is_whitespace()
                        && !(chars[col - 1].is_alphanumeric() || chars[col - 1] == '_')
                    {
                        col -= 1;
                    }
                }
            }

            let start_byte = char_to_byte_index(line, col);
            let end_byte = char_to_byte_index(line, end_col);
            if start_byte < end_byte && end_byte <= line.len() {
                line.drain(start_byte..end_byte);
            }

            self.cursor_col = col;
            self.preferred_col = col;
            self.is_modified = true;
            self.completion.dismiss();
        } else if self.cursor_row > 0 {
            self.handle_backspace();
        }
    }

    pub fn is_runner_active(&mut self) -> bool {
        if let Some(ref mut child) = self.runner_child {
            match child.try_wait() {
                Ok(Some(_)) => {
                    self.runner_child = None;
                    self.current_runner_path = None;
                    false
                }
                Ok(None) => true,
                Err(_) => {
                    self.runner_child = None;
                    self.current_runner_path = None;
                    false
                }
            }
        } else {
            false
        }
    }

    pub fn open_file(&mut self, path: &Path) {
        if let Ok(content) = std::fs::read_to_string(path) {
            let file_lines: Vec<String> = content.lines().map(String::from).collect();
            self.lines = if file_lines.is_empty() {
                vec![String::new()]
            } else {
                file_lines
            };
            self.current_file_path = Some(path.to_path_buf());
            self.cursor_row = 0;
            self.cursor_col = 0;
            self.preferred_col = 0;
            self.scroll_row = 0;
            self.scroll_col = 0;
            self.is_modified = false;
            self.symbol_index = FileSymbolIndex::extract(&self.lines);
            self.completion.dismiss();
            self.update_token_count();
        }
    }

    pub fn save_file(&mut self) -> bool {
        let is_python = self
            .current_file_path
            .as_ref()
            .and_then(|p| p.extension())
            .map(|ext| ext == "py" || ext == "pyw")
            .unwrap_or(true);

        if is_python {
            let content = self.lines.join("\n");
            let res = crate::ruff_service::process_python_code(
                &content,
                self.current_file_path.as_deref(),
            );

            if res.parse_error.is_none() {
                let formatted = res.formatted_code;
                let mut new_lines: Vec<String> = formatted.lines().map(String::from).collect();
                if new_lines.is_empty() {
                    new_lines.push(String::new());
                }
                self.lines = new_lines;
                self.cursor_row = self.cursor_row.min(self.lines.len().saturating_sub(1));
                self.cursor_col = self
                    .cursor_col
                    .min(self.lines[self.cursor_row].chars().count());
                self.preferred_col = self.cursor_col;
                self.symbol_index = FileSymbolIndex::extract(&self.lines);

                if res.fixes_applied > 0 {
                    self.last_ruff_message = Some(format!(
                        "✔ FORMATTED & LINTED ({} fixes applied)",
                        res.fixes_applied
                    ));
                } else if !res.diagnostics.is_empty() {
                    self.last_ruff_message = Some(format!(
                        "✔ FORMATTED ({} lint warnings)",
                        res.diagnostics.len()
                    ));
                } else {
                    self.last_ruff_message = Some("✔ FORMATTED & LINTED WITH RUFF".to_string());
                }
            } else if let Some(ref err) = res.parse_error {
                self.last_ruff_message = Some(format!("⚠ Saved (Syntax issue: {})", err));
            }
        }

        if let Some(ref path) = self.current_file_path {
            let mut content = self.lines.join("\n");
            if !content.is_empty() && !content.ends_with('\n') {
                content.push('\n');
            }
            if std::fs::write(path, content).is_ok() {
                self.is_modified = false;
                self.save_toast_ticks = 35;
                self.update_token_count();
                return true;
            }
        } else {
            self.is_modified = false;
            self.save_toast_ticks = 35;
            self.update_token_count();
            return true;
        }
        false
    }

    pub fn increase_font_size(&mut self) {
        if self.font_size < 28 {
            self.font_size += 1;
            self.font_size_hud_ticks = 30;
        }
    }

    pub fn decrease_font_size(&mut self) {
        if self.font_size > 8 {
            self.font_size -= 1;
            self.font_size_hud_ticks = 30;
        }
    }

    pub fn commands_hint(&self) -> Vec<(&'static str, &'static str)> {
        if self.completion.active {
            return vec![
                ("Tab / Enter", "Accept Suggestion"),
                ("↑/↓", "Navigate"),
                ("Esc", "Dismiss"),
            ];
        }

        let mut hints = vec![
            ("Ctrl+A", "Select All"),
            ("Ctrl+C", "Copy"),
            ("Ctrl+S", "Save & Ruff"),
            ("Ctrl+←/→", "Skip Word"),
            ("Ctrl+Bksp", "Del Word"),
            ("Ctrl +/-", "Font Size ±1"),
            ("Tab", "4 Spaces"),
            ("Shift+Tab", "Unindent"),
            ("Enter", "Auto-Indent"),
            ("↑/↓/←/→", "Navigate"),
        ];

        if self.runner_child.is_some() || self.current_file_path.is_some() {
            hints.insert(2, ("Ctrl+R", "Run Code"));
        }

        if self.opened_from_explorer {
            hints.push(("Esc", "Back to Explorer"));
        } else {
            hints.push(("Esc", "Back to Menu"));
        }

        hints
    }

    pub fn handle_key(&mut self, key: KeyEvent) {
        // If completion popup is active, prioritize completion navigation & acceptance
        if self.completion.active {
            if key.modifiers.contains(KeyModifiers::CONTROL) {
                match key.code {
                    KeyCode::Char('n') | KeyCode::Char('N') => {
                        self.completion.select_next();
                        return;
                    }
                    KeyCode::Char('p') | KeyCode::Char('P') => {
                        self.completion.select_prev();
                        return;
                    }
                    _ => {}
                }
            }

            match key.code {
                KeyCode::Tab | KeyCode::Enter => {
                    if self.cursor_row < self.lines.len()
                        && self
                            .completion
                            .apply_selected(&mut self.lines[self.cursor_row], &mut self.cursor_col)
                    {
                        self.preferred_col = self.cursor_col;
                        self.is_modified = true;
                        self.symbol_index = FileSymbolIndex::extract(&self.lines);
                    }
                    return;
                }
                KeyCode::Down => {
                    self.completion.select_next();
                    return;
                }
                KeyCode::Up => {
                    self.completion.select_prev();
                    return;
                }
                KeyCode::Esc => {
                    self.completion.dismiss();
                    return;
                }
                _ => {}
            }
        }

        // Control key shortcuts
        if key.modifiers.contains(KeyModifiers::CONTROL) {
            match key.code {
                KeyCode::Char('a') | KeyCode::Char('A') => {
                    self.select_all();
                    return;
                }
                KeyCode::Char('c') | KeyCode::Char('C') => {
                    self.copy_selection_or_all();
                    return;
                }
                KeyCode::Left => {
                    self.move_word_left();
                    return;
                }
                KeyCode::Right => {
                    self.move_word_right();
                    return;
                }
                KeyCode::Backspace
                | KeyCode::Char('w')
                | KeyCode::Char('W')
                | KeyCode::Char('h')
                | KeyCode::Char('H')
                | KeyCode::Char('\x08')
                | KeyCode::Char('\x7f') => {
                    self.delete_word_backward();
                    if self.is_modified {
                        self.update_token_count();
                    }
                    return;
                }
                KeyCode::Char('+') | KeyCode::Char('=') => {
                    self.increase_font_size();
                    return;
                }
                KeyCode::Char('-') | KeyCode::Char('_') => {
                    self.decrease_font_size();
                    return;
                }
                KeyCode::Char('s') | KeyCode::Char('S') => {
                    self.save_file();
                    return;
                }
                KeyCode::Char('r') | KeyCode::Char('R') => {
                    self.save_file();
                    self.run_toast_ticks = 30;
                    return;
                }
                _ => {}
            }
        }

        match key.code {
            KeyCode::Esc => {
                // Return to home handled by App
            }
            KeyCode::Left => self.move_left(),
            KeyCode::Right => self.move_right(),
            KeyCode::Up => self.move_up(),
            KeyCode::Down => self.move_down(),
            KeyCode::Home => self.move_home(),
            KeyCode::End => self.move_end(),
            KeyCode::PageUp => self.page_up(20),
            KeyCode::PageDown => self.page_down(20),

            // Tab inserts 4 spaces
            KeyCode::Tab => {
                self.insert_tab();
            }

            // Shift+Tab or BackTab unindents
            KeyCode::BackTab => {
                self.unindent();
            }

            // Enter splits line and auto-indents
            KeyCode::Enter => {
                self.insert_newline();
                self.completion.dismiss();
            }

            // Backspace deletes character or 4-space indent
            KeyCode::Backspace => {
                self.handle_backspace();
            }

            // Delete key removes char at cursor
            KeyCode::Delete => {
                self.handle_delete();
            }

            KeyCode::Char(c)
                if !key.modifiers.contains(KeyModifiers::CONTROL)
                    && !key.modifiers.contains(KeyModifiers::ALT) =>
            {
                self.insert_char(c);
            }

            _ => {}
        }

        if self.is_modified {
            self.update_token_count();
        }
    }

    pub fn handle_mouse(&mut self, mouse: MouseEvent) {
        match mouse.kind {
            MouseEventKind::Down(crossterm::event::MouseButton::Left) => {
                if let Some(pos) = self.screen_coords_to_text_pos(mouse.column, mouse.row) {
                    self.cursor_row = pos.row;
                    self.cursor_col = pos.col;
                    self.preferred_col = pos.col;
                    self.mouse_selecting = true;
                    self.selection = Some(SelectionRange {
                        start: pos,
                        end: pos,
                    });
                    self.completion.dismiss();
                }
            }
            MouseEventKind::Drag(crossterm::event::MouseButton::Left) => {
                if !self.mouse_selecting {
                    return;
                }
                if let Some(pos) = self.screen_coords_to_text_pos(mouse.column, mouse.row) {
                    self.cursor_row = pos.row;
                    self.cursor_col = pos.col;
                    self.preferred_col = pos.col;
                    if let Some(ref mut sel) = self.selection {
                        sel.end = pos;
                    }
                }
            }
            MouseEventKind::Up(crossterm::event::MouseButton::Left) => {
                self.mouse_selecting = false;
                if self.selection.as_ref().is_some_and(|s| s.is_empty()) {
                    self.selection = None;
                }
            }
            MouseEventKind::ScrollUp => {
                self.scroll_row = self.scroll_row.saturating_sub(3);
            }
            MouseEventKind::ScrollDown if self.scroll_row + 3 < self.lines.len() => {
                self.scroll_row += 3;
            }
            _ => {}
        }
    }

    pub fn insert_char(&mut self, c: char) {
        if self.selection.is_some() {
            self.delete_selection();
        }

        if self.cursor_row >= self.lines.len() {
            self.lines.push(String::new());
        }

        let line = &mut self.lines[self.cursor_row];
        let byte_index = char_to_byte_index(line, self.cursor_col);
        line.insert(byte_index, c);

        self.cursor_col += 1;
        self.preferred_col = self.cursor_col;
        self.is_modified = true;

        if c.is_alphanumeric() || c == '_' {
            self.completion.update(
                &self.lines[self.cursor_row],
                self.cursor_col,
                &self.symbol_index,
            );
        } else {
            self.completion.dismiss();
            if c == ' ' || c == ':' || c == '=' {
                self.symbol_index = FileSymbolIndex::extract(&self.lines);
            }
        }
    }

    pub fn insert_tab(&mut self) {
        if self.selection.is_some() {
            self.delete_selection();
        }

        // Tab inserts 4 spaces
        const TAB_SPACES: &str = "    ";
        if self.cursor_row >= self.lines.len() {
            self.lines.push(String::new());
        }

        let line = &mut self.lines[self.cursor_row];
        let byte_index = char_to_byte_index(line, self.cursor_col);
        line.insert_str(byte_index, TAB_SPACES);

        self.cursor_col += 4;
        self.preferred_col = self.cursor_col;
        self.is_modified = true;
        self.completion.dismiss();
    }

    pub fn unindent(&mut self) {
        if self.cursor_row >= self.lines.len() {
            return;
        }

        let line = &mut self.lines[self.cursor_row];
        let leading_spaces = line.chars().take_while(|&c| c == ' ').count();
        let remove_count = leading_spaces.min(4);

        if remove_count > 0 {
            for _ in 0..remove_count {
                line.remove(0);
            }
            self.cursor_col = self.cursor_col.saturating_sub(remove_count);
            self.preferred_col = self.cursor_col;
            self.is_modified = true;
        }
        self.completion.dismiss();
    }

    pub fn insert_newline(&mut self) {
        if self.selection.is_some() {
            self.delete_selection();
        }

        if self.cursor_row >= self.lines.len() {
            self.lines.push(String::new());
        }

        let current_line = self.lines[self.cursor_row].clone();
        let split_byte = char_to_byte_index(&current_line, self.cursor_col);

        let left = current_line[..split_byte].to_string();
        let right = current_line[split_byte..].to_string();

        // Calculate leading indentation of current line
        let leading_spaces = left.chars().take_while(|&c| c == ' ').count();
        let mut new_indent = " ".repeat(leading_spaces);

        // If line ends with ':' (common in Python for def/class/if/etc), increase indent by 4
        if left.trim_end().ends_with(':') {
            new_indent.push_str("    ");
        }

        let next_line = format!("{}{}", new_indent, right);
        self.lines[self.cursor_row] = left;
        self.lines.insert(self.cursor_row + 1, next_line);

        self.cursor_row += 1;
        self.cursor_col = new_indent.len();
        self.preferred_col = self.cursor_col;
        self.is_modified = true;
        self.completion.dismiss();
    }

    pub fn handle_backspace(&mut self) {
        if self.delete_selection() {
            return;
        }

        if self.cursor_row >= self.lines.len() {
            return;
        }

        if self.cursor_col == 0 {
            // Merge with previous line if row > 0
            if self.cursor_row > 0 {
                let current_line = self.lines.remove(self.cursor_row);
                self.cursor_row -= 1;
                let prev_len = self.lines[self.cursor_row].chars().count();
                self.lines[self.cursor_row].push_str(&current_line);
                self.cursor_col = prev_len;
                self.preferred_col = self.cursor_col;
                self.is_modified = true;
            }
            if self.cursor_row < self.lines.len() {
                self.completion.update(
                    &self.lines[self.cursor_row],
                    self.cursor_col,
                    &self.symbol_index,
                );
            } else {
                self.completion.dismiss();
            }
            return;
        }

        let line = &mut self.lines[self.cursor_row];

        // Smart 4-space backspace: if preceding characters are 4 spaces and cursor is aligned to 4
        if self.cursor_col >= 4 {
            let byte_idx = char_to_byte_index(line, self.cursor_col);
            let prev_byte_idx = char_to_byte_index(line, self.cursor_col - 4);
            if &line[prev_byte_idx..byte_idx] == "    " {
                line.drain(prev_byte_idx..byte_idx);
                self.cursor_col -= 4;
                self.preferred_col = self.cursor_col;
                self.is_modified = true;
                if self.cursor_row < self.lines.len() {
                    self.completion.update(
                        &self.lines[self.cursor_row],
                        self.cursor_col,
                        &self.symbol_index,
                    );
                }
                return;
            }
        }

        // Single character backspace
        let byte_idx = char_to_byte_index(line, self.cursor_col);
        let prev_byte_idx = char_to_byte_index(line, self.cursor_col - 1);
        line.drain(prev_byte_idx..byte_idx);
        self.cursor_col -= 1;
        self.preferred_col = self.cursor_col;
        self.is_modified = true;

        if self.cursor_row < self.lines.len() {
            self.completion.update(
                &self.lines[self.cursor_row],
                self.cursor_col,
                &self.symbol_index,
            );
        } else {
            self.completion.dismiss();
        }
    }

    pub fn handle_delete(&mut self) {
        if self.delete_selection() {
            return;
        }

        if self.cursor_row >= self.lines.len() {
            return;
        }

        let line_char_count = self.lines[self.cursor_row].chars().count();
        if self.cursor_col < line_char_count {
            let line = &mut self.lines[self.cursor_row];
            let byte_idx = char_to_byte_index(line, self.cursor_col);
            let next_byte_idx = char_to_byte_index(line, self.cursor_col + 1);
            line.drain(byte_idx..next_byte_idx);
            self.is_modified = true;
        } else if self.cursor_row + 1 < self.lines.len() {
            // Merge next line into current
            let next_line = self.lines.remove(self.cursor_row + 1);
            self.lines[self.cursor_row].push_str(&next_line);
            self.is_modified = true;
        }
        self.completion.dismiss();
    }

    pub fn move_left(&mut self) {
        self.selection = None;
        if self.cursor_col > 0 {
            self.cursor_col -= 1;
        } else if self.cursor_row > 0 {
            self.cursor_row -= 1;
            self.cursor_col = self.lines[self.cursor_row].chars().count();
        }
        self.preferred_col = self.cursor_col;
        self.completion.dismiss();
    }

    pub fn move_right(&mut self) {
        self.selection = None;
        if self.cursor_row < self.lines.len() {
            let line_len = self.lines[self.cursor_row].chars().count();
            if self.cursor_col < line_len {
                self.cursor_col += 1;
            } else if self.cursor_row + 1 < self.lines.len() {
                self.cursor_row += 1;
                self.cursor_col = 0;
            }
        }
        self.preferred_col = self.cursor_col;
        self.completion.dismiss();
    }

    pub fn move_up(&mut self) {
        self.selection = None;
        if self.cursor_row > 0 {
            self.cursor_row -= 1;
            let target_line_len = self.lines[self.cursor_row].chars().count();
            self.cursor_col = self.preferred_col.min(target_line_len);
        }
        self.completion.dismiss();
    }

    pub fn move_down(&mut self) {
        self.selection = None;
        if self.cursor_row + 1 < self.lines.len() {
            self.cursor_row += 1;
            let target_line_len = self.lines[self.cursor_row].chars().count();
            self.cursor_col = self.preferred_col.min(target_line_len);
        }
        self.completion.dismiss();
    }

    pub fn move_home(&mut self) {
        self.selection = None;
        if self.cursor_row < self.lines.len() {
            let line = &self.lines[self.cursor_row];
            let first_non_ws = line.chars().take_while(|c| c.is_whitespace()).count();
            if self.cursor_col == first_non_ws {
                self.cursor_col = 0;
            } else {
                self.cursor_col = first_non_ws;
            }
            self.preferred_col = self.cursor_col;
        }
        self.completion.dismiss();
    }

    pub fn move_end(&mut self) {
        self.selection = None;
        if self.cursor_row < self.lines.len() {
            self.cursor_col = self.lines[self.cursor_row].chars().count();
            self.preferred_col = self.cursor_col;
        }
        self.completion.dismiss();
    }

    pub fn page_up(&mut self, count: usize) {
        self.selection = None;
        self.cursor_row = self.cursor_row.saturating_sub(count);
        let target_line_len = self.lines[self.cursor_row].chars().count();
        self.cursor_col = self.preferred_col.min(target_line_len);
        self.completion.dismiss();
    }

    pub fn page_down(&mut self, count: usize) {
        self.selection = None;
        self.cursor_row = (self.cursor_row + count).min(self.lines.len().saturating_sub(1));
        let target_line_len = self.lines[self.cursor_row].chars().count();
        self.cursor_col = self.preferred_col.min(target_line_len);
        self.completion.dismiss();
    }

    pub fn render(&mut self, frame: &mut Frame, area: Rect) {
        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .margin(0)
            .constraints([
                Constraint::Length(3), // Top header / info bar
                Constraint::Min(6),    // Code canvas with gutter
                Constraint::Length(1), // Editor status bar
            ])
            .split(area);

        let header_area = chunks[0];
        let code_area = chunks[1];
        let status_area = chunks[2];

        // 1. Top Header Bar
        let mod_indicator = if self.is_modified {
            Span::styled(
                " ● Modified",
                Style::default()
                    .fg(Color::Yellow)
                    .add_modifier(Modifier::BOLD),
            )
        } else {
            Span::styled(" ✓ Saved", Style::default().fg(Color::Green))
        };

        let file_name = if let Some(ref path) = self.current_file_path {
            path.file_name()
                .and_then(|s| s.to_str())
                .unwrap_or("script.py")
        } else {
            "engine.py"
        };

        let lang_str = if file_name.ends_with(".rs") {
            "Rust"
        } else if file_name.ends_with(".sh") || file_name.ends_with(".bash") {
            "Bash"
        } else if file_name.ends_with(".json") {
            "JSON"
        } else if file_name.ends_with(".md") {
            "Markdown"
        } else {
            "Python 3.12"
        };

        let header_text = Line::from(vec![
            Span::styled(
                format!(" 📝 {} ", file_name),
                Style::default()
                    .fg(Color::White)
                    .add_modifier(Modifier::BOLD),
            ),
            mod_indicator,
            Span::styled("   │   Language: ", Style::default().fg(Color::DarkGray)),
            Span::styled(
                lang_str,
                Style::default()
                    .fg(Color::Cyan)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::styled("   │   Font: ", Style::default().fg(Color::DarkGray)),
            Span::styled(
                format!("JetBrains Mono NL ({} pt)", self.font_size),
                Style::default()
                    .fg(Color::Magenta)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::styled("   │   Indent: ", Style::default().fg(Color::DarkGray)),
            Span::styled("4 Spaces (Tab)", Style::default().fg(Color::Yellow)),
        ]);

        let header_block = Block::default()
            .title(" ⚡ Tomy High-Performance Code Editor ")
            .borders(Borders::ALL)
            .border_type(BorderType::Rounded)
            .border_style(Style::default().fg(Color::Blue));

        frame.render_widget(Paragraph::new(header_text).block(header_block), header_area);

        // 2. Main Editor Area: compute viewport bounds
        self.last_code_area = Some(code_area);
        let visible_height = code_area.height.saturating_sub(2) as usize; // inside border
        let inner_width = code_area.width.saturating_sub(2) as usize;

        // Auto scroll vertically to keep cursor visible
        if self.cursor_row < self.scroll_row {
            self.scroll_row = self.cursor_row;
        } else if self.cursor_row >= self.scroll_row + visible_height && visible_height > 0 {
            self.scroll_row = self.cursor_row - visible_height + 1;
        }

        // Line number gutter width (at least 3 chars wide + separator)
        let total_lines = self.lines.len();
        let gutter_digits = format!("{}", total_lines).len().max(3);
        let gutter_total_width = gutter_digits + 3; // " 123 │ "

        let code_view_width = inner_width.saturating_sub(gutter_total_width);

        // Auto scroll horizontally
        if self.cursor_col < self.scroll_col {
            self.scroll_col = self.cursor_col;
        } else if self.cursor_col >= self.scroll_col + code_view_width && code_view_width > 0 {
            self.scroll_col = self.cursor_col - code_view_width + 1;
        }

        // Calculate multiline state up to the visible range
        let multiline_states =
            compute_multiline_states(&self.lines, self.scroll_row + visible_height);

        // Build rendered lines for visible slice only (Zero-allocation / O(visible) -> "fast as fuck")
        let mut rendered_lines: Vec<Line> = Vec::with_capacity(visible_height);

        for screen_y in 0..visible_height {
            let line_idx = self.scroll_row + screen_y;
            if line_idx >= self.lines.len() {
                // Beyond end of file: draw subtle tilde like Vim
                rendered_lines.push(Line::from(vec![Span::styled(
                    format!("{:>width$} │ ", "~", width = gutter_digits),
                    Style::default().fg(Color::DarkGray),
                )]));
                continue;
            }

            let is_active_line = line_idx == self.cursor_row;
            let gutter_style = if is_active_line {
                Style::default()
                    .fg(Color::Yellow)
                    .add_modifier(Modifier::BOLD)
            } else {
                Style::default().fg(Color::DarkGray)
            };

            let gutter_span = Span::styled(
                format!("{:>width$} ", line_idx + 1, width = gutter_digits),
                gutter_style,
            );
            let sep_span = Span::styled("│ ", Style::default().fg(Color::DarkGray));

            let raw_line = &self.lines[line_idx];
            let ml_state = multiline_states
                .get(line_idx)
                .copied()
                .unwrap_or(MultilineStringState::None);

            // Tokenize and highlight line
            let highlighted_spans = highlight_python_line(raw_line, ml_state, self.font_size);

            // Apply selection highlight
            let selected_spans =
                apply_selection_style(highlighted_spans, line_idx, self.selection.as_ref());

            // Apply horizontal scroll offset
            let scrolled_spans = apply_horizontal_scroll(&selected_spans, self.scroll_col);

            let mut full_line_spans = vec![gutter_span, sep_span];
            full_line_spans.extend(scrolled_spans);

            rendered_lines.push(Line::from(full_line_spans));
        }

        let editor_block = Block::default()
            .borders(Borders::ALL)
            .border_type(BorderType::Rounded)
            .border_style(Style::default().fg(Color::DarkGray));

        frame.render_widget(
            Paragraph::new(rendered_lines).block(editor_block),
            code_area,
        );

        // Position hardware terminal cursor exactly on active character
        let cursor_screen_x = code_area.x
            + 1
            + gutter_total_width as u16
            + (self.cursor_col.saturating_sub(self.scroll_col)) as u16;
        let cursor_screen_y =
            code_area.y + 1 + (self.cursor_row.saturating_sub(self.scroll_row)) as u16;

        if cursor_screen_x < code_area.right() - 1 && cursor_screen_y < code_area.bottom() - 1 {
            frame.set_cursor_position(Position::new(cursor_screen_x, cursor_screen_y));
        }

        // 3. Status Bar
        let char_count: usize = self.lines.iter().map(|l| l.chars().count()).sum();
        let mut status_spans = vec![
            Span::styled(
                " [EDITING] ",
                Style::default()
                    .fg(Color::Black)
                    .bg(Color::Green)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::styled("  Ln ", Style::default().fg(Color::DarkGray)),
            Span::styled(
                format!("{}", self.cursor_row + 1),
                Style::default()
                    .fg(Color::White)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::styled(", Col ", Style::default().fg(Color::DarkGray)),
            Span::styled(
                format!("{}", self.cursor_col + 1),
                Style::default()
                    .fg(Color::White)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::styled("   │   Lines: ", Style::default().fg(Color::DarkGray)),
            Span::styled(
                format!("{}", total_lines),
                Style::default().fg(Color::White),
            ),
            Span::styled("   │   Chars: ", Style::default().fg(Color::DarkGray)),
            Span::styled(format!("{}", char_count), Style::default().fg(Color::White)),
            Span::styled("   │   Tokens: ", Style::default().fg(Color::DarkGray)),
            Span::styled(
                crate::token_counter::format_token_count(self.token_count),
                Style::default()
                    .fg(Color::Cyan)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::styled("   │   Font: ", Style::default().fg(Color::DarkGray)),
            Span::styled(
                format!("{} pt", self.font_size),
                Style::default()
                    .fg(Color::Yellow)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::styled("   │   Tab: ", Style::default().fg(Color::DarkGray)),
            Span::styled("4 Spaces", Style::default().fg(Color::Yellow)),
            Span::styled("   │   UTF-8", Style::default().fg(Color::DarkGray)),
        ];

        if let Some(sel_text) = self.get_selected_text() {
            let sel_chars = sel_text.chars().count();
            status_spans.push(Span::styled(
                format!("   │   Selected: {} chars", sel_chars),
                Style::default()
                    .fg(Color::LightCyan)
                    .add_modifier(Modifier::BOLD),
            ));
        }

        if self.copy_toast_ticks > 0 {
            self.copy_toast_ticks = self.copy_toast_ticks.saturating_sub(1);
            status_spans.push(Span::styled(
                "    ✔ COPIED TO CLIPBOARD ",
                Style::default()
                    .fg(Color::Black)
                    .bg(Color::Cyan)
                    .add_modifier(Modifier::BOLD),
            ));
        }

        if self.save_toast_ticks > 0 {
            self.save_toast_ticks = self.save_toast_ticks.saturating_sub(1);
            let msg = self
                .last_ruff_message
                .as_deref()
                .unwrap_or("✔ SAVED TO DISK");
            let bg_color = if msg.starts_with('⚠') {
                Color::Yellow
            } else {
                Color::Green
            };
            status_spans.push(Span::styled(
                format!("    {} ", msg),
                Style::default()
                    .fg(Color::Black)
                    .bg(bg_color)
                    .add_modifier(Modifier::BOLD),
            ));
        }

        if self.run_toast_ticks > 0 {
            self.run_toast_ticks = self.run_toast_ticks.saturating_sub(1);
            status_spans.push(Span::styled(
                "    ⚡ EXECUTING SCRIPT IN RUNNER... ",
                Style::default()
                    .fg(Color::Black)
                    .bg(Color::Yellow)
                    .add_modifier(Modifier::BOLD),
            ));
        }

        if self.completion.active {
            status_spans.push(Span::styled(
                "    [Tab/Enter: Insert  ↑/↓: Nav  Esc: Dismiss] ",
                Style::default()
                    .fg(Color::Cyan)
                    .add_modifier(Modifier::BOLD),
            ));
        }

        frame.render_widget(Paragraph::new(Line::from(status_spans)), status_area);

        // 4. Render Floating Autocomplete Suggestions Popup
        if self.completion.active && !self.completion.suggestions.is_empty() {
            let visible_count = self
                .completion
                .suggestions
                .len()
                .min(self.completion.max_visible);
            let popup_height = (visible_count as u16) + 2;
            let max_item_len = self
                .completion
                .suggestions
                .iter()
                .map(|s| s.name.chars().count())
                .max()
                .unwrap_or(12);
            let popup_width = (max_item_len as u16 + 14).clamp(24, 40);

            // Place below cursor if possible, otherwise above
            let mut popup_y = cursor_screen_y + 1;
            if popup_y + popup_height >= code_area.bottom() {
                popup_y = cursor_screen_y.saturating_sub(popup_height);
            }

            let mut popup_x = cursor_screen_x;
            if popup_x + popup_width >= code_area.right() {
                popup_x = code_area.right().saturating_sub(popup_width);
            }

            let completion_rect = Rect::new(popup_x, popup_y, popup_width, popup_height);
            frame.render_widget(Clear, completion_rect);

            let start_idx = self.completion.scroll_offset;
            let end_idx = (start_idx + visible_count).min(self.completion.suggestions.len());

            let mut comp_lines = Vec::new();
            for idx in start_idx..end_idx {
                let item = &self.completion.suggestions[idx];
                let is_selected = idx == self.completion.selected_index;

                let badge = match item.kind {
                    crate::ruff_service::SymbolKind::Keyword => Span::styled(
                        "[kw] ",
                        Style::default()
                            .fg(Color::Magenta)
                            .add_modifier(Modifier::BOLD),
                    ),
                    crate::ruff_service::SymbolKind::Class => Span::styled(
                        "[class] ",
                        Style::default()
                            .fg(Color::Yellow)
                            .add_modifier(Modifier::BOLD),
                    ),
                    crate::ruff_service::SymbolKind::Variable => {
                        Span::styled("[var] ", Style::default().fg(Color::LightCyan))
                    }
                };

                let name_style = if is_selected {
                    Style::default()
                        .fg(Color::Black)
                        .bg(Color::Cyan)
                        .add_modifier(Modifier::BOLD)
                } else {
                    Style::default().fg(Color::White)
                };

                let pointer = if is_selected { "▸ " } else { "  " };
                let pointer_span = Span::styled(
                    pointer,
                    if is_selected {
                        Style::default()
                            .fg(Color::Cyan)
                            .add_modifier(Modifier::BOLD)
                    } else {
                        Style::default().fg(Color::DarkGray)
                    },
                );
                let name_span = Span::styled(&item.name, name_style);

                comp_lines.push(Line::from(vec![pointer_span, badge, name_span]));
            }

            let title = format!(
                " Suggestions ({}/{}) ",
                self.completion.selected_index + 1,
                self.completion.suggestions.len()
            );
            let comp_block = Block::default()
                .title(title)
                .borders(Borders::ALL)
                .border_type(BorderType::Rounded)
                .border_style(Style::default().fg(Color::Cyan));

            frame.render_widget(
                Paragraph::new(comp_lines).block(comp_block),
                completion_rect,
            );
        }

        // 4. Render Transient Zoom HUD if active
        if self.font_size_hud_ticks > 0 {
            self.font_size_hud_ticks = self.font_size_hud_ticks.saturating_sub(1);

            let popup_area = centered_rect(48, 20, area);
            frame.render_widget(Clear, popup_area);

            let zoom_percent = ((self.font_size as f32 / 14.0) * 100.0).round() as u16;
            let hud_lines = vec![
                Line::from(vec![
                    Span::styled(
                        "🔍 Code Font Size: ",
                        Style::default()
                            .fg(Color::Cyan)
                            .add_modifier(Modifier::BOLD),
                    ),
                    Span::styled(
                        format!("{} pt", self.font_size),
                        Style::default()
                            .fg(Color::Yellow)
                            .add_modifier(Modifier::BOLD),
                    ),
                    Span::styled(
                        format!(" ({}%)", zoom_percent),
                        Style::default().fg(Color::Green),
                    ),
                ]),
                Line::from(Span::styled(
                    "Font: JetBrains Mono NL",
                    Style::default().fg(Color::White),
                )),
                Line::from(""),
                Line::from(vec![
                    Span::styled(
                        "[Ctrl +] ",
                        Style::default()
                            .fg(Color::Cyan)
                            .add_modifier(Modifier::BOLD),
                    ),
                    Span::styled("Increase (+1 pt)  ", Style::default().fg(Color::White)),
                    Span::styled(
                        "[Ctrl -] ",
                        Style::default()
                            .fg(Color::Cyan)
                            .add_modifier(Modifier::BOLD),
                    ),
                    Span::styled("Decrease (-1 pt)", Style::default().fg(Color::White)),
                ]),
            ];

            let hud_block = Block::default()
                .title(" Font Zoom ")
                .title_alignment(Alignment::Center)
                .borders(Borders::ALL)
                .border_type(BorderType::Double)
                .border_style(Style::default().fg(Color::Yellow));

            let hud_paragraph = Paragraph::new(hud_lines)
                .alignment(Alignment::Center)
                .block(hud_block);

            frame.render_widget(hud_paragraph, popup_area);
        }
    }
}

/// Computes multiline docstring states across lines up to `max_line`
fn compute_multiline_states(lines: &[String], max_line: usize) -> Vec<MultilineStringState> {
    let limit = max_line.min(lines.len());
    let mut states = Vec::with_capacity(limit);
    let mut current = MultilineStringState::None;

    for line in lines.iter().take(limit) {
        states.push(current);

        // Update state based on tokens in this line
        let mut idx = 0;
        let chars: Vec<char> = line.chars().collect();
        let len = chars.len();

        while idx < len {
            match current {
                MultilineStringState::None => {
                    // Check if comment starts
                    if chars[idx] == '#' {
                        break;
                    }
                    // Check for """
                    if idx + 2 < len
                        && chars[idx] == '"'
                        && chars[idx + 1] == '"'
                        && chars[idx + 2] == '"'
                    {
                        current = MultilineStringState::DoubleTriple;
                        idx += 3;
                        continue;
                    }
                    // Check for '''
                    if idx + 2 < len
                        && chars[idx] == '\''
                        && chars[idx + 1] == '\''
                        && chars[idx + 2] == '\''
                    {
                        current = MultilineStringState::SingleTriple;
                        idx += 3;
                        continue;
                    }
                    // Skip regular single-line string
                    if chars[idx] == '"' || chars[idx] == '\'' {
                        let quote = chars[idx];
                        idx += 1;
                        while idx < len {
                            if chars[idx] == '\\' {
                                idx += 2;
                            } else if chars[idx] == quote {
                                idx += 1;
                                break;
                            } else {
                                idx += 1;
                            }
                        }
                        continue;
                    }
                    idx += 1;
                }
                MultilineStringState::DoubleTriple => {
                    if idx + 2 < len
                        && chars[idx] == '"'
                        && chars[idx + 1] == '"'
                        && chars[idx + 2] == '"'
                    {
                        current = MultilineStringState::None;
                        idx += 3;
                    } else {
                        idx += 1;
                    }
                }
                MultilineStringState::SingleTriple => {
                    if idx + 2 < len
                        && chars[idx] == '\''
                        && chars[idx + 1] == '\''
                        && chars[idx + 2] == '\''
                    {
                        current = MultilineStringState::None;
                        idx += 3;
                    } else {
                        idx += 1;
                    }
                }
            }
        }
    }

    states
}

/// Ultra-fast single-pass Python syntax highlighter for a single line with font-size scale styling
fn highlight_python_line(
    line: &str,
    start_state: MultilineStringState,
    font_size: u16,
) -> Vec<Span<'static>> {
    let mut spans = Vec::new();
    let chars: Vec<char> = line.chars().collect();
    let len = chars.len();
    let mut i = 0;

    // If starting inside a multiline string
    if start_state != MultilineStringState::None {
        let mut end_pos = None;
        for pos in 0..len.saturating_sub(2) {
            let target = match start_state {
                MultilineStringState::DoubleTriple => {
                    chars[pos] == '"' && chars[pos + 1] == '"' && chars[pos + 2] == '"'
                }
                MultilineStringState::SingleTriple => {
                    chars[pos] == '\'' && chars[pos + 1] == '\'' && chars[pos + 2] == '\''
                }
                _ => false,
            };
            if target {
                end_pos = Some(pos + 3);
                break;
            }
        }

        if let Some(close_idx) = end_pos {
            let doc_text: String = chars[..close_idx].iter().collect();
            spans.push(Span::styled(doc_text, Style::default().fg(Color::Green)));
            i = close_idx;
        } else {
            // Entire line is inside docstring
            spans.push(Span::styled(
                line.to_string(),
                Style::default().fg(Color::Green),
            ));
            return spans;
        }
    }

    let mut last_word_was_def = false;
    let mut last_word_was_class = false;

    while i < len {
        // 1. Check for Whitespace
        if chars[i].is_whitespace() {
            let start = i;
            while i < len && chars[i].is_whitespace() {
                i += 1;
            }
            let ws: String = chars[start..i].iter().collect();
            spans.push(Span::raw(ws));
            continue;
        }

        // 2. Check for Single-line Comment
        if chars[i] == '#' {
            let comment: String = chars[i..].iter().collect();
            spans.push(Span::styled(
                comment,
                Style::default()
                    .fg(Color::DarkGray)
                    .add_modifier(Modifier::ITALIC),
            ));
            break;
        }

        // 3. Check for Triple-quoted strings beginning
        if i + 2 < len {
            if chars[i] == '"' && chars[i + 1] == '"' && chars[i + 2] == '"' {
                // Find closing """
                let mut close_idx = None;
                for j in (i + 3)..len.saturating_sub(2) {
                    if chars[j] == '"' && chars[j + 1] == '"' && chars[j + 2] == '"' {
                        close_idx = Some(j + 3);
                        break;
                    }
                }
                let end = close_idx.unwrap_or(len);
                let doc: String = chars[i..end].iter().collect();
                spans.push(Span::styled(doc, Style::default().fg(Color::Green)));
                i = end;
                continue;
            } else if chars[i] == '\'' && chars[i + 1] == '\'' && chars[i + 2] == '\'' {
                let mut close_idx = None;
                for j in (i + 3)..len.saturating_sub(2) {
                    if chars[j] == '\'' && chars[j + 1] == '\'' && chars[j + 2] == '\'' {
                        close_idx = Some(j + 3);
                        break;
                    }
                }
                let end = close_idx.unwrap_or(len);
                let doc: String = chars[i..end].iter().collect();
                spans.push(Span::styled(doc, Style::default().fg(Color::Green)));
                i = end;
                continue;
            }
        }

        // 4. Check for String literals (including f"", r"", b"" prefixes)
        let is_prefix = (chars[i] == 'f' || chars[i] == 'r' || chars[i] == 'b' || chars[i] == 'u')
            && i + 1 < len
            && (chars[i + 1] == '"' || chars[i + 1] == '\'');

        if chars[i] == '"' || chars[i] == '\'' || is_prefix {
            let start = i;
            let quote = if is_prefix { chars[i + 1] } else { chars[i] };
            i = if is_prefix { i + 2 } else { i + 1 };

            while i < len {
                if chars[i] == '\\' {
                    i += 2; // skip escaped character
                } else if chars[i] == quote {
                    i += 1;
                    break;
                } else {
                    i += 1;
                }
            }

            let str_val: String = chars[start..i.min(len)].iter().collect();
            spans.push(Span::styled(str_val, Style::default().fg(Color::Green)));
            continue;
        }

        // 5. Check for Decorators (@decorator)
        if chars[i] == '@' {
            let start = i;
            i += 1;
            while i < len && (chars[i].is_alphanumeric() || chars[i] == '_' || chars[i] == '.') {
                i += 1;
            }
            let dec_val: String = chars[start..i].iter().collect();
            spans.push(Span::styled(
                dec_val,
                Style::default()
                    .fg(Color::Yellow)
                    .add_modifier(Modifier::BOLD),
            ));
            continue;
        }

        // 6. Check for Numbers (Hex, Binary, Decimals, Floats)
        if chars[i].is_ascii_digit()
            || (chars[i] == '.' && i + 1 < len && chars[i + 1].is_ascii_digit())
        {
            let start = i;
            let is_hex =
                chars[i] == '0' && i + 1 < len && (chars[i + 1] == 'x' || chars[i + 1] == 'X');
            let is_bin =
                chars[i] == '0' && i + 1 < len && (chars[i + 1] == 'b' || chars[i + 1] == 'B');

            if is_hex || is_bin {
                i += 2;
                while i < len && (chars[i].is_ascii_hexdigit() || chars[i] == '_') {
                    i += 1;
                }
            } else {
                while i < len
                    && (chars[i].is_ascii_digit()
                        || chars[i] == '.'
                        || chars[i] == '_'
                        || chars[i] == 'e'
                        || chars[i] == 'E')
                {
                    i += 1;
                }
            }

            let num_val: String = chars[start..i].iter().collect();
            spans.push(Span::styled(num_val, Style::default().fg(Color::LightCyan)));
            continue;
        }

        // 7. Check for Identifiers, Keywords, Builtins
        if chars[i].is_alphabetic() || chars[i] == '_' {
            let start = i;
            while i < len && (chars[i].is_alphanumeric() || chars[i] == '_') {
                i += 1;
            }
            let word: String = chars[start..i].iter().collect();

            if last_word_was_def {
                // Function declaration name
                spans.push(Span::styled(
                    word,
                    Style::default()
                        .fg(Color::LightBlue)
                        .add_modifier(Modifier::BOLD),
                ));
                last_word_was_def = false;
            } else if last_word_was_class {
                // Class declaration name
                spans.push(Span::styled(
                    word,
                    Style::default()
                        .fg(Color::Yellow)
                        .add_modifier(Modifier::BOLD),
                ));
                last_word_was_class = false;
            } else if is_python_keyword(&word) {
                if word == "def" {
                    last_word_was_def = true;
                } else if word == "class" {
                    last_word_was_class = true;
                }
                spans.push(Span::styled(
                    word,
                    Style::default()
                        .fg(Color::Magenta)
                        .add_modifier(Modifier::BOLD),
                ));
            } else if is_special_constant(&word) {
                spans.push(Span::styled(
                    word,
                    Style::default()
                        .fg(Color::LightYellow)
                        .add_modifier(Modifier::ITALIC),
                ));
            } else if is_python_builtin(&word) {
                spans.push(Span::styled(word, Style::default().fg(Color::Cyan)));
            } else {
                // Regular identifier (rendered bolder if font size >= 16)
                let id_style = if font_size >= 16 {
                    Style::default()
                        .fg(Color::White)
                        .add_modifier(Modifier::BOLD)
                } else {
                    Style::default().fg(Color::White)
                };
                spans.push(Span::styled(word, id_style));
            }
            continue;
        }

        // 8. Operators and Punctuation
        let op = chars[i];
        i += 1;

        let op_style = match op {
            '=' | '+' | '-' | '*' | '/' | '%' | '<' | '>' | '!' | '&' | '|' | '^' | '~' => {
                Style::default().fg(Color::LightBlue)
            }
            ':' | ';' | ',' | '.' => Style::default().fg(Color::DarkGray),
            '(' | ')' | '[' | ']' | '{' | '}' => Style::default().fg(Color::White),
            _ => Style::default().fg(Color::White),
        };

        spans.push(Span::styled(op.to_string(), op_style));
    }

    spans
}

fn is_python_keyword(w: &str) -> bool {
    matches!(
        w,
        "def"
            | "class"
            | "return"
            | "if"
            | "elif"
            | "else"
            | "for"
            | "while"
            | "try"
            | "except"
            | "finally"
            | "with"
            | "async"
            | "await"
            | "lambda"
            | "yield"
            | "pass"
            | "break"
            | "continue"
            | "raise"
            | "import"
            | "from"
            | "as"
            | "in"
            | "is"
            | "not"
            | "and"
            | "or"
            | "global"
            | "nonlocal"
            | "assert"
            | "match"
            | "case"
            | "type"
    )
}

fn is_special_constant(w: &str) -> bool {
    matches!(w, "True" | "False" | "None" | "self" | "cls")
}

fn is_python_builtin(w: &str) -> bool {
    matches!(
        w,
        "print"
            | "len"
            | "range"
            | "enumerate"
            | "zip"
            | "isinstance"
            | "issubclass"
            | "getattr"
            | "setattr"
            | "hasattr"
            | "type"
            | "id"
            | "repr"
            | "str"
            | "int"
            | "float"
            | "bool"
            | "list"
            | "dict"
            | "set"
            | "tuple"
            | "bytes"
            | "open"
            | "map"
            | "filter"
            | "sum"
            | "min"
            | "max"
            | "abs"
            | "round"
            | "any"
            | "all"
            | "super"
            | "object"
            | "Exception"
            | "RuntimeError"
            | "ValueError"
            | "TypeError"
            | "KeyError"
            | "IndexError"
            | "Optional"
            | "Union"
            | "List"
            | "Dict"
            | "Tuple"
            | "Set"
            | "Any"
            | "Callable"
            | "AsyncGenerator"
    )
}

/// Helper function to convert a character column index into a byte index within a string
fn char_to_byte_index(s: &str, char_index: usize) -> usize {
    s.char_indices()
        .nth(char_index)
        .map(|(idx, _)| idx)
        .unwrap_or(s.len())
}

/// Applies background selection highlighting to spans for a given line
fn apply_selection_style(
    spans: Vec<Span<'static>>,
    line_idx: usize,
    selection: Option<&SelectionRange>,
) -> Vec<Span<'static>> {
    let Some(sel_raw) = selection else {
        return spans;
    };
    if sel_raw.is_empty() {
        return spans;
    }

    let sel = SelectionRange::normalized(sel_raw.start, sel_raw.end);

    if line_idx < sel.start.row || line_idx > sel.end.row {
        return spans;
    }

    let line_sel_start = if line_idx == sel.start.row {
        sel.start.col
    } else {
        0
    };
    let line_sel_end = if line_idx == sel.end.row {
        sel.end.col
    } else {
        usize::MAX
    };

    if line_sel_start >= line_sel_end {
        return spans;
    }

    let selection_bg = Color::Rgb(40, 80, 160);

    if spans.is_empty() {
        if line_idx < sel.end.row || line_sel_end > 0 {
            return vec![Span::styled(" ", Style::default().bg(selection_bg))];
        }
        return spans;
    }

    let mut result = Vec::with_capacity(spans.len() + 2);
    let mut current_col = 0;

    for span in spans {
        let text = span.content.as_ref();
        let span_len = text.chars().count();
        let span_end = current_col + span_len;

        if span_end <= line_sel_start || current_col >= line_sel_end {
            // Span is entirely outside selection
            result.push(span);
        } else if current_col >= line_sel_start && span_end <= line_sel_end {
            // Span is entirely inside selection
            let style = span.style.bg(selection_bg);
            result.push(Span::styled(span.content, style));
        } else {
            // Span is partially inside selection
            let mut before = String::new();
            let mut selected = String::new();
            let mut after = String::new();

            for (i, ch) in text.chars().enumerate() {
                let col = current_col + i;
                if col < line_sel_start {
                    before.push(ch);
                } else if col < line_sel_end {
                    selected.push(ch);
                } else {
                    after.push(ch);
                }
            }

            if !before.is_empty() {
                result.push(Span::styled(before, span.style));
            }
            if !selected.is_empty() {
                result.push(Span::styled(selected, span.style.bg(selection_bg)));
            }
            if !after.is_empty() {
                result.push(Span::styled(after, span.style));
            }
        }

        current_col += span_len;
    }

    if line_idx < sel.end.row {
        result.push(Span::styled(" ", Style::default().bg(selection_bg)));
    }

    result
}

/// Applies horizontal scroll by slicing spans according to `scroll_col`
fn apply_horizontal_scroll(spans: &[Span<'static>], scroll_col: usize) -> Vec<Span<'static>> {
    if scroll_col == 0 {
        return spans.to_vec();
    }

    let mut result = Vec::new();
    let mut current_col = 0;

    for span in spans {
        let text = span.content.as_ref();
        let span_len = text.chars().count();

        if current_col + span_len <= scroll_col {
            // Span is entirely hidden to the left
            current_col += span_len;
            continue;
        }

        if current_col < scroll_col {
            // Span is partially hidden
            let skip_chars = scroll_col - current_col;
            let visible_part: String = text.chars().skip(skip_chars).collect();
            result.push(Span::styled(visible_part, span.style));
            current_col += span_len;
        } else {
            // Span is fully visible
            result.push(span.clone());
            current_col += span_len;
        }
    }

    result
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

/// Comprehensive, realistic Python demonstration program showcased upon opening the code editor
pub fn get_default_python_code() -> &'static str {
    r#""""
Tomy On-Device LLM Inference & Code Generation Harness
Demonstrating real-time streaming, KV-cache quantization, and local scheduling.
"""

from dataclasses import dataclass, field
from enum import Enum
import asyncio
import math
import time
from typing import AsyncGenerator, Dict, List, Optional, Tuple, Union


class ModelArchitecture(Enum):
    SMOLLM_2 = "smollm2-1.7b"
    QWEN_CODER = "qwen2.5-coder-1.5b"
    LLAMA_3 = "llama-3.2-3b"
    PHI_35 = "phi-3.5-mini"


@dataclass
class SamplingParams:
    temperature: float = 0.7
    top_p: float = 0.95
    top_k: int = 50
    max_tokens: int = 2048
    repetition_penalty: float = 1.15
    stop_sequences: List[str] = field(default_factory=lambda: ["<|im_end|>", "<|endoftext|>"])


@dataclass
class GenerationStats:
    prompt_tokens: int
    completion_tokens: int
    time_to_first_token_ms: float
    tokens_per_second: float

    @property
    def total_tokens(self) -> int:
        return self.prompt_tokens + self.completion_tokens


def timing_benchmark(func):
    """Decorator to benchmark async model execution latency."""
    async def wrapper(*args, **kwargs):
        start_time = time.perf_counter()
        result = await func(*args, **kwargs)
        duration = (time.perf_counter() - start_time) * 1000.0
        print(f"[{func.__name__}] Execution completed in {duration:.2f} ms")
        return result
    return wrapper


class LocalInferenceEngine:
    """Zero-overhead local tensor inference harness for GGUF weights."""

    def __init__(
        self,
        model_path: str,
        architecture: ModelArchitecture = ModelArchitecture.SMOLLM_2,
        context_size: int = 4096,
        threads: int = 8,
    ) -> None:
        self.model_path = model_path
        self.architecture = architecture
        self.context_size = context_size
        self.threads = threads
        self.kv_cache: Dict[int, List[float]] = {}
        self._is_loaded: bool = False

    async def initialize(self) -> bool:
        """Memory-maps GGUF tensor weights into contiguous CPU buffer."""
        print(f"Loading {self.architecture.value} weights from: {self.model_path}")
        await asyncio.sleep(0.05)  # Simulate fast mmap loading
        self._is_loaded = True
        return True

    @timing_benchmark
    async def stream_tokens(
        self,
        prompt: str,
        params: Optional[SamplingParams] = None,
    ) -> AsyncGenerator[str, None]:
        """Streams generated tokens asynchronously with simulated micro-delays."""
        if not self._is_loaded:
            raise RuntimeError("Model must be initialized before calling generate()!")

        params = params or SamplingParams()
        simulated_tokens = [
            "def ", "solve_n_queens", "(n: int) -> List[List[str]]:\n",
            "    # Backtracking solution with bitmask optimization\n",
            "    solutions = []\n",
            "    cols, diag1, diag2 = set(), set(), set()\n",
            "    board = [['.'] * n for _ in range(n)]\n",
            "    return solutions\n",
        ]

        for token in simulated_tokens:
            await asyncio.sleep(0.02)
            yield token


async def main() -> None:
    # Initialize engine with SmolLM2-1.7B
    engine = LocalInferenceEngine(
        model_path="models/smollm2-1.7b-instruct-q4_k_m.gguf",
        architecture=ModelArchitecture.SMOLLM_2,
        threads=8,
    )
    await engine.initialize()

    prompt = "Write an optimized N-Queens solver in Python with type hints."
    print(f"Generating code response for prompt: '{prompt}'\n")

    async for token in engine.stream_tokens(prompt):
        print(token, end="", flush=True)


if __name__ == "__main__":
    asyncio.run(main())
"#
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_editor_initialization() {
        let editor = EditorScreen::new();
        assert!(!editor.lines.is_empty());
        assert_eq!(editor.cursor_row, 0);
        assert_eq!(editor.cursor_col, 0);
        assert!(!editor.is_modified);
    }

    #[test]
    fn test_tab_inserts_four_spaces() {
        let mut editor = EditorScreen::new();
        editor.lines = vec![String::new()];
        editor.cursor_row = 0;
        editor.cursor_col = 0;

        editor.insert_tab();
        assert_eq!(editor.lines[0], "    ");
        assert_eq!(editor.cursor_col, 4);
        assert!(editor.is_modified);
    }

    #[test]
    fn test_unindent_removes_four_spaces() {
        let mut editor = EditorScreen::new();
        editor.lines = vec!["        def test():".to_string()];
        editor.cursor_row = 0;
        editor.cursor_col = 8;

        editor.unindent();
        assert_eq!(editor.lines[0], "    def test():");
        assert_eq!(editor.cursor_col, 4);
    }

    #[test]
    fn test_smart_backspace_removes_four_spaces() {
        let mut editor = EditorScreen::new();
        editor.lines = vec!["    print('hello')".to_string()];
        editor.cursor_row = 0;
        editor.cursor_col = 4;

        editor.handle_backspace();
        assert_eq!(editor.lines[0], "print('hello')");
        assert_eq!(editor.cursor_col, 0);
    }

    #[test]
    fn test_single_char_backspace() {
        let mut editor = EditorScreen::new();
        editor.lines = vec!["x = 5".to_string()];
        editor.cursor_row = 0;
        editor.cursor_col = 5;

        editor.handle_backspace();
        assert_eq!(editor.lines[0], "x = ");
        assert_eq!(editor.cursor_col, 4);
    }

    #[test]
    fn test_backspace_at_start_merges_lines() {
        let mut editor = EditorScreen::new();
        editor.lines = vec!["line 1".to_string(), "line 2".to_string()];
        editor.cursor_row = 1;
        editor.cursor_col = 0;

        editor.handle_backspace();
        assert_eq!(editor.lines.len(), 1);
        assert_eq!(editor.lines[0], "line 1line 2");
        assert_eq!(editor.cursor_row, 0);
        assert_eq!(editor.cursor_col, 6);
    }

    #[test]
    fn test_newline_carries_indent_and_colon() {
        let mut editor = EditorScreen::new();
        editor.lines = vec!["def my_func():".to_string()];
        editor.cursor_row = 0;
        editor.cursor_col = 14;

        editor.insert_newline();
        assert_eq!(editor.lines.len(), 2);
        assert_eq!(editor.lines[0], "def my_func():");
        assert_eq!(editor.lines[1], "    ");
        assert_eq!(editor.cursor_row, 1);
        assert_eq!(editor.cursor_col, 4);
    }

    #[test]
    fn test_syntax_highlighter_tokens() {
        let line = "def calculate_sum(a: int, b: int = 10) -> int:";
        let spans = highlight_python_line(line, MultilineStringState::None, 14);

        // Verify keyword 'def' has Magenta style
        let def_span = spans.iter().find(|s| s.content == "def").unwrap();
        assert_eq!(def_span.style.fg, Some(Color::Magenta));

        // Verify function name has LightBlue style
        let fn_span = spans.iter().find(|s| s.content == "calculate_sum").unwrap();
        assert_eq!(fn_span.style.fg, Some(Color::LightBlue));

        // Verify builtin 'int' has Cyan style
        let int_span = spans.iter().find(|s| s.content == "int").unwrap();
        assert_eq!(int_span.style.fg, Some(Color::Cyan));

        // Verify number '10' has LightCyan style
        let num_span = spans.iter().find(|s| s.content == "10").unwrap();
        assert_eq!(num_span.style.fg, Some(Color::LightCyan));
    }

    #[test]
    fn test_syntax_highlighter_comment() {
        let line = "x = 42  # This is a comment";
        let spans = highlight_python_line(line, MultilineStringState::None, 14);
        let comment_span = spans.iter().find(|s| s.content.starts_with('#')).unwrap();
        assert_eq!(comment_span.style.fg, Some(Color::DarkGray));
    }

    #[test]
    fn test_syntax_highlighter_strings() {
        let line = "msg = f\"Hello {name}\"";
        let spans = highlight_python_line(line, MultilineStringState::None, 14);
        let str_span = spans.iter().find(|s| s.content.starts_with("f\"")).unwrap();
        assert_eq!(str_span.style.fg, Some(Color::Green));
    }

    #[test]
    fn test_increase_and_decrease_font_size() {
        let mut editor = EditorScreen::new();
        assert_eq!(editor.font_size, 14);

        // Increase by +1
        editor.increase_font_size();
        assert_eq!(editor.font_size, 15);
        assert!(editor.font_size_hud_ticks > 0);

        // Decrease by -1
        editor.decrease_font_size();
        assert_eq!(editor.font_size, 14);
    }

    #[test]
    fn test_ctrl_plus_and_minus_key_event() {
        let mut editor = EditorScreen::new();
        assert_eq!(editor.font_size, 14);

        // Simulate Ctrl + '+'
        editor.handle_key(KeyEvent::new(KeyCode::Char('+'), KeyModifiers::CONTROL));
        assert_eq!(editor.font_size, 15);

        // Simulate Ctrl + '='
        editor.handle_key(KeyEvent::new(KeyCode::Char('='), KeyModifiers::CONTROL));
        assert_eq!(editor.font_size, 16);

        // Simulate Ctrl + '-'
        editor.handle_key(KeyEvent::new(KeyCode::Char('-'), KeyModifiers::CONTROL));
        assert_eq!(editor.font_size, 15);

        // Simulate Ctrl + '_'
        editor.handle_key(KeyEvent::new(KeyCode::Char('_'), KeyModifiers::CONTROL));
        assert_eq!(editor.font_size, 14);
    }

    #[test]
    fn test_open_and_save_file() {
        let temp_file = std::env::temp_dir().join("tomy_editor_io_test.py");
        std::fs::write(&temp_file, "val = 100\nprint(val)").unwrap();

        let mut editor = EditorScreen::new();
        editor.open_file(&temp_file);
        assert_eq!(editor.lines.len(), 2);
        assert_eq!(editor.lines[0], "val = 100");
        assert_eq!(editor.current_file_path, Some(temp_file.clone()));

        // Edit and save
        editor.lines.push("print('done')".to_string());
        editor.is_modified = true;
        assert!(editor.save_file());
        assert!(!editor.is_modified);

        let reloaded = std::fs::read_to_string(&temp_file).unwrap();
        assert!(reloaded.contains("done"));

        let _ = std::fs::remove_file(temp_file);
    }

    #[test]
    fn test_editor_save_auto_formats_with_ruff() {
        let temp_file = std::env::temp_dir().join("tomy_editor_ruff_format_test.py");
        std::fs::write(&temp_file, "def   bad_format( x,y ):\n    return x+y\n").unwrap();

        let mut editor = EditorScreen::new();
        editor.open_file(&temp_file);
        assert_eq!(editor.lines[0], "def   bad_format( x,y ):");

        // Save file -> triggers in-process Ruff formatting
        assert!(editor.save_file());
        assert_eq!(editor.lines[0], "def bad_format(x, y):");
        assert_eq!(editor.lines[1], "    return x + y");

        let reloaded = std::fs::read_to_string(&temp_file).unwrap();
        assert_eq!(reloaded, "def bad_format(x, y):\n    return x + y\n");

        let _ = std::fs::remove_file(temp_file);
    }

    #[test]
    fn test_editor_autocomplete_trigger_and_insert() {
        let mut editor = EditorScreen::new();
        editor.lines = vec![String::new()];
        editor.cursor_row = 0;
        editor.cursor_col = 0;

        // Type 'c'
        editor.handle_key(KeyEvent::new(KeyCode::Char('c'), KeyModifiers::NONE));
        assert!(editor.completion.active);
        assert_eq!(editor.completion.query, "c");

        // Type 'l' -> "cl"
        editor.handle_key(KeyEvent::new(KeyCode::Char('l'), KeyModifiers::NONE));
        assert!(editor.completion.active);
        assert_eq!(editor.completion.query, "cl");
        assert!(
            editor
                .completion
                .suggestions
                .iter()
                .any(|s| s.name == "class")
        );

        // Press Tab to complete "class"
        editor.handle_key(KeyEvent::new(KeyCode::Tab, KeyModifiers::NONE));
        assert!(!editor.completion.active);
        assert_eq!(editor.lines[0], "class");
        assert_eq!(editor.cursor_col, 5);
    }

    #[test]
    fn test_editor_autocomplete_custom_class_and_variable() {
        let mut editor = EditorScreen::new();
        editor.lines = vec![
            "class PipelineEngine:".to_string(),
            "    batch_counter = 100".to_string(),
            "".to_string(),
        ];
        editor.symbol_index = FileSymbolIndex::extract(&editor.lines);
        assert!(editor.symbol_index.classes.contains("PipelineEngine"));
        assert!(editor.symbol_index.variables.contains("batch_counter"));

        // Move to empty line 2
        editor.cursor_row = 2;
        editor.cursor_col = 0;

        // Type 'P', 'i', 'p'
        for ch in ['P', 'i', 'p'] {
            editor.handle_key(KeyEvent::new(KeyCode::Char(ch), KeyModifiers::NONE));
        }
        assert!(editor.completion.active);
        assert_eq!(editor.completion.query, "Pip");
        assert_eq!(editor.completion.suggestions[0].name, "PipelineEngine");

        // Accept with Enter
        editor.handle_key(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE));
        assert_eq!(editor.lines[2], "PipelineEngine");
        assert_eq!(editor.cursor_col, 14);

        // Next, test variable completion: type ' ', 'b', 'a', 't'
        editor.handle_key(KeyEvent::new(KeyCode::Char(' '), KeyModifiers::NONE));
        for ch in ['b', 'a', 't'] {
            editor.handle_key(KeyEvent::new(KeyCode::Char(ch), KeyModifiers::NONE));
        }
        assert!(editor.completion.active);
        assert!(
            editor
                .completion
                .suggestions
                .iter()
                .any(|s| s.name == "batch_counter")
        );

        // Dismiss with Esc
        editor.handle_key(KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE));
        assert!(!editor.completion.active);
    }

    #[test]
    fn test_editor_token_count_tracking() {
        let mut editor = EditorScreen::new();
        assert!(editor.token_count > 0);

        let initial_tokens = editor.token_count;
        editor.lines.push("x = [1, 2, 3, 4, 5]".to_string());
        editor.update_token_count();
        assert!(editor.token_count > initial_tokens);
    }

    #[test]
    fn test_ctrl_a_select_all_and_copy() {
        let mut editor = EditorScreen::new();
        editor.lines = vec!["def foo():".to_string(), "    return 42".to_string()];
        assert!(editor.selection.is_none());

        // Ctrl + A
        editor.handle_key(KeyEvent::new(KeyCode::Char('a'), KeyModifiers::CONTROL));
        let sel = editor
            .selection
            .expect("Selection should be active after Ctrl+A");
        assert_eq!(sel.start, TextPosition { row: 0, col: 0 });
        assert_eq!(sel.end, TextPosition { row: 1, col: 13 });

        // Get selected text
        let text = editor.get_selected_text().unwrap();
        assert_eq!(text, "def foo():\n    return 42");

        // Ctrl + C
        editor.handle_key(KeyEvent::new(KeyCode::Char('c'), KeyModifiers::CONTROL));
        assert_eq!(editor.copy_toast_ticks, 30);
    }

    #[test]
    fn test_mouse_selection_and_deletion() {
        let mut editor = EditorScreen::new();
        editor.lines = vec![
            "line zero".to_string(),
            "line one target words".to_string(),
            "line two".to_string(),
        ];

        // Simulate code_area: 80x24 at (0, 1)
        editor.last_code_area = Some(Rect::new(0, 1, 80, 24));
        editor.scroll_row = 0;
        editor.scroll_col = 0;

        // Line 1 is at terminal row 1 (header is row 0, border at row 1, line 0 at row 2, line 1 at row 3)
        // gutter digits = 3, gutter total width = 6 ("   1 │ ")
        // content starts at x = 0 + 1 + 6 = 7
        let pos1 = editor
            .screen_coords_to_text_pos(7 + 5, 3)
            .expect("Valid text pos");
        assert_eq!(pos1.row, 1);
        assert_eq!(pos1.col, 5);

        let pos2 = editor
            .screen_coords_to_text_pos(7 + 15, 3)
            .expect("Valid text pos");
        assert_eq!(pos2.row, 1);
        assert_eq!(pos2.col, 15);

        editor.selection = Some(SelectionRange {
            start: pos1,
            end: pos2,
        });
        let selected_text = editor.get_selected_text().unwrap();
        assert_eq!(selected_text, "one target");

        // Delete selection
        assert!(editor.delete_selection());
        assert_eq!(editor.lines[1], "line  words");
        assert_eq!(editor.cursor_col, 5);
        assert!(editor.selection.is_none());
    }

    #[test]
    fn test_word_navigation_ctrl_arrows() {
        let mut editor = EditorScreen::new();
        editor.lines = vec!["alpha_beta  gamma  delta".to_string()];
        editor.cursor_row = 0;
        editor.cursor_col = 0;

        // Ctrl + Right: skip alpha_beta
        editor.handle_key(KeyEvent::new(KeyCode::Right, KeyModifiers::CONTROL));
        assert_eq!(editor.cursor_col, 12); // starts at 'gamma'

        // Ctrl + Right: skip gamma
        editor.handle_key(KeyEvent::new(KeyCode::Right, KeyModifiers::CONTROL));
        assert_eq!(editor.cursor_col, 19); // starts at 'delta'

        // Ctrl + Left: back to 'gamma'
        editor.handle_key(KeyEvent::new(KeyCode::Left, KeyModifiers::CONTROL));
        assert_eq!(editor.cursor_col, 12);

        // Ctrl + Left: back to 0
        editor.handle_key(KeyEvent::new(KeyCode::Left, KeyModifiers::CONTROL));
        assert_eq!(editor.cursor_col, 0);
    }

    #[test]
    fn test_word_deletion_ctrl_backspace() {
        let mut editor = EditorScreen::new();
        editor.lines = vec!["result = calculate_sum()".to_string()];
        editor.cursor_row = 0;
        editor.cursor_col = 24; // at end of line

        // Ctrl + Backspace: deletes "()"
        editor.handle_key(KeyEvent::new(KeyCode::Backspace, KeyModifiers::CONTROL));
        assert_eq!(editor.lines[0], "result = calculate_sum");
        assert_eq!(editor.cursor_col, 22);

        // Ctrl + Backspace: deletes "calculate_sum"
        editor.handle_key(KeyEvent::new(KeyCode::Backspace, KeyModifiers::CONTROL));
        assert_eq!(editor.lines[0], "result = ");
        assert_eq!(editor.cursor_col, 9);

        // Also test Ctrl+W keycode variation
        editor.handle_key(KeyEvent::new(KeyCode::Char('w'), KeyModifiers::CONTROL));
        assert_eq!(editor.lines[0], "result ");
        assert_eq!(editor.cursor_col, 7);
    }

    #[test]
    fn test_selection_replacement_on_typing() {
        let mut editor = EditorScreen::new();
        editor.lines = vec!["hello world".to_string()];
        editor.cursor_row = 0;
        editor.cursor_col = 11;
        editor.selection = Some(SelectionRange {
            start: TextPosition { row: 0, col: 6 },
            end: TextPosition { row: 0, col: 11 },
        });

        // Type 't'
        editor.handle_key(KeyEvent::new(KeyCode::Char('t'), KeyModifiers::NONE));
        assert_eq!(editor.lines[0], "hello t");
        assert!(editor.selection.is_none());
    }

    #[test]
    fn test_apply_selection_style_slicing() {
        let spans = vec![Span::raw("hello "), Span::raw("world")];
        let selection = SelectionRange {
            start: TextPosition { row: 0, col: 3 },
            end: TextPosition { row: 0, col: 8 },
        };

        let styled = apply_selection_style(spans, 0, Some(&selection));
        let combined: String = styled.iter().map(|s| s.content.as_ref()).collect();
        assert_eq!(combined, "hello world");

        // Verify that selected slice has selection background
        let selection_bg = Color::Rgb(40, 80, 160);
        let has_selected_bg = styled.iter().any(|s| s.style.bg == Some(selection_bg));
        assert!(has_selected_bg);
    }
}
