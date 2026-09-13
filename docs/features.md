# Tomy Features

**Tomy** is a high-performance terminal workspace and harness designed for small Large Language Models (LLMs) (e.g., SmolLM2, Qwen2.5-Coder, Llama 3.2, Phi-3.5). Built on `ratatui` and `crossterm`.

---

## 1. Modular Architecture

The codebase is split into modular files under `src/`:

- **`main.rs`**: Terminal lifecycle management, raw mode setup, safe panic restoration hook, and the main event loop.
- **`app.rs`**: Central application state, screen router, key event dispatcher, and unified footer command bar renderer.
- **`home.rs`**: Main interactive menu with title banner, description panel, and arrow navigation.
- **`todo.rs`**: Interactive To-do List UX for development workflows, model benchmarking items, and experiment tracking.
- **`chat.rs`**: Conversational interface preview for small LLMs.
- **`code.rs`**: Code assistant and prompt generation harness preview.
- **`settings.rs`**: Configuration preview for local model providers, context windows, and sampling parameters.

---

## 2. Interactive To-do List UX

The To-do list feature provides an interactive task manager:

- **Status Styling**:
  - **Completed tasks**: Marked with `[✓]` in **bold green** (`Color::Green`, `Modifier::BOLD`).
  - **Pending tasks**: Marked with `[ ]` in **red** (`Color::Red`).
- **Interactive Management**:
  - Add tasks with interactive modal prompt.
  - Delete selected tasks.
  - Toggle completion status with `Space` or `Enter`.
- **Live Statistics**:
  - Displays dynamic counters: Total tasks, Completed tasks, and Pending tasks.
- **Navigation & Escape**:
  - Navigate list with arrow keys (`↑`/`↓`) or `k`/`j`.
  - Press `Esc` at any time to return to the primary Homescreen.

---

## 3. Interactive Chat Container UX

The Chat screen features a container-based conversation interface:

- **Message Bubbles**:
  - **User Prompts**: Displayed in square cards with a 1px **Blue** border (`Color::Blue`), titled ` You `.
  - **Assistant Responses**: Displayed in square cards with a 1px **Green** border (`Color::Green`), titled ` Tomy (SmolLM2) `.
- **Text Input Bar**:
  - Located at the bottom with a 1px **Blue** border (`Color::Blue`).
  - Interactive typing with cursor indicator (`█`), backspacing, and submission with `Enter`.
- **Scrolling**:
  - Supports both **Mouse Wheel** scrolling and keyboard arrow keys (`↑` / `↓`).
  - Auto-scrolls to the bottom upon receiving new responses.
- **Session Reset (`/clear`)**:
  - Type `/clear` and press `Enter` to open a confirmation modal: `⚠️ Clear chat history? [y] Confirm  [n/Esc] Cancel`.
  - Pressing `y` wipes the conversation history.

---

## 4. Other Secondary Screens

- **Code Screen**: Syntax-highlighted mock code buffer and prompt evaluation area.
- **Settings Screen**: Inspection view for local GGUF paths, context window lengths, CPU thread allocations, and sampling settings.
- All secondary screens support the uniform `Esc` key to seamlessly return to the main Homescreen.

