# AI Agent Instructions: ComputerUse

This document provides concise instructions and operational guidelines for autonomous AI agents controlling a Linux desktop running **KDE Plasma 6 on Wayland**.

You are equipped with two standalone native binaries:
1. `capture`: Vision, screenshots, region cropping, coordinate grids, perceptual motion detection, local OCR with bounding boxes, and video recording.
2. `interact`: Input injection (uinput + libei), AT-SPI2 semantic accessibility tree inspection, KWin window tiling & geometry control, bidirectional clipboard, notification interception, and inotify download tracking.

---

## 🔄 The Recommended Agent Loop

For optimal speed, accuracy, and reliability, follow this loop:

1. **Manage Window Layout & Focus**
   Never waste steps visually searching for window titles or dock icons:
   ```bash
   interact focus chrome
   interact tile chrome left
   ```
   To list all active application windows:
   ```bash
   interact windows
   ```

2. **Perceive the State (Visual, Semantic, or OCR)**
   - **Semantic element search (fastest, zero vision tokens):**
     ```bash
     interact find "Submit"
     interact click-element "Submit"
     ```
   - **Full visual overview:** `capture screen.png`
   - **Targeting small buttons / uncertain coords:** `capture --grid screen.png`
     *(Every 100px intersection has a labeled tag like `500,300`. Read coordinates directly from the grid!)*
   - **Local OCR with exact bounding boxes:**
     ```bash
     capture ocr screen.png --json
     ```
   - **Zooming in on a known area:** `capture --region X,Y,W,H region.png`

3. **Act with Chained Batch Execution (`run`)**
   Always prefer `interact run "..."` over separate shell commands. A single `run` keeps the virtual device alive and executes actions in milliseconds:
   ```bash
   interact run "hotkey ctrl+t; sleep 200; paste https://example.com; key enter"
   ```

4. **Wait for Completion Reactively (No Blind Sleeps!)**
   - **Wait for page load / animation to settle:** `capture wait-settle --settle 200`
   - **Wait for button click to trigger a change:** `capture wait-change --timeout 3000`
   - **Wait for file download to finish:** `interact wait-download ~/Downloads --timeout 30`
   - **Wait for desktop notification / 2FA code:** `interact wait-notification --timeout 15000`

---

## ⚡ Critical Rules for AI Agents

- **Rule 1: Always use `paste` for URLs, text, and code.**
  Do NOT use `type` for URLs or strings containing punctuation (`:`, `/`, `@`, `-`, etc.). The host system may use a non-US keyboard layout (e.g. German QWERTZ). `interact paste "..."` uses the system clipboard and is 100% immune to keyboard layouts.
- **Rule 2: Read clipboard with `interact clipboard`.**
  After selecting text and pressing `hotkey ctrl+c`, read the copied text directly via `interact clipboard`.
- **Rule 3: Use semantic clicks when possible (`click-element`).**
  If an application exposes accessibility (browsers, Qt, GTK apps), `interact click-element "<NAME>"` clicks the exact center without visual trial-and-error.
- **Rule 4: Reactive waiting beats fixed sleeps.**
  Instead of guessing how many seconds a page or download takes, use `capture wait-settle` or `interact wait-download`.
- **Rule 5: Standard keyboard shortcuts are instant & infallible:**
  - `hotkey ctrl+t` = New browser tab.
  - `hotkey ctrl+w` = Close current tab.
  - `hotkey ctrl+l` = Focus browser URL/search bar.
  - `hotkey alt+tab` = Switch active window.
  - `hotkey ctrl+c` / `hotkey ctrl+v` = Copy / Paste.

---

## 📖 Command Quick Reference

### `capture`

| Command | Arguments | Purpose |
| :--- | :--- | :--- |
| `capture <file.png>` | `[-r RES] [-g]` | Capture desktop at standard 1080p |
| `capture --grid <file.png>` | `[SPACING]` | Capture with 100px coordinate grid & micro-labels |
| `capture --region <X,Y,W,H> <file.png>` | | Crop capture directly to bounding box without scaling |
| `capture wait-change` | `[-c REGION] [--timeout MS]` | Block until on-screen pixels change |
| `capture wait-settle` | `[-c REGION] [--settle MS]` | Block until animations finish and screen stabilizes |
| `capture ocr [IMAGE]` | `[--json]` | Extract on-screen text with `[x, y, w, h]` bounding boxes |
| `capture record <FILE.mp4>` | `[--duration SEC]` | Record desktop video snippet (ffmpeg x11grab) |

### `interact`

| Command | Arguments | Purpose |
| :--- | :--- | :--- |
| **Mouse & Cursor** | | |
| `click` | `<X> <Y> [BTN] [double]` | Move to $(X, Y)$ and click (`left`, `right`, `middle`) |
| `click-element` | `<QUERY>` | Find element in AT-SPI2 tree and click its center |
| `move` | `<X> <Y>` | Move cursor to absolute pixel coordinates |
| `move-rel` | `<DX> <DY>` | Move cursor relative to current position |
| `drag` | `<X1> <Y1> <X2> <Y2> [N]` | Smooth mouse drag from $(X_1, Y_1)$ to $(X_2, Y_2)$ |
| `scroll` | `<DELTA>` | Scroll mouse wheel (negative = down, positive = up) |
| `mouse-down` / `mouse-up` | `[BTN]` | Press or release mouse button without releasing grab |
| **Keyboard & Clipboard** | | |
| `paste` | `<TEXT>` | Paste text via clipboard (layout-independent) |
| `type` | `<TEXT>` | Type text using hardware keycodes |
| `clipboard` | | Read and print current clipboard buffer |
| `key` | `<KEY>` | Press single key (`enter`, `esc`, `tab`, `space`, `backspace`, etc.) |
| `hotkey` | `<COMBO>` | Press key combination (`ctrl+t`, `alt+tab`, `ctrl+w`, etc.) |
| **Semantic & Desktop OS** | | |
| `find` | `<QUERY>` | Search AT-SPI2 accessibility tree for elements |
| `wait-notification` | `[--timeout MS]` | Intercept incoming desktop notification |
| `wait-download` | `[DIR] [--timeout SEC]` | Watch directory via inotify for completed download |
| `windows` | | List all open application windows |
| `focus` | `<QUERY>` | Bring window to foreground |
| `window-set` | `<Q> <X> <Y> <W> <H>` | Set window position and size |
| `tile` | `<Q> <MODE>` | Tile window (`left`, `right`, `maximize`, `minimize`) |
| `resolution` | | Print current screen dimensions |
| **Batch & Daemon** | | |
| `run` | `"<cmd1>; <cmd2>; ..."` | Execute chained commands in a single session |
| `batch` | `[FILE\|-]` | Interactive daemon mode reading commands from stdin or file |

---

## 💡 Typical Agent Workflows

### 1. Download a File and Wait for It
```bash
# Click download button on web page
interact click 850 420
# Block until the file download finishes completely
interact wait-download ~/Downloads --timeout 30
```

### 2. Fill Form and Click Semantic Submit
```bash
interact run "focus chrome; hotkey ctrl+l; paste https://example.com/login; key enter"
capture wait-settle --settle 200
interact click-element "Username"
interact paste "admin_user"
interact key tab
interact paste "SecurePassword123"
interact click-element "Sign In"
capture wait-change --timeout 5000
```

### 3. Split Screen: Terminal Left, Browser Right
```bash
interact tile code left
interact tile chrome right
```

### 4. Read Copied Code or Text
```bash
interact run "hotkey ctrl+a; hotkey ctrl+c"
COPIED_TEXT=$(interact clipboard)
echo "Extracted: $COPIED_TEXT"
```

### 5. Find Button via Local OCR
```bash
capture /tmp/screen.png
# Get exact coordinates of button labeled "Save Changes"
capture ocr /tmp/screen.png --json
```
