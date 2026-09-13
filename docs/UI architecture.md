# Tomy UI Architecture & Chat Container Implementation Plan

This document details the architectural roadmap for **Tomy**, a Rust-based Terminal User Interface (TUI) harness for small Large Language Models (LLMs) built on `ratatui` and `crossterm`. It covers the modular screen system, the To-do List UX, and the container-based **Chat UX**.

---

## 1. Required Libraries for the Small LLM Harness

To transform `tomy` into a small LLM harness while maintaining 60 FPS responsive terminal rendering, we will require the following libraries:

| Library | Category | Purpose |
| :--- | :--- | :--- |
| `tokio` (features: `rt-multi-thread`, `macros`, `sync`) | Async Runtime | Keeps the TUI responsive by offloading model loading and token generation to background tasks, communicating via channels (`tokio::sync::mpsc`). |
| `serde` & `serde_json` | Persistence / Data | Serializing/deserializing To-do lists, chat histories, prompt templates, and model configuration files. |
| `directories` | System Paths | Determining cross-platform paths for storing app state, configs, and model caches (`~/.config/tomy/`, `~/.local/share/tomy/`). |
| `tui-textarea` | UI Widget | Multi-line text input widget for editing code snippets and crafting rich chat prompts inside Ratatui. |
| `pulldown-cmark` or `tui-markdown` | Markdown Parser | Rendering formatted LLM responses (headings, bullet lists, bold text, code blocks) inside the terminal. |
| `syntect` | Syntax Highlighting | Syntax highlighting for code completions in the `Code` UI screen. |

> [!NOTE]
> **Scope Roadmap:**
> - Full LLM streaming will be implemented in future milestones with `tokio` channels.
> - For the current milestone, LLM responses in the Chat interface are simulated/mocked for on-device harness evaluation.

---

## 2. Modular File Structure

Each primary screen resides in its own dedicated file inside `src/`:

```
src/
├── main.rs         # Terminal lifecycle, crossterm raw mode setup/restore, mouse capture, event loop
├── app.rs          # App state, active screen router, global key & mouse event dispatcher
├── home.rs         # Homescreen UI (Chat, Code, To-do List, Settings), menu navigation
├── chat.rs         # Interactive Chat Container UX (square bubbles, blue input bar, /clear command)
├── code.rs         # Code harness UI (prompt input, code preview, generation actions)
├── todo.rs         # Interactive To-do List UX (Add, Delete, Toggle completed, bold green/red styling)
└── settings.rs     # Settings UI (Model configuration, provider, parameters)
```

---

## 3. Chat Container UX Specifications

### Message Bubble Styling
- **User Messages:** Rendered inside a clean, squared card with a 1px **Blue** border (`Borders::ALL`, `BorderType::Plain`, `Color::Blue`), titled ` You `.
- **Assistant Responses:** Rendered inside a clean, squared card with a 1px **Green** border (`Borders::ALL`, `BorderType::Plain`, `Color::Green`), titled ` Tomy (SmolLM2) `.

### Blue-Bordered Text Input Bar
- Positioned at the bottom with a 1px **Blue** border (`Color::Blue`).
- Displays typed prompt characters with a cursor indicator (`█`).
- Placeholder hint: `Type prompt and press Enter... (type '/clear' to wipe)`.

### Scrolling Controls
- Users can scroll through conversation history using:
  - **Mouse Wheel** (`ScrollUp` / `ScrollDown`).
  - **Arrow Keys** (`↑` / `↓`).
- New prompts automatically snap the view to the latest messages.

### Session Clearing (`/clear`)
- Entering the command `/clear` into the text prompt triggers a confirmation modal:
  `⚠️ Clear chat history? [y] Confirm  [n/Esc] Cancel`
- Pressing `y` empties the chat history.
- Pressing `n` or `Esc` cancels the operation safely without clearing.

---

## 4. Verification Plan

### Automated Verification
- Unit tests covering:
  - Text input handling & Backspace editing.
  - Sending prompts and appending User & Assistant messages.
  - Activating `/clear` confirmation modal.
  - Confirming `/clear` wiping history.
  - Cancelling `/clear` preserving history.
  - Scroll position boundary checks.

### Manual Verification
- Launch `cargo run --release`.
- Select **Chat** and press `Enter`.
- Verify the blue-bordered text input bar at the bottom.
- Enter a prompt and press `Enter`.
- Confirm the sent prompt appears in a square blue box and the response appears in a square green box.
- Use mouse wheel and `↑`/`↓` keys to scroll.
- Type `/clear`, press `Enter`, and confirm deletion with `y`.
- Press `Esc` to return to the Homescreen.
