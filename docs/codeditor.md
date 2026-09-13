# High-Performance Python Code Editor for Tomy

Implement an ultra-responsive, interactive terminal code editor for **Tomy** with dedicated Python syntax highlighting, 4-space tab indentation, line numbering gutter, hardware cursor tracking, and an extensive hardcoded Python demonstration script.

---

## User Review Required

> [!IMPORTANT]
> **Homescreen Menu Update**: We are introducing `"💻 Code Editor"` as a dedicated option on the Tomy homescreen. The existing `"💻 Code"` option (which serves as the small model coding harness preview) will remain or be renamed to `"⚡ Code Harness"` to prevent user confusion between the interactive editor and the LLM coding harness.
>
> **Font Rendering in Terminals**: Terminal user interfaces (Ratatui / Crossterm) emit ANSI styling sequences to the host terminal emulator (e.g., Alacritty, Kitty, WezTerm, GNOME Terminal). The system already has **JetBrains Mono NL** installed (`JetBrains Mono NL, Light, Medium, Bold, etc.`). The editor layout, line numbers, and box drawing are engineered specifically for JetBrains Mono's geometry and spacing.
>
> **Performance Architecture**: To make the editor "fast as fuck", tokenization is implemented as a single-pass zero-allocation lexer executed **only on the visible lines within the active viewport**, keeping render cycle latency under 0.1ms at 60 FPS even on massive files.

---

## Open Questions

None currently blocking. Default behaviors planned:
- `Tab` inserts 4 spaces (`"    "`).
- `BackTab` (`Shift+Tab`) unindents current line by up to 4 spaces.
- `Backspace` at a 4-space boundary removes 4 spaces.
- `Enter` carries over indentation from the previous line.
- `Esc` returns to the Tomy Homescreen.

---

## Proposed Changes

### Core Terminal Engine & Navigation

#### [MODIFY] [home.rs](file:///home/avisheks/Documents/rust-projects/tomy/src/home.rs)
- Add `Editor` to the `HomeOption` enum.
- Update `HomeOption::ALL` to include `Editor` with title `"💻 Code Editor"` and description `"High-performance interactive code editor with real-time Python syntax highlighting."`.
- Keep existing menu options (`Chat`, `Code Harness`, `Todo`, `Settings`) fully functional.

#### [MODIFY] [app.rs](file:///home/avisheks/Documents/rust-projects/tomy/src/app.rs)
- Add `CurrentScreen::Editor` to the screen router enum.
- Instantiate `EditorScreen` in `App::new()`.
- Route key events to `editor.handle_key(key)` and mouse scroll events to `editor.scroll()`.
- Add command hints for the Editor screen: `[Tab] 4 Spaces`, `[↑/↓/←/→] Navigate`, `[Enter] Newline`, `[Esc] Main Menu`.
- Render the editor when `self.current_screen == CurrentScreen::Editor`.

#### [MODIFY] [main.rs](file:///home/avisheks/Documents/rust-projects/tomy/src/main.rs)
- Register `mod editor;` in the module hierarchy.

---

### Code Editor Implementation

#### [NEW] [editor.rs](file:///home/avisheks/Documents/rust-projects/tomy/src/editor.rs)
Create the dedicated `EditorScreen` struct and high-performance Python syntax highlighter:

1. **Editor Buffer & State**:
   - `lines: Vec<String>`: Line-based buffer holding document contents.
   - `cursor_row: usize`, `cursor_col: usize`: Active cursor coordinates in character units.
   - `preferred_col: usize`: Remembers intended horizontal column when navigating across shorter lines.
   - `scroll_row: usize`, `scroll_col: usize`: Viewport window offsets for horizontal and vertical scrolling.
   - `is_modified: bool`: Change detection indicator.

2. **Zero-Allocation Python Lexer & Syntax Highlighter**:
   - Ultra-fast single-pass character scanner converting raw lines into `ratatui::text::Line` with styled `Span`s.
   - **Keywords** (`def`, `class`, `import`, `from`, `as`, `return`, `if`, `elif`, `else`, `for`, `while`, `try`, `except`, `finally`, `with`, `async`, `await`, `lambda`, `yield`, `pass`, `break`, `continue`, `raise`, `in`, `is`, `not`, `and`, `or`, `assert`, `match`, `case`) -> `Color::Magenta`, `Modifier::BOLD`.
   - **Built-in Functions & Types** (`print`, `len`, `range`, `enumerate`, `zip`, `isinstance`, `str`, `int`, `float`, `bool`, `list`, `dict`, `set`, `tuple`, `open`, `super`, `RuntimeError`, etc.) -> `Color::Cyan`.
   - **Constants & Specials** (`True`, `False`, `None`, `self`, `cls`) -> `Color::LightYellow`, `Modifier::ITALIC`.
   - **Decorators** (`@dataclass`, `@property`, `@...`) -> `Color::Yellow`.
   - **Function & Class Declarations** (`def my_func`, `class MyClass`) -> Distinct `Color::LightBlue` and `Color::LightYellow` bold titles.
   - **Strings**:
     - Single & double quoted strings (`'...'`, `"..."`).
     - Prefixes (`f"..."`, `r"..."`, `b"..."`).
     - Triple-quoted docstrings (`"""..."""`, `'''...'''`) with cross-line multiline state tracking.
     - Color: `Color::Green`.
   - **Comments** (`# ...`) -> `Color::DarkGray`, `Modifier::ITALIC`.
   - **Numbers** (Integers, Decimals, Hex `0x...`, Binary `0b...`) -> `Color::LightCyan` / `Color::Yellow`.
   - **Operators & Punctuation** (`+`, `-`, `*`, `/`, `->`, `:`, `==`, `!=`, etc.) -> `Color::LightBlue` / `Color::White`.

3. **Tab & Indentation Handling**:
   - `Tab` key inserts 4 spaces `"    "` and advances cursor by 4.
   - `BackTab` (`Shift+Tab`) removes up to 4 leading spaces for unindenting.
   - Smart `Backspace`: if the preceding 4 characters are spaces aligned to a 4-space tab stop, removes all 4 spaces in one stroke.
   - `Enter` key: splits the line and carries forward the indentation level of the preceding line (auto-indent).

4. **Font Size Scaling for Code Text (`Ctrl +` / `Ctrl -`)**:
   - `Ctrl + "+"` (or `Ctrl + "="`): Increases code font size by +1 pt (up to 28 pt).
   - `Ctrl + "-"` (or `Ctrl + "_"`): Decreases code font size by -1 pt (down to 8 pt).
   - **Visual Scale for Code Only**: Higher font sizes (`>= 16 pt`) apply bolder typography and heightened contrast to code tokens; lower font sizes (`<= 12 pt`) provide compact representation.
   - **Transient HUD Toast**: Renders a floating popup (`🔍 Code Font Size: X pt`) for 1.5 seconds upon adjustment.
   - **Dynamic Readouts**: Live font size displayed in both the header bar and the bottom status bar.

5. **Hardcoded Python Demonstration Code**:
   - A realistic 85+ line production script: An asynchronous on-device LLM streaming inference engine (`LocalInferenceEngine`) featuring dataclasses, enums, async generators, timing decorators, typing hints, docstrings, list comprehensions, exception handling, and KV-cache simulation.

6. **Visual Layout & UI**:
   - **Top Header Bar**: Shows filename `engine.py`, modified status badge `●`, encoding `UTF-8`, language `Python 3.12`, font hint with live point size `JetBrains Mono NL (14 pt)`.
   - **Line Number Gutter**: Left-side gutter with 4-digit aligned line numbers (` 1 │`, ` 2 │`, ...), active line highlighted in `Color::Yellow`.
   - **Main Code Canvas**: Syntax-highlighted code with horizontal/vertical scrolling.
   - **Hardware Terminal Cursor**: Synchronized via `frame.set_cursor_position(...)` for crisp blinking cursor rendering in the terminal.
   - **Status Bar**: Displays mode `[EDITING]`, `Ln X, Col Y`, `Font: 14 pt`, `Spaces: 4`, total lines, and keyboard command hints (`[Ctrl +/-] Font Size`).

---

## Required Features for a Production Terminal Code Editor

Beyond basic syntax highlighting and cursor navigation, the following features are required for a complete, production-grade terminal code editor:

```
┌────────────────────────────────────────────────────────────────────────┐
│               Terminal Code Editor Architecture Roadmap                │
├────────────────────────────────────────────────────────────────────────┤
│ 1. File I/O & Buffers      │ Open, save (Ctrl+S), file watch, auto-save│
│ 2. Undo/Redo Engine        │ Command history stack, typing coalescing  │
│ 3. Search & Replace        │ Regex search, match highlighting, replace │
│ 4. Visual Selection        │ Shift+Arrows selection, block selection   │
│ 5. Clipboard Integration   │ OSC 52 / system clipboard (arboard)       │
│ 6. Language Server (LSP)   │ pyright / ruff diagnostics, completion    │
│ 7. Tree-sitter AST         │ Structural folding, precise scope analysis│
│ 8. Git & Diff Gutter       │ Changed (+), modified (~), deleted lines  │
│ 9. Command Palette         │ Quick file fuzzy finder (Ctrl+P)          │
└────────────────────────────────────────────────────────────────────────┘
```

1. **File I/O & Buffer Persistence**:
   - File opening and saving (`Ctrl+S`, `Ctrl+O`, `:w`, `:q`).
   - CLI arguments: `tomy edit path/to/file.py`.
   - File modification monitoring (alert if file was modified externally).
2. **Undo / Redo History Stack**:
   - Reversible action stack (`Ctrl+Z` / `Ctrl+Y` or `u` / `Ctrl+R`) using the Command pattern.
   - Grouping / coalescing sequential keystrokes so undo reverts words or blocks rather than single characters.
3. **Interactive Search & Replace**:
   - In-buffer search (`Ctrl+F`) with real-time highlighting of all matches.
   - Jump between matches with `Enter` / `Shift+Enter` (or `n` / `N`).
   - Find and Replace dialog (`Ctrl+H`).
4. **Text Selection & Clipboard (OSC 52 / System)**:
   - Visual mode / `Shift+Arrows` range selection.
   - Copy (`Ctrl+C`), Cut (`Ctrl+X`), Paste (`Ctrl+V`).
   - Terminal clipboard synchronization via OSC 52 escape codes and the `arboard` crate for native OS clipboard integration.
5. **Language Server Protocol (LSP) Client**:
   - Communication with `pyright`, `pylsp`, or `ruff` via JSON-RPC over `stdio`.
   - Real-time diagnostic squiggles (errors in red, warnings in yellow).
   - Autocompletion popup menu with documentation tooltips.
   - "Go to Definition" and "Find References".
6. **Tree-sitter Parsing & Code Folding**:
   - Incremental parsing using `tree-sitter-python`.
   - Code folding for functions, classes, and blocks (`[+]` / `[-]` indicators in gutter).
7. **Git Gutter Integration**:
   - Parsing git diffs against `HEAD`.
   - Visual gutter indicators: green `+` for added lines, blue `~` for modified lines, red `_` for deleted lines.
8. **Fuzzy File Picker & Command Palette**:
   - `Ctrl+P` fuzzy search across project directory files.
   - `Ctrl+Shift+P` command palette for editor actions, themes, and configuration.

---

## Verification Plan

### Automated Tests
- Run `cargo test` to verify:
  - Python tokenizer tests:
    - Keywords (`def`, `class`, `return`, `async`, etc.)
    - Built-in functions (`print`, `len`, `range`, `isinstance`)
    - String literals (single, double, f-strings, triple-quoted docstrings)
    - Comments (`# comment`)
    - Numbers (integers, floats, hex)
  - Editor buffer operations:
    - Tab key inserts 4 spaces.
    - Shift+Tab unindents line.
    - Backspace at 4-space boundary deletes 4 spaces.
    - Enter key preserves indentation of previous line.
    - Line joining and line splitting at cursor.
    - Viewport scrolling bounds.
  - App navigation:
    - Homescreen navigation to `Code Editor` and pressing `Enter`.
    - `Esc` returns cleanly from Code Editor to Homescreen.

```bash
cargo test
```

### Manual Verification
1. Launch `cargo run`.
2. Verify that `"💻 Code Editor"` appears in the Tomy Homescreen menu.
3. Navigate to `"💻 Code Editor"` and press `Enter`.
4. Confirm the long hardcoded Python script renders with vivid syntax highlighting (keywords in magenta, strings in green, builtins in cyan, docstrings in green, comments in dark gray, decorators in yellow).
5. Verify line number gutter on the left with active line highlighted.
6. Verify hardware blinking cursor positioning.
7. Test typing new code, pressing `Tab` (inserts 4 spaces), pressing `Enter` (indents automatically), and using `Backspace` (deletes 4 spaces).
8. Use arrow keys (`↑`, `↓`, `←`, `→`) to scroll and navigate through the 85+ line document.
9. Press `Esc` to return to the Homescreen.
