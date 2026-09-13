# Tomy Command Reference

Every screen in Tomy features a persistent command bar at the bottom of the terminal to clearly display available keybindings.

---

## 1. Global Commands

| Key Combination | Action | Scope |
| :--- | :--- | :--- |
| `Ctrl + C` | Force quit application | Global (Any screen) |

---

## 2. Homescreen Commands

| Key | Action |
| :--- | :--- |
| `↑` or `k` | Navigate selection up |
| `↓` or `j` | Navigate selection down |
| `Enter` | Enter selected screen (`Chat`, `Code`, `To-do List`, `Settings`) |
| `q` | Quit application |

---

## 3. To-do List Commands

### Normal Mode

| Key | Action |
| :--- | :--- |
| `a` or `n` | Open **Add Task** dialog |
| `Space` or `Enter` | Toggle completed / pending status (switches between **bold green** and **red**) |
| `d`, `x`, or `Delete` | Remove/delete currently selected task |
| `↑` or `k` | Move selection up |
| `↓` or `j` | Move selection down |
| `Esc` | Return to primary **Homescreen** |

### Adding Mode (Dialog Active)

| Key | Action |
| :--- | :--- |
| `Char keys` | Type task text into input field |
| `Backspace` | Delete previous character |
| `Enter` | Confirm and add task |
| `Esc` | Cancel dialog and return to normal mode |

---

## 4. Chat Screen Commands

### Normal Chat Mode

| Key / Input | Action |
| :--- | :--- |
| `Char keys` | Type prompt into the blue-bordered input bar |
| `Backspace` | Delete previous character |
| `Enter` | Send prompt to model (or trigger `/clear` if entered) |
| `↑` / `↓` | Scroll conversation container history |
| `Mouse Wheel` | Scroll conversation container history up or down |
| `Esc` | Return to primary **Homescreen** |

### Clear Confirmation Mode (Triggered by typing `/clear`)

| Key | Action |
| :--- | :--- |
| `y` | Confirm and wipe session chat history |
| `n` or `Esc` | Cancel reset and return to normal chat |

---

## 5. Other Secondary Screens (Code, Settings)

| Key | Action |
| :--- | :--- |
| `Esc` | Return to primary **Homescreen** |
