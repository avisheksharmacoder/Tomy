# Elite Code Workspace: Project File Explorer & Dual-Terminal Output Runner

Transform Tomy's Code Editor into an elite development environment featuring a prompt initialization modal, a 2-column interactive tree file explorer with strict color hierarchies (blue folders, orange files, purple expanded files), and a dual-terminal scratchpad runner with real-time Ratatui output inspection, `Ctrl + R` code execution, and `Ctrl + C` clipboard copying.

---

## Architecture & Workflow Overview

### 1. Workflow Initialization on Homescreen
When selecting `"💻 Code Editor"` from the homescreen, instead of immediately showing hardcoded Python code, Tomy displays an interactive modal prompt:
1. `[1] 📁 Open Project Source Folder`: Launches Linux native GUI folder picker (`zenity`) to select a workspace (with in-terminal path fallback), then displays a 2-column Ratatui file explorer.
2. `[2] 🧪 Scratch Script & Dual-Terminal Runner`: Creates a temporary Python script and spawns a secondary terminal window (`gnome-terminal`) running a dedicated Ratatui output runner that stays open and syncs as long as the editor is active.

### 2. Color Hierarchy in 2-Column File Explorer
- **Folders**: Styled in **Blue** (`Color::Cyan` / `Color::Blue`) with `📁 ` prefix.
- **Top-level Files**: Styled in **Orange** (`Color::Rgb(255, 140, 0)`) with `📄 ` prefix.
- **Expanded Folder Files**: When pressing `Enter` on a folder, it expands inline below it, displaying its nested files in **Purple** (`Color::Magenta` / `Color::Rgb(180, 100, 255)`).

### 3. Execution & Navigation Hierarchy
- **Code Execution (`Ctrl + R`)**: Pressing `Ctrl + R` inside the code editor executes the script in the secondary Ratatui runner terminal and displays live output.
- **Clipboard Output (`Ctrl + C`)**: Pressing `Ctrl + C` in the runner terminal window copies the stdout/stderr output directly to the system clipboard via `xclip` and OSC 52.
- **Esc Navigation Hierarchy**:
  - Code Editor -> `Esc` -> Returns to Ratatui File Explorer (or Home if opened via Scratch).
  - File Explorer -> `Esc` -> Returns to Homescreen Menu.

---

## Detailed Components

### 1. Subcommand Support & Terminal Lifecycle (`src/main.rs`)
- Inspect CLI arguments (`std::env::args()`):
  - If invoked with `tomy runner <file_path>`, skip the main Tomy menu and enter the dedicated **Runner TUI** loop directly.
  - Otherwise, run the normal Tomy interactive workspace.

### 2. 2-Column File Explorer & Tree Navigation (`src/explorer.rs`)
- `FileExplorerScreen` managing project files:
  - **Left Column (45% width)**: Scrollable interactive tree with visual indentation guides (`├─`, `└─`). Folders in Blue, Files in Orange, Nested Files in Purple upon expansion.
  - **Right Column (55% width)**: File preview and metadata pane (file size, line count, permissions, live syntax preview of top 40 lines).
  - Arrow navigation (`↑`/`↓`/`k`/`j`), `Enter` to expand/collapse or open in Editor, `Esc` to return to Home.

### 3. Editor Disk I/O, Run Trigger & Runner Lifecycle (`src/editor.rs`)
- `open_file(path: &Path)`: Loads actual file content into editor buffer.
- `save_file()`: Writes buffer to disk upon `Ctrl + S`.
- `Ctrl + R`: Triggers code execution in the dual-terminal runner.
- `opened_from_explorer`: Tracks whether `Esc` returns to File Explorer or Home.
- `runner_child`: Holds `Child` process handle for secondary terminal, terminating it cleanly when editor is closed.

### 4. Dual-Terminal Output Runner (`src/runner.rs`)
- Dedicated standalone Ratatui runner application:
  - Header: `⚡ Tomy Execution Runner`, watched file, execution status.
  - Scrollable canvas: Real-time stdout & stderr streams with exit code and timing.
  - Commands: `[Ctrl+C] Copy Output`, `[r] Re-run`, `[q/Esc] Quit`.

### 5. Initialization Modal & Routing (`src/app.rs`)
- `CurrentScreen::EditorPrompt` modal displaying Options `[1]` and `[2]`.
- Native `zenity` invocation with fallback.
- Spawning of `gnome-terminal --title="Tomy Output Runner" -- <current_exe> runner <file>` with `x-terminal-emulator` fallback.
