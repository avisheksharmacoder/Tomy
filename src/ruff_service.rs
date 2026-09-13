use std::collections::BTreeSet;
use std::path::Path;

use ruff_linter::linter::lint_fix;
use ruff_linter::settings::types::UnsafeFixes;
use ruff_linter::settings::{LinterSettings, flags};
use ruff_linter::source_kind::SourceKind;
use ruff_python_ast::{PySourceType, SourceType};
use ruff_python_formatter::{FormatModuleError, PyFormatOptions, format_module_source};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RuffLintDiagnostic {
    pub message: String,
    pub rule_code: String,
    pub line: usize,
    pub column: usize,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub enum SymbolKind {
    Keyword,
    Class,
    Variable,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct FileSymbol {
    pub name: String,
    pub kind: SymbolKind,
}

pub const PYTHON_KEYWORDS: &[&str] = &[
    "False", "None", "True", "and", "as", "assert", "async", "await", "break", "case", "class",
    "cls", "continue", "def", "del", "elif", "else", "except", "finally", "for", "from", "global",
    "if", "import", "in", "is", "lambda", "match", "nonlocal", "not", "or", "pass", "raise",
    "return", "self", "try", "type", "while", "with", "yield",
];

/// Result of executing Ruff format and strict lint-fix on Python source code
#[derive(Debug, Clone)]
pub struct RuffProcessResult {
    pub formatted_code: String,
    pub diagnostics: Vec<RuffLintDiagnostic>,
    pub fixes_applied: usize,
    pub parse_error: Option<String>,
}

/// Formats and strictly lints/fixes Python source code using in-process Ruff
pub fn process_python_code(code: &str, file_path: Option<&Path>) -> RuffProcessResult {
    let dummy_path = std::path::PathBuf::from(file_path.unwrap_or_else(|| Path::new("script.py")));
    let mut current_code = code.to_string();
    let mut diagnostics = Vec::new();
    let mut fixes_applied = 0;

    // 1. Strict Lint & Auto-Fix pass
    let source_type = SourceType::Python(PySourceType::Python);
    if let Ok(Some(source_kind)) = SourceKind::from_source_code(current_code.clone(), source_type) {
        let settings = LinterSettings::default();
        if let Ok(fixer_result) = lint_fix(
            &dummy_path,
            None,
            flags::Noqa::Enabled,
            UnsafeFixes::Enabled,
            &settings,
            &source_kind,
            PySourceType::Python,
        ) {
            fixes_applied = fixer_result.fixed.counts().sum();
            current_code = fixer_result.transformed.source_code().to_string();

            // Collect remaining diagnostics
            for diag in fixer_result.result.diagnostics {
                let (line, col) = if let Some(loc) = diag.ruff_start_location() {
                    (loc.line.get(), loc.column.get())
                } else if let Some(range) = diag.range() {
                    (range.start().to_usize(), range.end().to_usize())
                } else {
                    (0, 0)
                };

                diagnostics.push(RuffLintDiagnostic {
                    message: diag.headline_message().to_string(),
                    rule_code: diag.secondary_code_or_id().to_string(),
                    line,
                    column: col,
                });
            }
        }
    }

    // 2. Format pass with ruff_python_formatter (PEP 8 / Black style)
    let format_options = PyFormatOptions::from_source_type(PySourceType::Python);
    match format_module_source(&current_code, format_options) {
        Ok(printed) => {
            let final_code = printed.into_code();
            RuffProcessResult {
                formatted_code: final_code,
                diagnostics,
                fixes_applied,
                parse_error: None,
            }
        }
        Err(FormatModuleError::ParseError(err)) => {
            // Syntax error prevented formatting: preserve current code and report error
            RuffProcessResult {
                formatted_code: current_code,
                diagnostics,
                fixes_applied,
                parse_error: Some(format!("{}", err)),
            }
        }
        Err(err) => RuffProcessResult {
            formatted_code: current_code,
            diagnostics,
            fixes_applied,
            parse_error: Some(format!("{}", err)),
        },
    }
}

/// Symbol index containing classes and variable names extracted strictly from the active file
#[derive(Debug, Default, Clone)]
pub struct FileSymbolIndex {
    pub classes: BTreeSet<String>,
    pub variables: BTreeSet<String>,
}

impl FileSymbolIndex {
    pub fn new() -> Self {
        Self::default()
    }

    /// Rebuilds symbol index from buffer lines using AST and regex/token fallback
    pub fn extract(lines: &[String]) -> Self {
        let mut index = FileSymbolIndex::new();
        let full_text = lines.join("\n");

        // 1. Extract with Ruff AST Parser (try full parse, fallback to unchecked parse)
        if let Ok(parsed) = ruff_python_parser::parse_module(&full_text) {
            extract_symbols_from_ast(parsed.suite(), &mut index);
        } else {
            let unchecked =
                ruff_python_parser::parse_unchecked_source(&full_text, PySourceType::Python);
            extract_symbols_from_ast(unchecked.suite(), &mut index);
        }

        // 2. Resilient lexical regex pass across all lines (catches symbols even when code is partially typed or invalid)
        extract_symbols_lexically(lines, &mut index);

        index
    }

    /// Queries symbols matching a given prefix (case-sensitive or smart prefix)
    pub fn query_matches<'a>(&'a self, prefix: &'a str) -> Vec<FileSymbol> {
        let mut results = Vec::new();
        if prefix.is_empty() {
            return results;
        }

        let prefix_lower = prefix.to_lowercase();

        // 1. Python Keywords
        for &kw in PYTHON_KEYWORDS {
            if kw.starts_with(prefix) || kw.to_lowercase().starts_with(&prefix_lower) {
                results.push(FileSymbol {
                    name: kw.to_string(),
                    kind: SymbolKind::Keyword,
                });
            }
        }

        // 2. Class Names from current file
        for class_name in &self.classes {
            if class_name.starts_with(prefix)
                || class_name.to_lowercase().starts_with(&prefix_lower)
            {
                results.push(FileSymbol {
                    name: class_name.clone(),
                    kind: SymbolKind::Class,
                });
            }
        }

        // 3. Variable Names from current file
        for var_name in &self.variables {
            if var_name.starts_with(prefix) || var_name.to_lowercase().starts_with(&prefix_lower) {
                results.push(FileSymbol {
                    name: var_name.clone(),
                    kind: SymbolKind::Variable,
                });
            }
        }

        // De-duplicate if a name appears multiple times
        results.sort();
        results.dedup_by(|a, b| a.name == b.name && a.kind == b.kind);

        // Filter out if the only match is already identical to what's typed
        if results.len() == 1 && results[0].name == prefix {
            return Vec::new();
        }

        results
    }
}

fn extract_symbols_from_ast(suite: &[ruff_python_ast::Stmt], index: &mut FileSymbolIndex) {
    use ruff_python_ast::Stmt;

    for stmt in suite {
        match stmt {
            Stmt::ClassDef(class_def) => {
                let name = class_def.name.as_str().to_string();
                if is_valid_identifier(&name) {
                    index.classes.insert(name);
                }
                extract_symbols_from_ast(&class_def.body, index);
            }
            Stmt::FunctionDef(fn_def) => {
                // Function parameters count as variables
                for param in &fn_def.parameters.posonlyargs {
                    let p = param.parameter.name.as_str().to_string();
                    if is_valid_identifier(&p) && !is_keyword(&p) {
                        index.variables.insert(p);
                    }
                }
                for param in &fn_def.parameters.args {
                    let p = param.parameter.name.as_str().to_string();
                    if is_valid_identifier(&p) && !is_keyword(&p) {
                        index.variables.insert(p);
                    }
                }
                for param in &fn_def.parameters.kwonlyargs {
                    let p = param.parameter.name.as_str().to_string();
                    if is_valid_identifier(&p) && !is_keyword(&p) {
                        index.variables.insert(p);
                    }
                }
                if let Some(ref vararg) = fn_def.parameters.vararg {
                    let p = vararg.name.as_str().to_string();
                    if is_valid_identifier(&p) && !is_keyword(&p) {
                        index.variables.insert(p);
                    }
                }
                if let Some(ref kwarg) = fn_def.parameters.kwarg {
                    let p = kwarg.name.as_str().to_string();
                    if is_valid_identifier(&p) && !is_keyword(&p) {
                        index.variables.insert(p);
                    }
                }
                extract_symbols_from_ast(&fn_def.body, index);
            }
            Stmt::Assign(assign) => {
                for target in &assign.targets {
                    extract_expr_targets(target, index);
                }
            }
            Stmt::AnnAssign(ann_assign) => {
                extract_expr_targets(&ann_assign.target, index);
            }
            Stmt::AugAssign(aug_assign) => {
                extract_expr_targets(&aug_assign.target, index);
            }
            Stmt::For(for_stmt) => {
                extract_expr_targets(&for_stmt.target, index);
                extract_symbols_from_ast(&for_stmt.body, index);
                extract_symbols_from_ast(&for_stmt.orelse, index);
            }
            Stmt::While(while_stmt) => {
                extract_symbols_from_ast(&while_stmt.body, index);
                extract_symbols_from_ast(&while_stmt.orelse, index);
            }
            Stmt::If(if_stmt) => {
                extract_symbols_from_ast(&if_stmt.body, index);
                let elif_bodies: Vec<ruff_python_ast::Stmt> = if_stmt
                    .elif_else_clauses
                    .iter()
                    .flat_map(|c| c.body.iter().cloned())
                    .collect();
                extract_symbols_from_ast(&elif_bodies, index);
            }
            Stmt::With(with_stmt) => {
                for item in &with_stmt.items {
                    if let Some(ref target) = item.optional_vars {
                        extract_expr_targets(target, index);
                    }
                }
                extract_symbols_from_ast(&with_stmt.body, index);
            }
            _ => {}
        }
    }
}

fn extract_expr_targets(expr: &ruff_python_ast::Expr, index: &mut FileSymbolIndex) {
    use ruff_python_ast::Expr;
    match expr {
        Expr::Name(name_expr) => {
            let id = name_expr.id.as_str().to_string();
            if is_valid_identifier(&id) && !is_keyword(&id) {
                index.variables.insert(id);
            }
        }
        Expr::Attribute(attr_expr) => {
            // e.g. self.my_field = ...
            let attr = attr_expr.attr.as_str().to_string();
            if is_valid_identifier(&attr) && !is_keyword(&attr) {
                index.variables.insert(attr);
            }
        }
        Expr::Tuple(tuple_expr) => {
            for el in &tuple_expr.elts {
                extract_expr_targets(el, index);
            }
        }
        Expr::List(list_expr) => {
            for el in &list_expr.elts {
                extract_expr_targets(el, index);
            }
        }
        _ => {}
    }
}

/// Fast lexical pattern scanner that works even on syntactically broken/partial Python code
fn extract_symbols_lexically(lines: &[String], index: &mut FileSymbolIndex) {
    for raw_line in lines {
        let trimmed = raw_line.trim();

        // 1. Check for class declaration: class Foo...
        if let Some(rest) = trimmed.strip_prefix("class ") {
            let class_name = rest
                .chars()
                .take_while(|c| c.is_alphanumeric() || *c == '_')
                .collect::<String>();
            if is_valid_identifier(&class_name) && !is_keyword(&class_name) {
                index.classes.insert(class_name);
            }
        }

        // 2. Check for variable assignment: foo = ... or self.foo = ... or foo: int = ...
        if let Some(eq_idx) = trimmed.find('=') {
            // Ensure not ==, !=, <=, >=
            let is_comparison = (eq_idx > 0
                && matches!(trimmed.as_bytes()[eq_idx - 1], b'=' | b'!' | b'<' | b'>'))
                || (eq_idx + 1 < trimmed.len() && trimmed.as_bytes()[eq_idx + 1] == b'=');

            if !is_comparison {
                let lhs = trimmed[..eq_idx].trim();
                // Strip type annotation if present: x: int
                let target_part = if let Some(colon_idx) = lhs.find(':') {
                    lhs[..colon_idx].trim()
                } else {
                    lhs
                };

                // Split commas in case of tuple unpack: a, b = ...
                for raw_var in target_part.split(',') {
                    let var = raw_var.trim();
                    let final_var = if let Some(dot_idx) = var.rfind('.') {
                        &var[dot_idx + 1..]
                    } else {
                        var
                    };

                    if is_valid_identifier(final_var) && !is_keyword(final_var) {
                        index.variables.insert(final_var.to_string());
                    }
                }
            }
        }

        // 3. Check for for loop target: for item in ...
        if let Some(rest) = trimmed.strip_prefix("for ")
            && let Some(in_idx) = rest.find(" in ")
        {
            let target = rest[..in_idx].trim();
            for raw_var in target.split(',') {
                let v = raw_var.trim();
                if is_valid_identifier(v) && !is_keyword(v) {
                    index.variables.insert(v.to_string());
                }
            }
        }
    }
}

fn is_valid_identifier(s: &str) -> bool {
    if s.is_empty() {
        return false;
    }
    let mut chars = s.chars();
    let first = chars.next().unwrap();
    if !first.is_alphabetic() && first != '_' {
        return false;
    }
    chars.all(|c| c.is_alphanumeric() || c == '_')
}

fn is_keyword(s: &str) -> bool {
    PYTHON_KEYWORDS.contains(&s)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_format_valid_python_code() {
        let code = "def   calculate(  a , b ):\n    return    a+b\n";
        let res = process_python_code(code, None);
        assert!(res.parse_error.is_none());
        assert_eq!(
            res.formatted_code,
            "def calculate(a, b):\n    return a + b\n"
        );
    }

    #[test]
    fn test_format_syntax_error_resilience() {
        let code = "def broken(x:\n    return x +";
        let res = process_python_code(code, None);
        assert!(res.parse_error.is_some());
        // Code is preserved without loss
        assert_eq!(res.formatted_code, code);
    }

    #[test]
    fn test_symbol_extraction_classes_and_variables() {
        let lines = vec![
            "class InferenceEngine:".to_string(),
            "    def __init__(self, model_name: str):".to_string(),
            "        self.model_name = model_name".to_string(),
            "        self.total_tokens = 0".to_string(),
            "".to_string(),
            "    def compute(self, max_tokens: int):".to_string(),
            "        current_batch = 128".to_string(),
            "        for token_id in range(max_tokens):".to_string(),
            "            self.total_tokens += 1".to_string(),
            "        return current_batch".to_string(),
        ];

        let index = FileSymbolIndex::extract(&lines);
        assert!(index.classes.contains("InferenceEngine"));
        assert!(index.variables.contains("model_name"));
        assert!(index.variables.contains("total_tokens"));
        assert!(index.variables.contains("max_tokens"));
        assert!(index.variables.contains("current_batch"));
        assert!(index.variables.contains("token_id"));
    }

    #[test]
    fn test_query_matches_keywords_and_symbols() {
        let mut index = FileSymbolIndex::new();
        index.classes.insert("ModelLoader".to_string());
        index.variables.insert("mode_state".to_string());

        // Query "mo"
        let matches = index.query_matches("mo");
        let names: Vec<String> = matches.into_iter().map(|s| s.name).collect();
        assert!(names.contains(&"ModelLoader".to_string()));
        assert!(names.contains(&"mode_state".to_string()));

        // Query "de" -> should match keyword "def" and "del"
        let kw_matches = index.query_matches("de");
        let kw_names: Vec<String> = kw_matches.into_iter().map(|s| s.name).collect();
        assert!(kw_names.contains(&"def".to_string()));
        assert!(kw_names.contains(&"del".to_string()));
    }
}
