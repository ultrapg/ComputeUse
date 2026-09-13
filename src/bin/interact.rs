use std::collections::HashMap;
use std::env;
use std::ffi::CString;
use std::fs::{self, File};
use std::io::{self, BufRead, BufReader};
use std::os::fd::IntoRawFd;
use std::path::{Path, PathBuf};
use std::process;
use std::thread;
use std::time::{Duration, Instant};
use zbus::blocking::Connection;
use zbus::zvariant::{OwnedObjectPath, OwnedValue};

// Linux input event codes
const EV_SYN: u16 = 0x00;
const EV_KEY: u16 = 0x01;
const EV_REL: u16 = 0x02;
const EV_ABS: u16 = 0x03;

const SYN_REPORT: u16 = 0x00;

const ABS_X: u16 = 0x00;
const ABS_Y: u16 = 0x01;

const REL_X: u16 = 0x00;
const REL_Y: u16 = 0x01;
const REL_HWHEEL: u16 = 0x06;
const REL_WHEEL: u16 = 0x08;

const BTN_LEFT: u16 = 0x110;
const BTN_RIGHT: u16 = 0x111;
const BTN_MIDDLE: u16 = 0x112;

const UI_SET_EVBIT: u64 = 0x40045564;
const UI_SET_KEYBIT: u64 = 0x40045565;
const UI_SET_RELBIT: u64 = 0x40045566;
const UI_SET_ABSBIT: u64 = 0x40045567;
const UI_DEV_SETUP: u64 = 0x405c5503;
const UI_DEV_CREATE: u64 = 0x5501;
const UI_DEV_DESTROY: u64 = 0x5502;
const UI_ABS_SETUP: u64 = 0x401c5504;

#[repr(C)]
struct input_event {
    time: [i64; 2],
    type_: u16,
    code: u16,
    value: i32,
}

#[repr(C)]
struct uinput_abs_setup {
    code: u16,
    pad: [u8; 2],
    value: i32,
    minimum: i32,
    maximum: i32,
    fuzz: i32,
    flat: i32,
    resolution: i32,
}

#[repr(C)]
struct uinput_id {
    bustype: u16,
    vendor: u16,
    product: u16,
    version: u16,
}

#[repr(C)]
struct uinput_setup {
    id: uinput_id,
    name: [u8; 80],
    ff_effects_max: u32,
}

unsafe extern "C" {
    fn write(fd: i32, buf: *const input_event, count: usize) -> isize;
    fn ioctl(fd: i32, request: u64, ...) -> i32;
}

// --- DISPLAY RESOLUTION DETECTION ---

pub fn detect_screen_resolution() -> (u32, u32) {
    if let Ok(output) = std::process::Command::new("kscreen-doctor").arg("-j").output() {
        if let Ok(text) = std::str::from_utf8(&output.stdout) {
            if let Some(pos) = text.find("\"currentSize\"") {
                let slice = &text[pos..];
                if let (Some(w_pos), Some(h_pos)) = (slice.find("\"width\""), slice.find("\"height\"")) {
                    let parse_num = |s: &str| -> Option<u32> {
                        let mut digits = String::new();
                        for ch in s.chars() {
                            if ch.is_ascii_digit() {
                                digits.push(ch);
                            } else if !digits.is_empty() {
                                break;
                            }
                        }
                        digits.parse().ok()
                    };
                    let w = parse_num(&slice[w_pos + 7..]);
                    let h = parse_num(&slice[h_pos + 8..]);
                    if let (Some(w), Some(h)) = (w, h) {
                        if w > 0 && h > 0 {
                            return (w, h);
                        }
                    }
                }
            }
        }
    }

    if let Ok(entries) = std::fs::read_dir("/sys/class/drm") {
        for entry in entries.flatten() {
            let modes_path = entry.path().join("modes");
            if let Ok(content) = std::fs::read_to_string(&modes_path) {
                if let Some(first_line) = content.lines().next() {
                    let parts: Vec<&str> = first_line.trim().split('x').collect();
                    if parts.len() == 2 {
                        if let (Ok(w), Ok(h)) = (parts[0].parse::<u32>(), parts[1].parse::<u32>()) {
                            if w > 0 && h > 0 {
                                return (w, h);
                            }
                        }
                    }
                }
            }
        }
    }

    (1920, 1080)
}

// --- UNIFIED UINPUT DEVICE IMPLEMENTATION ---

pub struct UInputDevice {
    fd: i32,
    pub screen_width: u32,
    pub screen_height: u32,
}

impl UInputDevice {
    pub fn new() -> Result<Self, String> {
        let (screen_width, screen_height) = detect_screen_resolution();
        let file = std::fs::OpenOptions::new()
            .write(true)
            .open("/dev/uinput")
            .map_err(|e| {
                format!(
                    "Failed to open /dev/uinput: {}. Please ensure your user has write permissions (e.g. user in 'input' group: sudo usermod -aG input $USER).",
                    e
                )
            })?;
        let fd = file.into_raw_fd();

        unsafe {
            ioctl(fd, UI_SET_EVBIT, EV_KEY as u64);
            ioctl(fd, UI_SET_KEYBIT, BTN_LEFT as u64);
            ioctl(fd, UI_SET_KEYBIT, BTN_RIGHT as u64);
            ioctl(fd, UI_SET_KEYBIT, BTN_MIDDLE as u64);

            for i in 1..=248 {
                ioctl(fd, UI_SET_KEYBIT, i as u64);
            }

            ioctl(fd, UI_SET_EVBIT, EV_REL as u64);
            ioctl(fd, UI_SET_RELBIT, REL_X as u64);
            ioctl(fd, UI_SET_RELBIT, REL_Y as u64);
            ioctl(fd, UI_SET_RELBIT, REL_WHEEL as u64);
            ioctl(fd, UI_SET_RELBIT, REL_HWHEEL as u64);

            ioctl(fd, UI_SET_EVBIT, EV_ABS as u64);
            ioctl(fd, UI_SET_ABSBIT, ABS_X as u64);
            ioctl(fd, UI_SET_ABSBIT, ABS_Y as u64);

            let abs_x_setup = uinput_abs_setup {
                code: ABS_X,
                pad: [0; 2],
                value: 0,
                minimum: 0,
                maximum: screen_width.saturating_sub(1) as i32,
                fuzz: 0,
                flat: 0,
                resolution: 1,
            };
            ioctl(fd, UI_ABS_SETUP, &abs_x_setup);

            let abs_y_setup = uinput_abs_setup {
                code: ABS_Y,
                pad: [0; 2],
                value: 0,
                minimum: 0,
                maximum: screen_height.saturating_sub(1) as i32,
                fuzz: 0,
                flat: 0,
                resolution: 1,
            };
            ioctl(fd, UI_ABS_SETUP, &abs_y_setup);

            let mut setup = uinput_setup {
                id: uinput_id {
                    bustype: 3,
                    vendor: 0x1234,
                    product: 0x5678,
                    version: 1,
                },
                name: [0; 80],
                ff_effects_max: 0,
            };
            let name_bytes = b"interact-virtual-device\0";
            setup.name[..name_bytes.len()].copy_from_slice(name_bytes);

            ioctl(fd, UI_DEV_SETUP, &setup);
            ioctl(fd, UI_DEV_CREATE);
        }

        thread::sleep(Duration::from_millis(120));

        Ok(Self {
            fd,
            screen_width,
            screen_height,
        })
    }

    fn emit(&self, type_: u16, code: u16, value: i32) {
        let ev = input_event {
            time: [0, 0],
            type_,
            code,
            value,
        };
        unsafe {
            write(self.fd, &ev, std::mem::size_of::<input_event>());
        }
    }

    pub fn move_absolute(&self, x: f64, y: f64) {
        let cx = (x.round() as i32).clamp(0, self.screen_width.saturating_sub(1) as i32);
        let cy = (y.round() as i32).clamp(0, self.screen_height.saturating_sub(1) as i32);
        self.emit(EV_ABS, ABS_X, cx);
        self.emit(EV_ABS, ABS_Y, cy);
        self.emit(EV_SYN, SYN_REPORT, 0);
    }

    pub fn move_rel(&self, dx: f64, dy: f64) {
        self.emit(EV_REL, REL_X, dx as i32);
        self.emit(EV_REL, REL_Y, dy as i32);
        self.emit(EV_SYN, SYN_REPORT, 0);
    }

    pub fn mouse_down(&self, button: u16) {
        self.emit(EV_KEY, button, 1);
        self.emit(EV_SYN, SYN_REPORT, 0);
        thread::sleep(Duration::from_millis(20));
    }

    pub fn mouse_up(&self, button: u16) {
        self.emit(EV_KEY, button, 0);
        self.emit(EV_SYN, SYN_REPORT, 0);
        thread::sleep(Duration::from_millis(20));
    }

    pub fn click(&self, button: u16, double: bool) {
        self.mouse_down(button);
        thread::sleep(Duration::from_millis(40));
        self.mouse_up(button);

        if double {
            thread::sleep(Duration::from_millis(90));
            self.mouse_down(button);
            thread::sleep(Duration::from_millis(40));
            self.mouse_up(button);
        }
        thread::sleep(Duration::from_millis(20));
    }

    pub fn drag(&self, x1: f64, y1: f64, x2: f64, y2: f64, steps: u32) {
        self.move_absolute(x1, y1);
        thread::sleep(Duration::from_millis(40));

        self.mouse_down(BTN_LEFT);
        thread::sleep(Duration::from_millis(40));

        let n = steps.max(8);
        for step in 1..=n {
            let t = step as f64 / n as f64;
            let cx = x1 + (x2 - x1) * t;
            let cy = y1 + (y2 - y1) * t;
            self.move_absolute(cx, cy);
            thread::sleep(Duration::from_millis(15));
        }

        thread::sleep(Duration::from_millis(40));
        self.mouse_up(BTN_LEFT);
        thread::sleep(Duration::from_millis(30));
    }

    pub fn scroll(&self, delta: i32) {
        self.emit(EV_REL, REL_WHEEL, delta);
        self.emit(EV_SYN, SYN_REPORT, 0);
        thread::sleep(Duration::from_millis(30));
    }

    fn char_to_keycode(c: char) -> Option<(u16, bool)> {
        let code = match c {
            'a' | 'A' => 30, 'b' | 'B' => 48, 'c' | 'C' => 46, 'd' | 'D' => 32,
            'e' | 'E' => 18, 'f' | 'F' => 33, 'g' | 'G' => 34, 'h' | 'H' => 35,
            'i' | 'I' => 23, 'j' | 'J' => 36, 'k' | 'K' => 37, 'l' | 'L' => 38,
            'm' | 'M' => 50, 'n' | 'N' => 49, 'o' | 'O' => 24, 'p' | 'P' => 25,
            'q' | 'Q' => 16, 'r' | 'R' => 19, 's' | 'S' => 31, 't' | 'T' => 20,
            'u' | 'U' => 22, 'v' | 'V' => 47, 'w' | 'W' => 17, 'x' | 'X' => 45,
            'y' | 'Y' => 21, 'z' | 'Z' => 44,
            '1' | '!' => 2,  '2' | '@' => 3,  '3' | '#' => 4,  '4' | '$' => 5,
            '5' | '%' => 6,  '6' | '^' => 7,  '7' | '&' => 8,  '8' | '*' => 9,
            '9' | '(' => 10, '0' | ')' => 11,
            '-' | '_' => 12, '=' | '+' => 13,
            '[' | '{' => 26, ']' | '}' => 27,
            ';' | ':' => 39, '\'' | '"' => 40,
            '`' | '~' => 41, '\\' | '|' => 43,
            ',' | '<' => 51, '.' | '>' => 52, '/' | '?' => 53,
            ' ' => 57,
            '\n' => 28,
            _ => return None,
        };
        let shift = c.is_ascii_uppercase() || "!@#$%^&*()_+{}|:\"~<>?".contains(c);
        Some((code, shift))
    }

    pub fn type_text(&self, text: &str) {
        for c in text.chars() {
            if let Some((code, shift)) = Self::char_to_keycode(c) {
                if shift {
                    self.emit(EV_KEY, 42, 1);
                    self.emit(EV_SYN, SYN_REPORT, 0);
                    thread::sleep(Duration::from_millis(5));
                }
                self.emit(EV_KEY, code, 1);
                self.emit(EV_SYN, SYN_REPORT, 0);
                thread::sleep(Duration::from_millis(10));
                self.emit(EV_KEY, code, 0);
                self.emit(EV_SYN, SYN_REPORT, 0);
                if shift {
                    thread::sleep(Duration::from_millis(5));
                    self.emit(EV_KEY, 42, 0);
                    self.emit(EV_SYN, SYN_REPORT, 0);
                }
                thread::sleep(Duration::from_millis(10));
            }
        }
    }

    fn key_name_to_code(name: &str) -> Option<u16> {
        match name.to_lowercase().as_str() {
            "enter" | "return" => Some(28),
            "esc" | "escape" => Some(1),
            "space" => Some(57),
            "backspace" => Some(14),
            "tab" => Some(15),
            "up" => Some(103),
            "left" => Some(105),
            "right" => Some(106),
            "down" => Some(108),
            "shift" => Some(42),
            "rshift" => Some(54),
            "ctrl" | "control" => Some(29),
            "rctrl" => Some(97),
            "alt" => Some(56),
            "altgr" | "ralt" => Some(100),
            "meta" | "super" | "win" => Some(125),
            "delete" | "del" => Some(111),
            "home" => Some(102),
            "end" => Some(107),
            "pageup" | "pgup" => Some(104),
            "pagedown" | "pgdn" => Some(109),
            "f1" => Some(59), "f2" => Some(60), "f3" => Some(61), "f4" => Some(62),
            "f5" => Some(63), "f6" => Some(64), "f7" => Some(65), "f8" => Some(66),
            "f9" => Some(67), "f10" => Some(68), "f11" => Some(87), "f12" => Some(88),
            s if s.len() == 1 => Self::char_to_keycode(s.chars().next().unwrap()).map(|(c, _)| c),
            _ => None,
        }
    }

    pub fn press_key_name(&self, key_name: &str) -> Result<(), String> {
        let code = Self::key_name_to_code(key_name)
            .ok_or_else(|| format!("Unknown key name: '{}'", key_name))?;

        self.emit(EV_KEY, code, 1);
        self.emit(EV_SYN, SYN_REPORT, 0);
        thread::sleep(Duration::from_millis(30));
        self.emit(EV_KEY, code, 0);
        self.emit(EV_SYN, SYN_REPORT, 0);
        thread::sleep(Duration::from_millis(20));

        Ok(())
    }

    pub fn hotkey(&self, combo: &str) -> Result<(), String> {
        let parts: Vec<&str> = combo.split('+').map(|s| s.trim()).collect();
        if parts.is_empty() {
            return Err("Empty hotkey combination".to_string());
        }

        let mut keycodes: Vec<u16> = Vec::new();
        for part in &parts {
            let code = Self::key_name_to_code(part)
                .ok_or_else(|| format!("Unknown key in hotkey: '{}'", part))?;
            keycodes.push(code);
        }

        for &code in &keycodes {
            self.emit(EV_KEY, code, 1);
            self.emit(EV_SYN, SYN_REPORT, 0);
            thread::sleep(Duration::from_millis(15));
        }

        thread::sleep(Duration::from_millis(40));

        for &code in keycodes.iter().rev() {
            self.emit(EV_KEY, code, 0);
            self.emit(EV_SYN, SYN_REPORT, 0);
            thread::sleep(Duration::from_millis(15));
        }

        Ok(())
    }
}

impl Drop for UInputDevice {
    fn drop(&mut self) {
        unsafe {
            ioctl(self.fd, UI_DEV_DESTROY);
            libc::close(self.fd);
        }
    }
}

// --- CLIPBOARD IMPLEMENTATION (READ & WRITE) ---

pub fn get_clipboard() -> Result<String, Box<dyn std::error::Error>> {
    // 1. Primary: KDE Klipper via D-Bus
    if let Ok(conn) = Connection::session() {
        if let Ok(reply) = conn.call_method(
            Some("org.kde.klipper"),
            "/klipper",
            Some("org.kde.klipper.klipper"),
            "getClipboardContents",
            &(),
        ) {
            if let Ok(s) = reply.body().deserialize::<String>() {
                return Ok(s);
            }
        }
    }

    // 2. Fallback: xsel or wl-paste
    if let Ok(out) = process::Command::new("xsel").args(["-b", "-o"]).output() {
        if out.status.success() {
            return Ok(String::from_utf8_lossy(&out.stdout).to_string());
        }
    }
    if let Ok(out) = process::Command::new("wl-paste").arg("--no-newline").output() {
        if out.status.success() {
            return Ok(String::from_utf8_lossy(&out.stdout).to_string());
        }
    }

    Ok(String::new())
}

pub fn set_clipboard(text: &str) {
    if let Ok(conn) = Connection::session() {
        let _ = conn.call_method(
            Some("org.kde.klipper"),
            "/klipper",
            Some("org.kde.klipper.klipper"),
            "setClipboardContents",
            &(text),
        );
    }

    let _ = process::Command::new("xsel")
        .args(["-b", "-i"])
        .stdin(std::process::Stdio::piped())
        .spawn()
        .and_then(|mut child| {
            use std::io::Write;
            if let Some(mut stdin) = child.stdin.take() {
                let _ = stdin.write_all(text.as_bytes());
            }
            child.wait()
        });
}

// --- KWIN WINDOW MANAGEMENT & SCRIPTING ---

type KRunnerMatch = (String, String, String, i32, f64, HashMap<String, OwnedValue>);

fn list_windows() -> Result<Vec<(String, String)>, Box<dyn std::error::Error>> {
    let conn = Connection::session()?;
    let reply = conn.call_method(
        Some("org.kde.KWin"),
        "/WindowsRunner",
        Some("org.kde.krunner1"),
        "Match",
        &(""),
    )?;
    let matches: Vec<KRunnerMatch> = reply.body().deserialize()?;
    let mut wins = Vec::new();
    let mut seen = std::collections::HashSet::new();
    for m in matches {
        if seen.insert(m.0.clone()) {
            wins.push((m.0, m.1));
        }
    }
    Ok(wins)
}

fn focus_window(query: &str) -> Result<bool, Box<dyn std::error::Error>> {
    let conn = Connection::session()?;
    let reply = conn.call_method(
        Some("org.kde.KWin"),
        "/WindowsRunner",
        Some("org.kde.krunner1"),
        "Match",
        &(query),
    );
    let matches: Vec<KRunnerMatch> = match reply {
        Ok(msg) => msg.body().deserialize().unwrap_or_default(),
        Err(_) => Vec::new(),
    };

    if let Some(best) = matches.first() {
        conn.call_method(
            Some("org.kde.KWin"),
            "/WindowsRunner",
            Some("org.kde.krunner1"),
            "Run",
            &(&best.0, ""),
        )?;
        println!("Focused window: \"{}\"", best.1);
        return Ok(true);
    }

    let all_wins = list_windows()?;
    let query_lower = query.to_lowercase();
    if let Some((id, title)) = all_wins.iter().find(|(id, title)| {
        title.to_lowercase().contains(&query_lower) || id.to_lowercase().contains(&query_lower)
    }) {
        conn.call_method(
            Some("org.kde.KWin"),
            "/WindowsRunner",
            Some("org.kde.krunner1"),
            "Run",
            &(id, ""),
        )?;
        println!("Focused window: \"{}\" (matched by title)", title);
        return Ok(true);
    }

    eprintln!("No window matching query '{}' found.", query);
    Ok(false)
}

fn run_kwin_script(js_code: &str) -> Result<(), Box<dyn std::error::Error>> {
    let conn = Connection::session()?;
    let temp_script = env::temp_dir().join(format!("kwin_script_{}.js", process::id()));
    fs::write(&temp_script, js_code)?;

    let reply = conn.call_method(
        Some("org.kde.KWin"),
        "/Scripting",
        Some("org.kde.kwin.Scripting"),
        "loadScript",
        &(temp_script.to_str().unwrap()),
    )?;
    let script_id: i32 = reply.body().deserialize()?;

    let script_path = format!("/Scripting/Script{}", script_id);
    let _ = conn.call_method(
        Some("org.kde.KWin"),
        script_path.as_str(),
        Some("org.kde.kwin.Script"),
        "run",
        &(),
    );
    thread::sleep(Duration::from_millis(30));
    let _ = conn.call_method(
        Some("org.kde.KWin"),
        script_path.as_str(),
        Some("org.kde.kwin.Script"),
        "stop",
        &(),
    );

    let _ = fs::remove_file(&temp_script);
    Ok(())
}

pub fn set_window_geometry(query: &str, x: i32, y: i32, w: i32, h: i32) -> Result<(), Box<dyn std::error::Error>> {
    let script = format!(
        r#"
var wins = workspace.windowList();
for (var i = 0; i < wins.length; i++) {{
    var win = wins[i];
    if (win.normalWindow && (win.caption.toLowerCase().indexOf("{0}".toLowerCase()) !== -1 || (win.resourceClass && win.resourceClass.toLowerCase().indexOf("{0}".toLowerCase()) !== -1))) {{
        win.setMaximize(false, false);
        win.frameGeometry = {{x: {1}, y: {2}, width: {3}, height: {4}}};
        break;
    }}
}}
"#,
        query.replace('"', "\\\""),
        x,
        y,
        w,
        h
    );
    run_kwin_script(&script)?;
    println!("Set window '{}' geometry to ({}, {}) {}x{}", query, x, y, w, h);
    Ok(())
}

pub fn tile_window(query: &str, mode: &str) -> Result<(), Box<dyn std::error::Error>> {
    let (screen_w, screen_h) = detect_screen_resolution();
    match mode.to_lowercase().as_str() {
        "left" => set_window_geometry(query, 0, 0, (screen_w / 2) as i32, screen_h as i32),
        "right" => set_window_geometry(query, (screen_w / 2) as i32, 0, (screen_w / 2) as i32, screen_h as i32),
        "maximize" | "max" => {
            let script = format!(
                r#"
var wins = workspace.windowList();
for (var i = 0; i < wins.length; i++) {{
    var win = wins[i];
    if (win.normalWindow && (win.caption.toLowerCase().indexOf("{0}".toLowerCase()) !== -1 || (win.resourceClass && win.resourceClass.toLowerCase().indexOf("{0}".toLowerCase()) !== -1))) {{
        win.setMaximize(true, true);
        break;
    }}
}}
"#,
                query.replace('"', "\\\"")
            );
            run_kwin_script(&script)?;
            println!("Maximized window '{}'", query);
            Ok(())
        }
        "minimize" | "min" => {
            let script = format!(
                r#"
var wins = workspace.windowList();
for (var i = 0; i < wins.length; i++) {{
    var win = wins[i];
    if (win.normalWindow && (win.caption.toLowerCase().indexOf("{0}".toLowerCase()) !== -1 || (win.resourceClass && win.resourceClass.toLowerCase().indexOf("{0}".toLowerCase()) !== -1))) {{
        win.minimized = true;
        break;
    }}
}}
"#,
                query.replace('"', "\\\"")
            );
            run_kwin_script(&script)?;
            println!("Minimized window '{}'", query);
            Ok(())
        }
        unknown => Err(format!("Unknown tile mode '{}'. Use: left, right, maximize, minimize", unknown).into()),
    }
}

// --- AT-SPI2 ACCESSIBILITY TREE INSPECTION ---

#[derive(Debug, Clone)]
pub struct AccessibleElement {
    pub app: String,
    pub name: String,
    pub role: String,
    pub x: i32,
    pub y: i32,
    pub w: i32,
    pub h: i32,
}

impl AccessibleElement {
    pub fn center(&self) -> (i32, i32) {
        (self.x + self.w / 2, self.y + self.h / 2)
    }
}

fn connect_atspi() -> Result<Connection, Box<dyn std::error::Error>> {
    let session = Connection::session()?;
    let reply = session.call_method(
        Some("org.a11y.Bus"),
        "/org/a11y/bus",
        Some("org.a11y.Bus"),
        "GetAddress",
        &(),
    )?;
    let addr: String = reply.body().deserialize()?;
    let conn = zbus::blocking::connection::Builder::address(addr.as_str())?.build()?;
    Ok(conn)
}

fn search_atspi_recursive(
    conn: &Connection,
    app_name: &str,
    dest: &str,
    path: &str,
    query: &str,
    depth: usize,
    results: &mut Vec<AccessibleElement>,
) {
    if depth > 7 || results.len() >= 20 {
        return;
    }

    let name = if let Ok(reply) = conn.call_method(
        Some(dest),
        path,
        Some("org.freedesktop.DBus.Properties"),
        "Get",
        &("org.a11y.atspi.Accessible", "Name"),
    ) {
        if let Ok(val) = reply.body().deserialize::<OwnedValue>() {
            String::try_from(val).unwrap_or_default()
        } else {
            String::new()
        }
    } else {
        String::new()
    };

    let role = if let Ok(reply) = conn.call_method(
        Some(dest),
        path,
        Some("org.a11y.atspi.Accessible"),
        "GetRoleName",
        &(),
    ) {
        reply.body().deserialize::<String>().unwrap_or_default()
    } else {
        String::new()
    };

    let extents = if let Ok(reply) = conn.call_method(
        Some(dest),
        path,
        Some("org.a11y.atspi.Component"),
        "GetExtents",
        &(0u32),
    ) {
        reply.body().deserialize::<(i32, i32, i32, i32)>().unwrap_or((0, 0, 0, 0))
    } else {
        (0, 0, 0, 0)
    };

    let q_lower = query.to_lowercase();
    if !name.is_empty() && (name.to_lowercase().contains(&q_lower) || role.to_lowercase().contains(&q_lower)) {
        results.push(AccessibleElement {
            app: app_name.to_string(),
            name: name.clone(),
            role: role.clone(),
            x: extents.0,
            y: extents.1,
            w: extents.2,
            h: extents.3,
        });
    }

    if let Ok(reply) = conn.call_method(
        Some(dest),
        path,
        Some("org.a11y.atspi.Accessible"),
        "GetChildren",
        &(),
    ) {
        if let Ok(children) = reply.body().deserialize::<Vec<(String, OwnedObjectPath)>>() {
            for (c_dest, c_path) in children {
                search_atspi_recursive(conn, app_name, &c_dest, c_path.as_str(), query, depth + 1, results);
            }
        }
    }
}

pub fn find_elements(query: &str) -> Result<Vec<AccessibleElement>, Box<dyn std::error::Error>> {
    let conn = connect_atspi()?;
    let reply = conn.call_method(
        Some("org.a11y.atspi.Registry"),
        "/org/a11y/atspi/accessible/root",
        Some("org.a11y.atspi.Accessible"),
        "GetChildren",
        &(),
    )?;
    let apps: Vec<(String, OwnedObjectPath)> = reply.body().deserialize()?;

    let mut results = Vec::new();
    for (dest, path) in apps {
        let app_name = if let Ok(reply) = conn.call_method(
            Some(dest.as_str()),
            path.as_str(),
            Some("org.freedesktop.DBus.Properties"),
            "Get",
            &("org.a11y.atspi.Accessible", "Name"),
        ) {
            if let Ok(val) = reply.body().deserialize::<OwnedValue>() {
                String::try_from(val).unwrap_or_default()
            } else {
                String::new()
            }
        } else {
            String::new()
        };
        search_atspi_recursive(&conn, &app_name, &dest, path.as_str(), query, 0, &mut results);
    }

    Ok(results)
}

// --- DESKTOP NOTIFICATIONS MONITOR ---

pub fn wait_for_notification(timeout_ms: u64) -> Result<(), Box<dyn std::error::Error>> {
    println!("Listening for desktop notifications (timeout: {}ms)...", timeout_ms);
    let mut child = process::Command::new("busctl")
        .args(["--user", "--json=short", "monitor", "org.freedesktop.Notifications"])
        .stdout(process::Stdio::piped())
        .spawn()?;

    let stdout = child.stdout.take().ok_or("Failed to open pipe to busctl")?;
    let reader = BufReader::new(stdout);
    let start = Instant::now();
    let timeout = Duration::from_millis(timeout_ms);

    for line in reader.lines().flatten() {
        if start.elapsed() >= timeout {
            break;
        }
        if line.contains("\"Notify\"") {
            // Simple string extraction of payload items
            let mut summary = String::new();
            let mut body = String::new();
            let mut app = String::new();

            if let Some(data_pos) = line.find("\"data\":[") {
                let slice = &line[data_pos + 8..];
                let tokens: Vec<&str> = slice.split(',').collect();
                if tokens.len() >= 5 {
                    app = tokens[0].trim_matches('"').to_string();
                    summary = tokens[3].trim_matches('"').to_string();
                    body = tokens[4].trim_matches('"').to_string();
                }
            }

            let _ = child.kill();
            println!("[Notification] App: \"{}\" | Summary: \"{}\" | Body: \"{}\"", app, summary, body);
            return Ok(());
        }
    }

    let _ = child.kill();
    Err(format!("Timeout after {}ms: No notifications received", timeout_ms).into())
}

// --- INOTIFY DOWNLOAD WATCHER ---

pub fn wait_for_download(dir: &Path, timeout_sec: u64) -> Result<PathBuf, Box<dyn std::error::Error>> {
    fs::create_dir_all(dir)?;
    let canonical_dir = dir.canonicalize()?;
    println!("Watching '{}' for completed downloads (timeout: {}s)...", canonical_dir.display(), timeout_sec);

    unsafe {
        let inotify_fd = libc::inotify_init1(libc::IN_NONBLOCK | libc::IN_CLOEXEC);
        if inotify_fd < 0 {
            return Err("Failed to initialize inotify".into());
        }

        let c_path = CString::new(canonical_dir.to_str().unwrap())?;
        let wd = libc::inotify_add_watch(inotify_fd, c_path.as_ptr(), libc::IN_CLOSE_WRITE | libc::IN_MOVED_TO);
        if wd < 0 {
            let err = std::io::Error::last_os_error();
            libc::close(inotify_fd);
            return Err(format!("Failed to add watch on directory '{}': {}", canonical_dir.display(), err).into());
        }

        let start = Instant::now();
        let timeout = Duration::from_secs(timeout_sec);
        let mut buffer = [0u8; 4096];

        while start.elapsed() < timeout {
            let bytes_read = libc::read(inotify_fd, buffer.as_mut_ptr() as *mut libc::c_void, buffer.len());
            if bytes_read > 0 {
                let mut offset = 0;
                while offset < bytes_read as usize {
                    let event = &*(buffer.as_ptr().add(offset) as *const libc::inotify_event);
                    let name_len = event.len as usize;
                    if name_len > 0 {
                        let name_bytes = &buffer[offset + std::mem::size_of::<libc::inotify_event>()..offset + std::mem::size_of::<libc::inotify_event>() + name_len];
                        let filename = std::str::from_utf8(name_bytes).unwrap_or("").trim_matches('\0');
                        if !filename.is_empty()
                            && !filename.ends_with(".crdownload")
                            && !filename.ends_with(".part")
                            && !filename.ends_with(".tmp")
                            && !filename.starts_with('.')
                        {
                            libc::close(inotify_fd);
                            let target = canonical_dir.join(filename);
                            println!("Download completed: {}", target.display());
                            return Ok(target);
                        }
                    }
                    offset += std::mem::size_of::<libc::inotify_event>() + event.len as usize;
                }
            }
            thread::sleep(Duration::from_millis(100));
        }

        libc::close(inotify_fd);
    }

    Err(format!("Timeout after {}s with no completed download", timeout_sec).into())
}

// --- EXECUTION CONTEXT & BATCH RUNNER ---

pub struct ExecutionContext {
    pub uinput: UInputDevice,
}

impl ExecutionContext {
    pub fn new() -> Result<Self, Box<dyn std::error::Error>> {
        let uinput = UInputDevice::new()?;
        Ok(Self { uinput })
    }

    pub fn paste_text(&self, text: &str) -> Result<(), Box<dyn std::error::Error>> {
        set_clipboard(text);
        thread::sleep(Duration::from_millis(50));
        self.uinput.hotkey("ctrl+v")?;
        thread::sleep(Duration::from_millis(30));
        Ok(())
    }

    pub fn execute(&mut self, cmd_line: &str) -> Result<(), Box<dyn std::error::Error>> {
        let trimmed = cmd_line.trim();
        if trimmed.is_empty() || trimmed.starts_with('#') {
            return Ok(());
        }

        let tokens = tokenize(trimmed);
        if tokens.is_empty() {
            return Ok(());
        }

        match tokens[0].to_lowercase().as_str() {
            "click" => {
                let mut numbers: Vec<f64> = Vec::new();
                let mut button = BTN_LEFT;
                let mut double = false;
                let mut i = 1;

                while i < tokens.len() {
                    match tokens[i].to_lowercase().as_str() {
                        "right" | "2" => button = BTN_RIGHT,
                        "middle" | "3" => button = BTN_MIDDLE,
                        "left" | "1" => button = BTN_LEFT,
                        "-d" | "--double" | "double" => double = true,
                        "-b" | "--button" => {
                            i += 1;
                            if i < tokens.len() {
                                match tokens[i].to_lowercase().as_str() {
                                    "right" | "2" => button = BTN_RIGHT,
                                    "middle" | "3" => button = BTN_MIDDLE,
                                    _ => button = BTN_LEFT,
                                }
                            }
                        }
                        s => {
                            if let Ok(num) = s.parse::<f64>() {
                                numbers.push(num);
                            }
                        }
                    }
                    i += 1;
                }

                if numbers.len() < 2 {
                    return Err("Usage: click <X> <Y> [left|right|middle] [double]".into());
                }

                let x = numbers[0];
                let y = numbers[1];

                self.uinput.move_absolute(x, y);
                thread::sleep(Duration::from_millis(20));
                self.uinput.click(button, double);
                println!(
                    "click ({}, {}) [Screen: {}x{}]",
                    x, y, self.uinput.screen_width, self.uinput.screen_height
                );
            }
            "click-element" => {
                if tokens.len() < 2 {
                    return Err("Usage: click-element <TEXT_OR_ROLE>".into());
                }
                let query = tokens[1..].join(" ");
                let elements = find_elements(&query)?;
                if let Some(first) = elements.iter().find(|e| e.w > 0 && e.h > 0) {
                    let (cx, cy) = first.center();
                    self.uinput.move_absolute(cx as f64, cy as f64);
                    thread::sleep(Duration::from_millis(20));
                    self.uinput.click(BTN_LEFT, false);
                    println!("click-element [{}] \"{}\" at ({}, {})", first.role, first.name, cx, cy);
                } else if let Some(first) = elements.first() {
                    let (cx, cy) = first.center();
                    self.uinput.move_absolute(cx as f64, cy as f64);
                    thread::sleep(Duration::from_millis(20));
                    self.uinput.click(BTN_LEFT, false);
                    println!("click-element [{}] \"{}\" at ({}, {})", first.role, first.name, cx, cy);
                } else {
                    return Err(format!("No accessible element found matching '{}'", query).into());
                }
            }
            "find" => {
                if tokens.len() < 2 {
                    return Err("Usage: find <QUERY>".into());
                }
                let query = tokens[1..].join(" ");
                let elements = find_elements(&query)?;
                println!("Found {} accessible element(s) matching '{}':", elements.len(), query);
                for el in elements {
                    let (cx, cy) = el.center();
                    println!(
                        "  - [{}] {} \"{}\" at ({}, {}, {}, {}) -> Center: ({}, {})",
                        el.app, el.role, el.name, el.x, el.y, el.w, el.h, cx, cy
                    );
                }
            }
            "clipboard" | "get-clipboard" => {
                let content = get_clipboard()?;
                println!("{}", content);
            }
            "move" => {
                if tokens.len() < 3 {
                    return Err("Usage: move <X> <Y>".into());
                }
                let x: f64 = tokens[1].parse()?;
                let y: f64 = tokens[2].parse()?;
                self.uinput.move_absolute(x, y);
                println!("move ({}, {})", x, y);
            }
            "move-rel" => {
                if tokens.len() < 3 {
                    return Err("Usage: move-rel <DX> <DY>".into());
                }
                let dx: f64 = tokens[1].parse()?;
                let dy: f64 = tokens[2].parse()?;
                self.uinput.move_rel(dx, dy);
                println!("move-rel ({}, {})", dx, dy);
            }
            "drag" => {
                if tokens.len() < 5 {
                    return Err("Usage: drag <X1> <Y1> <X2> <Y2> [steps]".into());
                }
                let x1: f64 = tokens[1].parse()?;
                let y1: f64 = tokens[2].parse()?;
                let x2: f64 = tokens[3].parse()?;
                let y2: f64 = tokens[4].parse()?;
                let steps: u32 = tokens.get(5).and_then(|s| s.parse().ok()).unwrap_or(15);
                self.uinput.drag(x1, y1, x2, y2, steps);
                println!("drag ({}, {}) -> ({}, {})", x1, y1, x2, y2);
            }
            "mouse-down" => {
                let btn = match tokens.get(1).map(|s| s.to_lowercase()).as_deref() {
                    Some("right") | Some("2") => BTN_RIGHT,
                    Some("middle") | Some("3") => BTN_MIDDLE,
                    _ => BTN_LEFT,
                };
                self.uinput.mouse_down(btn);
                println!("mouse-down");
            }
            "mouse-up" => {
                let btn = match tokens.get(1).map(|s| s.to_lowercase()).as_deref() {
                    Some("right") | Some("2") => BTN_RIGHT,
                    Some("middle") | Some("3") => BTN_MIDDLE,
                    _ => BTN_LEFT,
                };
                self.uinput.mouse_up(btn);
                println!("mouse-up");
            }
            "scroll" => {
                if tokens.len() < 2 {
                    return Err("Usage: scroll <DELTA> (positive=up, negative=down)".into());
                }
                let delta: i32 = tokens[1].parse()?;
                self.uinput.scroll(delta);
                println!("scroll {}", delta);
            }
            "type" => {
                if tokens.len() < 2 {
                    return Err("Usage: type <TEXT>".into());
                }
                let text = tokens[1..].join(" ");
                let safe_ascii = text.chars().all(|c| c.is_ascii_alphanumeric() || c == ' ' || c == '\n');
                if !safe_ascii {
                    self.paste_text(&text)?;
                } else {
                    self.uinput.type_text(&text);
                }
                println!("type '{}'", text);
            }
            "paste" => {
                if tokens.len() < 2 {
                    return Err("Usage: paste <TEXT>".into());
                }
                let text = tokens[1..].join(" ");
                self.paste_text(&text)?;
                println!("paste '{}'", text);
            }
            "key" => {
                if tokens.len() < 2 {
                    return Err("Usage: key <KEY>".into());
                }
                self.uinput.press_key_name(&tokens[1])?;
                println!("key '{}'", tokens[1]);
            }
            "hotkey" | "combo" => {
                if tokens.len() < 2 {
                    return Err("Usage: hotkey <KEY1+KEY2+...> (e.g. ctrl+t, alt+tab)".into());
                }
                self.uinput.hotkey(&tokens[1])?;
                println!("hotkey '{}'", tokens[1]);
            }
            "sleep" => {
                let ms: u64 = tokens.get(1).and_then(|s| s.trim_end_matches("ms").parse().ok()).unwrap_or(200);
                thread::sleep(Duration::from_millis(ms));
                println!("sleep {}ms", ms);
            }
            "focus" => {
                if tokens.len() < 2 {
                    return Err("Usage: focus <WINDOW_TITLE_OR_CLASS>".into());
                }
                let query = tokens[1..].join(" ");
                focus_window(&query)?;
            }
            "windows" => {
                let wins = list_windows()?;
                println!("Open Windows ({}):", wins.len());
                for (id, title) in wins {
                    println!("  - [{}] {}", id, title);
                }
            }
            "window-set" => {
                if tokens.len() < 6 {
                    return Err("Usage: window-set <QUERY> <X> <Y> <W> <H>".into());
                }
                let query = &tokens[1];
                let x: i32 = tokens[2].parse()?;
                let y: i32 = tokens[3].parse()?;
                let w: i32 = tokens[4].parse()?;
                let h: i32 = tokens[5].parse()?;
                set_window_geometry(query, x, y, w, h)?;
            }
            "tile" => {
                if tokens.len() < 3 {
                    return Err("Usage: tile <QUERY> <left|right|maximize|minimize>".into());
                }
                tile_window(&tokens[1], &tokens[2])?;
            }
            "wait-download" => {
                let default_dir = env::var("HOME").map(|h| PathBuf::from(h).join("Downloads")).unwrap_or_else(|_| PathBuf::from("."));
                let dir = tokens.get(1).map(PathBuf::from).unwrap_or(default_dir);
                let timeout_sec = tokens.get(2).and_then(|s| s.parse().ok()).unwrap_or(15);
                wait_for_download(&dir, timeout_sec)?;
            }
            "wait-notification" => {
                let timeout_ms = tokens.get(1).and_then(|s| s.parse().ok()).unwrap_or(5000);
                wait_for_notification(timeout_ms)?;
            }
            "resolution" | "res" => {
                println!("Screen Resolution: {}x{}", self.uinput.screen_width, self.uinput.screen_height);
            }
            unknown => return Err(format!("Unknown action '{}' in batch", unknown).into()),
        }

        Ok(())
    }
}

fn tokenize(s: &str) -> Vec<String> {
    let mut tokens = Vec::new();
    let mut current = String::new();
    let mut in_quotes = false;
    let mut quote_char = '"';

    for ch in s.chars() {
        if in_quotes {
            if ch == quote_char {
                in_quotes = false;
            } else {
                current.push(ch);
            }
        } else if ch == '"' || ch == '\'' {
            in_quotes = true;
            quote_char = ch;
        } else if ch.is_whitespace() {
            if !current.is_empty() {
                tokens.push(current);
                current = String::new();
            }
        } else {
            current.push(ch);
        }
    }
    if !current.is_empty() {
        tokens.push(current);
    }
    tokens
}

fn print_help(bin_name: &str) {
    println!("Usage: {} [COMMAND] [OPTIONS]", bin_name);
    println!();
    println!("Advanced Wayland / KDE Plasma desktop automation toolkit for AI agents.");
    println!("Controls mouse, keyboard, clipboard, window layouts, and AT-SPI semantic inspection with zero junk.");
    println!();
    println!("Mouse & Cursor Commands:");
    println!("  click <X> <Y>                   Move cursor to (X, Y) and click (default)");
    println!("  click-element <QUERY>           Find element in AT-SPI tree and click its center");
    println!("  move <X> <Y>                    Move cursor to absolute coordinates (X, Y)");
    println!("  move-rel <DX> <DY>              Move cursor relative to current position by (DX, DY)");
    println!("  drag <X1> <Y1> <X2> <Y2> [N]    Smooth mouse drag from (X1, Y1) to (X2, Y2)");
    println!("  scroll <DELTA>                  Scroll mouse wheel (positive=up, negative=down)");
    println!("  mouse-down [BTN]                Hold mouse button (left, right, middle)");
    println!("  mouse-up [BTN]                  Release mouse button");
    println!();
    println!("Keyboard & Text Commands:");
    println!("  type <TEXT>                     Type out string via raw keycodes");
    println!("  paste <TEXT>                    Paste text via clipboard (layout-independent)");
    println!("  key <KEY>                       Press single key (enter, esc, tab, space)");
    println!("  hotkey <COMBO>                  Press key combination (ctrl+t, alt+tab)");
    println!("  clipboard                       Read and print current clipboard buffer");
    println!();
    println!("Semantic Inspection & Automation:");
    println!("  find <QUERY>                    Search desktop accessibility tree (AT-SPI2) for element");
    println!("  wait-notification [--timeout N] Wait for desktop notification and print App, Summary, Body");
    println!("  wait-download [DIR] [--timeout] Wait for file download to complete via inotify");
    println!();
    println!("Window Layout & Management:");
    println!("  focus <QUERY>                   Activate window matching title or class");
    println!("  windows                         List all open application windows");
    println!("  window-set <Q> <X> <Y> <W> <H>  Set window position and dimensions via KWin");
    println!("  tile <QUERY> <MODE>             Tile window (left, right, maximize, minimize)");
    println!("  resolution                      Print screen resolution");
    println!();
    println!("Batch & Execution Modes:");
    println!("  run \"<CMD1>; <CMD2>; ...\"       Execute multiple commands in a single persistent session");
    println!("  batch [FILE|-]                  Interactive daemon mode reading commands from file/stdin");
}

fn run_batch_reader<R: BufRead>(mut reader: R, ctx: &mut ExecutionContext, is_interactive: bool) -> Result<(), Box<dyn std::error::Error>> {
    let mut line = String::new();
    loop {
        if is_interactive {
            eprint!("interact> ");
        }
        line.clear();
        let bytes = reader.read_line(&mut line)?;
        if bytes == 0 {
            break;
        }
        let trimmed = line.trim();
        if trimmed == "exit" || trimmed == "quit" {
            break;
        }
        for sub_cmd in trimmed.split(';') {
            let cmd = sub_cmd.trim();
            if !cmd.is_empty() {
                if let Err(e) = ctx.execute(cmd) {
                    eprintln!("Error executing '{}': {}", cmd, e);
                }
            }
        }
    }
    Ok(())
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<String> = env::args().collect();
    let bin_name = args.first().map(|s| s.as_str()).unwrap_or("interact");

    if args.len() <= 1 || args[1] == "-h" || args[1] == "--help" {
        print_help(bin_name);
        process::exit(0);
    }

    let cmd = args[1].to_lowercase();

    // Fast-path commands that do not require uinput device creation
    if cmd == "windows" {
        let wins = list_windows()?;
        println!("Open Windows ({}):", wins.len());
        for (id, title) in wins {
            println!("  - [{}] {}", id, title);
        }
        return Ok(());
    }
    if cmd == "focus" {
        if args.len() < 3 {
            eprintln!("Usage: {} focus <QUERY>", bin_name);
            process::exit(1);
        }
        let query = args[2..].join(" ");
        focus_window(&query)?;
        return Ok(());
    }
    if cmd == "clipboard" || cmd == "get-clipboard" {
        let content = get_clipboard()?;
        println!("{}", content);
        return Ok(());
    }
    if cmd == "find" {
        if args.len() < 3 {
            eprintln!("Usage: {} find <QUERY>", bin_name);
            process::exit(1);
        }
        let query = args[2..].join(" ");
        let elements = find_elements(&query)?;
        println!("Found {} accessible element(s) matching '{}':", elements.len(), query);
        for el in elements {
            let (cx, cy) = el.center();
            println!(
                "  - [{}] {} \"{}\" at ({}, {}, {}, {}) -> Center: ({}, {})",
                el.app, el.role, el.name, el.x, el.y, el.w, el.h, cx, cy
            );
        }
        return Ok(());
    }
    if cmd == "window-set" {
        if args.len() < 7 {
            eprintln!("Usage: {} window-set <QUERY> <X> <Y> <W> <H>", bin_name);
            process::exit(1);
        }
        let query = &args[2];
        let x: i32 = args[3].parse()?;
        let y: i32 = args[4].parse()?;
        let w: i32 = args[5].parse()?;
        let h: i32 = args[6].parse()?;
        set_window_geometry(query, x, y, w, h)?;
        return Ok(());
    }
    if cmd == "tile" {
        if args.len() < 4 {
            eprintln!("Usage: {} tile <QUERY> <left|right|maximize|minimize>", bin_name);
            process::exit(1);
        }
        tile_window(&args[2], &args[3])?;
        return Ok(());
    }
    if cmd == "wait-notification" {
        let timeout_ms = args.get(2).and_then(|s| s.parse().ok()).unwrap_or(5000);
        wait_for_notification(timeout_ms)?;
        return Ok(());
    }
    if cmd == "wait-download" {
        let default_dir = env::var("HOME").map(|h| PathBuf::from(h).join("Downloads")).unwrap_or_else(|_| PathBuf::from("."));
        let dir = args.get(2).map(PathBuf::from).unwrap_or(default_dir);
        let timeout_sec = args.get(3).and_then(|s| s.parse().ok()).unwrap_or(15);
        wait_for_download(&dir, timeout_sec)?;
        return Ok(());
    }

    // Direct batch run
    if cmd == "run" {
        if args.len() < 3 {
            eprintln!("Usage: {} run \"<CMD1>; <CMD2>; ...\"", bin_name);
            process::exit(1);
        }
        let script = args[2..].join(" ");
        let mut ctx = ExecutionContext::new()?;
        for sub_cmd in script.split(';') {
            let c = sub_cmd.trim();
            if !c.is_empty() {
                ctx.execute(c)?;
            }
        }
        return Ok(());
    }

    if cmd == "batch" || cmd == "daemon" {
        let mut ctx = ExecutionContext::new()?;
        let target = args.get(2).map(|s| s.as_str()).unwrap_or("-");
        if target == "-" {
            let stdin = io::stdin();
            run_batch_reader(stdin.lock(), &mut ctx, false)?;
        } else {
            let file = File::open(target)?;
            run_batch_reader(BufReader::new(file), &mut ctx, false)?;
        }
        return Ok(());
    }

    // Single command execution via ExecutionContext
    let mut ctx = ExecutionContext::new()?;

    if args[1].chars().next().map_or(false, |c| c.is_ascii_digit())
        || (args[1].starts_with('-')
            && args.iter().skip(1).filter(|a| a.chars().next().map_or(false, |c| c.is_ascii_digit())).count() >= 2)
    {
        let line = format!("click {}", args[1..].join(" "));
        ctx.execute(&line)?;
        return Ok(());
    }

    let full_line = args[1..].join(" ");
    ctx.execute(&full_line)?;

    Ok(())
}
