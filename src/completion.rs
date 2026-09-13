use crate::ruff_service::{FileSymbol, FileSymbolIndex};

#[derive(Debug, Clone, Default)]
pub struct CompletionState {
    pub active: bool,
    pub query: String,
    pub start_col: usize,
    pub suggestions: Vec<FileSymbol>,
    pub selected_index: usize,
    pub max_visible: usize,
    pub scroll_offset: usize,
}

impl CompletionState {
    pub fn new() -> Self {
        Self {
            active: false,
            query: String::new(),
            start_col: 0,
            suggestions: Vec::new(),
            selected_index: 0,
            max_visible: 6,
            scroll_offset: 0,
        }
    }

    /// Re-evaluates suggestions based on the word prefix right before cursor_col on the current line
    pub fn update(&mut self, line: &str, cursor_col: usize, symbol_index: &FileSymbolIndex) {
        let chars: Vec<char> = line.chars().collect();
        let col = cursor_col.min(chars.len());

        // Scan backwards from cursor_col to find the start of the identifier prefix
        let mut start = col;
        while start > 0 && (chars[start - 1].is_alphanumeric() || chars[start - 1] == '_') {
            start -= 1;
        }

        if start < col {
            let prefix: String = chars[start..col].iter().collect();
            // Trigger threshold: >= 1 character typed
            if !prefix.is_empty() {
                let matches = symbol_index.query_matches(&prefix);
                if !matches.is_empty() {
                    self.active = true;
                    self.query = prefix;
                    self.start_col = start;
                    self.suggestions = matches;
                    if self.selected_index >= self.suggestions.len() {
                        self.selected_index = 0;
                    }
                    self.adjust_scroll();
                    return;
                }
            }
        }

        self.dismiss();
    }

    pub fn select_next(&mut self) {
        if !self.suggestions.is_empty() {
            self.selected_index = (self.selected_index + 1) % self.suggestions.len();
            self.adjust_scroll();
        }
    }

    pub fn select_prev(&mut self) {
        if !self.suggestions.is_empty() {
            if self.selected_index == 0 {
                self.selected_index = self.suggestions.len() - 1;
            } else {
                self.selected_index -= 1;
            }
            self.adjust_scroll();
        }
    }

    fn adjust_scroll(&mut self) {
        if self.selected_index < self.scroll_offset {
            self.scroll_offset = self.selected_index;
        } else if self.selected_index >= self.scroll_offset + self.max_visible {
            self.scroll_offset = self.selected_index + 1 - self.max_visible;
        }
    }

    pub fn dismiss(&mut self) {
        self.active = false;
        self.query.clear();
        self.start_col = 0;
        self.suggestions.clear();
        self.selected_index = 0;
        self.scroll_offset = 0;
    }

    /// Applies the selected suggestion into the line, replacing the typed prefix and updating cursor_col
    pub fn apply_selected(&mut self, line: &mut String, cursor_col: &mut usize) -> bool {
        if !self.active || self.suggestions.is_empty() {
            return false;
        }

        let selected = &self.suggestions[self.selected_index];
        let chars: Vec<char> = line.chars().collect();
        let end_col = (*cursor_col).min(chars.len());
        let start_col = self.start_col.min(end_col);

        let prefix_part: String = chars[..start_col].iter().collect();
        let suffix_part: String = chars[end_col..].iter().collect();

        *line = format!("{}{}{}", prefix_part, selected.name, suffix_part);
        *cursor_col = start_col + selected.name.chars().count();

        self.dismiss();
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_completion_update_and_apply() {
        let mut index = FileSymbolIndex::new();
        index.classes.insert("SamplingParams".to_string());
        index.variables.insert("sample_rate".to_string());

        let mut comp = CompletionState::new();
        let mut line = "def setup(samp".to_string();
        let mut cursor_col = 14;

        comp.update(&line, cursor_col, &index);
        assert!(comp.active);
        assert_eq!(comp.query, "samp");
        assert_eq!(comp.start_col, 10);
        assert_eq!(comp.suggestions.len(), 2);

        // Accept first suggestion (SamplingParams)
        assert!(comp.apply_selected(&mut line, &mut cursor_col));
        assert_eq!(line, "def setup(SamplingParams");
        assert_eq!(cursor_col, 24);
        assert!(!comp.active);
    }

    #[test]
    fn test_completion_cycle_and_select() {
        let mut index = FileSymbolIndex::new();
        index.classes.insert("Apple".to_string());
        index.variables.insert("application".to_string());

        let mut comp = CompletionState::new();
        let line = "app".to_string();
        let cursor_col = 3;

        comp.update(&line, cursor_col, &index);
        assert!(comp.active);
        assert_eq!(comp.selected_index, 0);

        comp.select_next();
        assert_eq!(comp.selected_index, 1);

        comp.select_prev();
        assert_eq!(comp.selected_index, 0);

        comp.dismiss();
        assert!(!comp.active);
    }
}
