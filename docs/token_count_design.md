# Tomy Token Counter & File Operations Design

This document details the architecture and implementation for the high-performance global Tiktoken token counter and the in-TUI file operations (file creation and deletion) in Tomy.

---

## 1. Global Token Counter (`src/token_counter.rs`)

### Overview
A production-grade, zero-allocation token counting module using `tiktoken = "3"`, `memmap2 = "0.9"`, and `rayon = "1.10"`. It scales seamlessly from small scripts to multi-gigabyte codebases.

### Architecture & Scaling Strategy
1. **Model Management**:
   - Initialized once in memory. Supports dynamic switching between `cl100k_base` (GPT-4 / modern standard) and `o200k_base` (GPT-4o) via an atomic/synchronized global state.
   - Configurable explicitly in the **Settings** screen.
2. **Hybrid I/O Pipeline**:
   - **$< 64\text{ KB}$ (Small Files)**: Fast direct read into memory via `std::fs::read_to_string` avoiding kernel page-table setup and mmap fault overhead.
   - **$64\text{ KB} - 256\text{ KB}$ (Medium Files)**: Zero-copy memory mapping via `memmap2::Mmap` directly into memory without heap duplication.
   - **$> 256\text{ KB}$ (Large Files)**: Memory-mapped parallel chunking across CPU cores using Rayon (`par_lines()`). Splitting strictly on newline boundaries preserves multi-byte tokens safely.
3. **In-Memory Buffer Counting (`count_tokens_str`)**:
   - Computes token counts directly for editor buffer lines and runner stdout/stderr without disk I/O overhead.

---

## 2. File Explorer Operations (`src/explorer.rs`)

### 1. `Ctrl + N` (New File Creation)
- **Modal Input**: Interactive TUI popup prompting the user for a new file name or relative path (e.g. `engine.py`, `utils/helpers.py`).
- **Disk Creation**: Creates the file (and parent directories if needed).
- **Visual Feedback**:
  - Highlights newly created files with a distinct **50% blue background** (`Color::Rgb(30, 60, 110)`) in the tree view so the user can easily spot recently created files.
- **Immediate Navigation**: Once created, Tomy immediately transitions to the **Code Editor** with the new file opened and ready for typing.

### 2. `Delete` (File & Folder Deletion)
- **File Deletion**:
  - 1-step confirmation prompt: `"Are you sure you want to delete '<filename>'? [y/N]"`
  - Deletes the file, refreshes the tree, updates the preview pane.
- **Folder Deletion**:
  - **Double Confirmation** protection:
    - Step 1: `"Are you sure you want to delete folder '<folder_name>'? [y/N]"`
    - Step 2: `"This folder contains important files, are you absolutely sure to delete it? [y/N]"`
  - Deletes the directory recursively via `std::fs::remove_dir_all`.

---

## 3. UI Token Metrics Integration

1. **Code Editor Status Bar (`src/editor.rs`)**:
   - In the bottom status bar next to `Chars: ...`:
   - `│ Tokens: <count>` (e.g., `│ Tokens: 1,420`).
   - Automatically updated when opening a file, saving, or editing text.
2. **File Explorer Preview Inspector (`src/explorer.rs`)**:
   - In the right-hand pane header row on the exact same line as `Size:`:
   - `File: <name>  │  Size: <size>  │  Tokens: <count>`.
3. **Python Output Runner (`src/runner.rs`)**:
   - Output canvas title: `Output (X lines │ Y tokens)`.
   - Header banner: `│ Tokens: <count>`.
4. **Settings Screen (`src/settings.rs`)**:
   - Interactive option to select active BPE Tokenizer encoding: `cl100k_base` vs `o200k_base`.
