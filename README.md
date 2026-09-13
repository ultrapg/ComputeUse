# 🖥️ ComputerUse

[![Rust](https://img.shields.io/badge/Rust-2024_Edition-orange?logo=rust)](https://www.rust-lang.org/)
[![Platform](https://img.shields.io/badge/Platform-Linux%20(KDE%20Plasma%20%2F%20Wayland)-blue?logo=kde)](https://kde.org/plasma-desktop/)
[![Wayland](https://img.shields.io/badge/Display%20Server-Wayland%20%2F%20KWin-purple)](https://wayland.freedesktop.org/)
[![Zero Junk](https://img.shields.io/badge/System%20Footprint-Zero%20Junk-green)](https://github.com/ultrapg/computeuse)
[![License: MIT](https://img.shields.io/badge/License-MIT-yellow.svg)](LICENSE)

A high-performance, standalone Rust toolkit for next-generation desktop automation and AI Computer Use on **Linux (KDE Plasma 6 / Wayland)**.

Traditional automation tools (`xdotool`, `scrot`, `pyautogui`) fail under Wayland due to display server security boundaries. **ComputerUse** bypasses these limitations by interfacing directly with native Linux kernel subsystems (`uinput`), XDG Desktop Portals, D-Bus (KWin, AT-SPI2 accessibility, Klipper, Notifications), and kernel `inotify`—with zero Python runtime dependencies, zero background daemons, and zero temporary files left on disk.

---

## 📦 Binaries

The project compiles into two lightweight, standalone executables:

| Binary | Capabilities | Underlying Technology |
| :--- | :--- | :--- |
| **`capture`** | Screenshots, region cropping, coordinate grid overlays, reactive motion diffing, local OCR bounding boxes, and video recording | XDG Desktop Portal (`ashpd`), ffmpeg (`x11grab`), Tesseract OCR |
| **`interact`** | Cursor motion, clicks, drags, mouse wheel scrolling, typing, hotkeys, bidirectional clipboard, AT-SPI2 semantic inspection, window tiling & geometry, notifications, and inotify downloads | Linux Kernel `/dev/uinput` (`EV_ABS`, `EV_REL`, `EV_KEY`), KWin D-Bus, AT-SPI2, libc inotify |

---

## ⚡ Quick Start

### Installation & Build

```bash
git clone https://github.com/ultrapg/computeuse.git
cd computeuse
cargo build --release
```

The optimized binaries will be available in `./target/release/`:
- `./target/release/capture`
- `./target/release/interact`

> **Note:** To allow input simulation without root privileges, ensure your user account has write access to `/dev/uinput` (e.g. membership in the `input` group or a udev rule):
> ```bash
> sudo usermod -aG input $USER
> ```

### Packaging Deployment Bundle

To build optimized release binaries and create a portable tarball with the agent usage guide:

```bash
./deploy.sh
```

Generates `deploy/computeuse-x86_64.tar.gz` containing `capture`, `interact`, and `USAGE.md`.

---

## 📸 `capture` — Screenshot & Vision Toolkit

Captures desktop frames directly through the XDG Desktop Portal without interactive prompt dialogs, immediately cleaning up temporary portal handles from disk.

### Commands & Options

```bash
capture [COMMAND] [OPTIONS] [OUTPUT_FILE]
```

| Command / Option | Arguments | Description |
| :--- | :--- | :--- |
| `[file.png]` | | Default: Capture full desktop at 1080p |
| `-o, --output` | `<FILE>` | Destination file path (default: `screenshot.png`) |
| `-r, --resolution` | `<RES>` | Resolution preset (`1080p`, `720p`, `1440p`, `4k`, `original`, `WxH`) |
| `-g, --grid` | `[SPACING]` | Overlay a coordinate grid with micro-labels every $N$ pixels (default: `100px`) |
| `-c, --region` | `<X,Y,W,H>` | Crop capture directly to bounding box without downscaling |
| `wait-change` | `[-c REGION] [--timeout MS]` | Block until on-screen pixels change (perceptual diff) |
| `wait-settle` | `[-c REGION] [--settle MS]` | Block until on-screen animations finish and pixels stabilize |
| `ocr` | `[IMAGE] [--json]` | Local OCR with bounding box coordinates `[x, y, w, h]` and confidence |
| `record` | `<FILE.mp4> [--duration SEC]` | Record a desktop video snippet via ffmpeg |

### Examples

```bash
# Capture full desktop at default 1080p
capture screen.png

# Capture with a visual coordinate grid (essential for AI vision models)
capture --grid screen_grid.png

# Capture with fine 50px grid
capture --grid 50 screen_fine_grid.png

# Block until visual change occurs
capture wait-change --timeout 5000

# Wait for animations/loading to settle for 200ms
capture wait-settle --settle 200

# Extract text coordinates using local OCR
capture ocr screen.png --json

# Record 5-second MP4 video of the desktop
capture record demo.mp4 --duration 5
```

---

## 🖱️ `interact` — Input, Semantic & OS Automation

Simulates precise mouse movement, clicks, dragging, scrolling, raw keyboard strokes, key combos, layout-independent clipboard pasting, AT-SPI2 accessibility tree inspection, window geometry manipulation, and system event tracking.

### Commands

| Command | Arguments | Description |
| :--- | :--- | :--- |
| **Mouse & Cursor** | | |
| `click` | `<X> <Y> [BTN] [double]` | Move to coordinate and click (`left`, `right`, or `middle`) |
| `click-element` | `<QUERY>` | Find element in AT-SPI2 accessibility tree and click its center |
| `move` | `<X> <Y>` | Move cursor to absolute pixel coordinates |
| `move-rel` | `<DX> <DY>` | Move cursor relative to current position |
| `drag` | `<X1> <Y1> <X2> <Y2> [steps]` | Smooth mouse drag from $(X_1, Y_1)$ to $(X_2, Y_2)$ |
| `scroll` | `<DELTA>` | Scroll vertical mouse wheel (positive = up, negative = down) |
| `mouse-down` / `mouse-up` | `[BTN]` | Press or release mouse button without releasing grab |
| **Keyboard & Clipboard** | | |
| `type` | `<TEXT>` | Type string using raw keyboard keycodes |
| `paste` | `<TEXT>` | Paste text via system clipboard (immune to keyboard layouts like QWERTZ) |
| `clipboard` | | Read and print current system clipboard buffer |
| `key` | `<KEY>` | Press single key (`enter`, `esc`, `tab`, `backspace`, `space`, etc.) |
| `hotkey` | `<COMBO>` | Press key combination (`ctrl+t`, `alt+tab`, `ctrl+w`, etc.) |
| **Semantic & Window Management** | | |
| `find` | `<QUERY>` | Search AT-SPI2 accessibility tree for elements matching query |
| `windows` | | List all open application windows and titles |
| `focus` | `<QUERY>` | Bring window matching title or application class to foreground |
| `window-set` | `<Q> <X> <Y> <W> <H>` | Set window position and dimensions via KWin D-Bus |
| `tile` | `<Q> <MODE>` | Tile window (`left`, `right`, `maximize`, `minimize`) |
| `wait-notification` | `[--timeout MS]` | Intercept incoming desktop notification and print App, Title, Body |
| `wait-download` | `[DIR] [--timeout SEC]` | Watch directory via inotify for completed download |
| `resolution` | | Query active screen resolution |
| **Batch Execution** | | |
| `run` | `"<CMD1>; <CMD2>; ..."` | Execute chained commands sequentially in a single persistent session |
| `batch` | `[FILE\|-]` | Interactive daemon mode reading commands from a file or stdin |

### Examples

```bash
# Click on pixel (500, 300)
interact click 500 300

# Semantic click on a button without needing coordinates
interact click-element "Submit"

# Read clipboard text
interact clipboard

# Split screen: tile code left, chrome right
interact tile code left
interact tile chrome right

# Intercept a notification (e.g. 2FA code or confirmation)
interact wait-notification --timeout 10000

# Wait for a file download to finish in ~/Downloads
interact wait-download ~/Downloads --timeout 30

# Chained batch execution (single persistent session)
interact run "focus chrome; sleep 150; hotkey ctrl+t; sleep 200; paste https://news.ycombinator.com; key enter"
```

---

## 🚀 High-Speed Batch & Daemon Mode

In high-frequency automation, creating and tearing down virtual input devices for every command introduces unnecessary latency (~300ms per action). 

### Single-Line Batch (`run`)
Reuses a single persistent device session to execute an entire multi-step action sequence in milliseconds:

```bash
interact run "focus chrome; sleep 150; hotkey ctrl+t; sleep 200; paste https://news.ycombinator.com; key enter"
```

### Stdin Daemon (`batch -`)
Keeps `interact` continuously running in the background and pipes commands via standard input:

```bash
printf "focus chrome\nsleep 100\nhotkey ctrl+w\n" | interact batch -
```

---

## 🧪 Testing

ComputerUse includes comprehensive native Rust integration test suites located in `tests/`:

```bash
# Run core reliability test suite (22 tests)
cargo test --test reliability

# Run advanced features test suite (9 tests)
cargo test --test advanced_features
```

Both suites execute with 100% pass rates and feature cross-test synchronization (`GUI_LOCK`) for reliable execution.

---

## 🤖 AI Agent Integration

ComputerUse is designed from the ground up for Vision-Language Models (e.g. Gemini, Claude Computer Use, GPT-4o):

1. **Semantic First**: Call `interact find` or `interact click-element` to bypass vision tokens when interacting with standard accessible applications.
2. **Grid Overlay**: Call `capture --grid` to feed the model a screenshot with labelled coordinates. The model reads target coordinates directly from the nearest intersection label.
3. **Local Fast OCR**: Call `capture ocr --json` to get bounding boxes with exact pixel coordinates for buttons and text.
4. **Reactive Completion**: Replace blind sleeps with `capture wait-settle` and `interact wait-download` to minimize latency and eliminate timing flakiness.

---

## 🛠️ Architecture Details

- **KDE Plasma Wayland Compatibility**:
  - Leverages Linux `/dev/uinput` for reliable clicking, relative movement, wheel scrolling, and keyboard keystrokes.
- **D-Bus Native Integrations**:
  - `org.kde.KWin /Scripting`: Direct window geometry manipulation and half-screen tiling.
  - `org.a11y.atspi`: Dynamic AT-SPI2 bus discovery and recursive widget hierarchy inspection.
  - `org.kde.klipper`: Direct clipboard manipulation for layout-independent pasting and reading.
  - `org.freedesktop.Notifications`: System notification interception.
- **Kernel Inotify**:
  - Direct Linux `inotify_init1` monitoring for atomic `IN_CLOSE_WRITE` / `IN_MOVED_TO` file download detection.
- **Zero Artifact Guarantee**:
  - Screenshots are captured in-memory and immediately cleaned up from disk.
  - Generates zero cache files, config directories, or temporary junk.

---

## 📄 License

This project is licensed under the [MIT License](LICENSE).
