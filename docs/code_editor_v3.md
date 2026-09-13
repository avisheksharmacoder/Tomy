# Tomy Code Editor v3: Ruff Integration & In-Terminal Autocompletions

This document outlines the architecture, formatting/linting pipeline, and autocompletion engine for the Tomy Python Code Editor.

## 1. Ruff In-Process Formatting & Strict Linting

### Overview
Tomy uses Astral's Ruff engine natively in-process via compiled Rust crates:
- `ruff` (v0.16.7)
- `ruff_python_formatter` (v0.0.13)
- `ruff_linter` (v0.16.7)
- `ruff_python_ast` (v0.0.13)
- `ruff_python_parser` (v0.0.13)

### Save Workflow (`Ctrl+S`)
1. **Formatting**: Runs `format_module_source` with standard PEP 8 / Black style formatting rules.
2. **Strict Lint Auto-Fixing**: Runs `lint_fix` to automatically resolve fixable linting issues (import sorting, dead code removal, style fixes).
3. **Buffer Synchronization**: The editor's line buffer (`self.lines`) is seamlessly updated to reflect the newly formatted code. The hardware terminal cursor is clamped to valid row and column boundaries.
4. **Disk Persistence**: The formatted content is saved to disk, and a high-visibility toast (`✔ FORMATTED & LINTED WITH RUFF`) is shown in the editor status bar.
5. **Syntax Error Resilience**: If code has syntax errors during save, raw content is safely saved to disk to prevent data loss, and an informative status toast (`⚠ Saved with Syntax Error (Line X, Col Y)`) indicates the issue.

---

## 2. In-Terminal Python Autocompletion

### Scope
Autocompletion is scoped strictly to the current active file buffer — zero external network or heavy library indexing overhead:
1. **Python Keywords**: Complete set of Python keywords (`def`, `class`, `return`, `if`, `elif`, `else`, `for`, `while`, `try`, `except`, `finally`, `with`, `async`, `await`, `lambda`, `yield`, `import`, `from`, `as`, `in`, `is`, `not`, `and`, `or`, `pass`, `break`, `continue`, `raise`, `assert`, `match`, `case`, `global`, `nonlocal`, `type`) and special constants (`True`, `False`, `None`, `self`, `cls`).
2. **Class Names**: All class definitions (`class <ClassName>:`) extracted from the current file buffer.
3. **Variable Names**: All variable declarations, assignments (`var = ...`), function parameter names (`def func(param1, param2):`), and iteration targets (`for item in items:`, `with ... as f:`) found in the file.

### Trigger & Interaction
- **Trigger**: Automatically activates when typing an identifier with a minimum length of **1 character**.
- **Keyboard Controls**:
  - `Tab` / `Enter`: Accept and insert the currently selected completion.
  - `↓` / `↑` (or `Ctrl+N` / `Ctrl+P`): Navigate through the suggestion list.
  - `Esc`: Dismiss the suggestion popup.
  - Normal typing: Dynamically narrows candidates; closes if no matches remain.
  - When popup is inactive: `Tab` preserves standard 4-space indentation.

### UI & Styling
- Rendered as a floating popup box positioned directly beneath the active cursor line (or above if near the terminal bottom).
- Badges:
  - `[kw]` in Magenta
  - `[class]` in Yellow
  - `[var]` in LightCyan
- The highlighted item features an inverted highlight bar.
