use std::env;
use std::fs;
use std::path::PathBuf;
use std::process::Command;
use std::sync::Mutex;
use std::thread;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

// Global lock to prevent concurrent tests from conflicting over mouse/keyboard/screen
static GUI_LOCK: Mutex<()> = Mutex::new(());

fn temp_path(prefix: &str, ext: &str) -> PathBuf {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let filename = if ext.is_empty() {
        format!("{}_{}", prefix, nanos)
    } else {
        format!("{}_{}.{}", prefix, nanos, ext)
    };
    env::temp_dir().join(filename)
}

#[test]
fn test_01_bidirectional_clipboard() {
    let _lock = GUI_LOCK.lock().unwrap();
    let bin = env!("CARGO_BIN_EXE_interact");
    let secret = format!("TestSecret_{}", SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_nanos());

    let paste_status = Command::new(bin)
        .args(["paste", &secret])
        .status()
        .expect("Failed to run interact paste");
    assert!(paste_status.success());

    thread::sleep(Duration::from_millis(100));

    let output = Command::new(bin)
        .arg("clipboard")
        .output()
        .expect("Failed to run interact clipboard");
    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains(&secret), "Clipboard did not contain expected secret. Got: '{}'", stdout);
}

#[test]
fn test_02_atspi_semantic_search() {
    let _lock = GUI_LOCK.lock().unwrap();
    let bin = env!("CARGO_BIN_EXE_interact");

    let output = Command::new(bin)
        .args(["find", ""])
        .output()
        .expect("Failed to run interact find");
    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("accessible element"), "AT-SPI output missing accessible elements: {}", stdout);
}

#[test]
fn test_03_window_geometry_and_tiling() {
    let _lock = GUI_LOCK.lock().unwrap();
    let bin = env!("CARGO_BIN_EXE_interact");

    let win_output = Command::new(bin)
        .arg("windows")
        .output()
        .expect("Failed to run interact windows");
    assert!(win_output.status.success());
    let win_str = String::from_utf8_lossy(&win_output.stdout);

    // Look for any open window title
    let first_window = win_str.lines()
        .find(|line| line.contains("- [") && line.contains("] "))
        .and_then(|line| line.split("] ").nth(1))
        .map(|s| s.trim())
        .filter(|s| !s.is_empty());

    if let Some(title) = first_window {
        let short_query = &title[..title.len().min(15)];

        // Test window-set
        let set_status = Command::new(bin)
            .args(["window-set", short_query, "100", "100", "800", "600"])
            .status()
            .expect("Failed to run window-set");
        assert!(set_status.success());

        // Test tile left
        let tile_left = Command::new(bin)
            .args(["tile", short_query, "left"])
            .status()
            .expect("Failed to run tile left");
        assert!(tile_left.success());

        // Test tile right
        let tile_right = Command::new(bin)
            .args(["tile", short_query, "right"])
            .status()
            .expect("Failed to run tile right");
        assert!(tile_right.success());

        // Test tile maximize
        let tile_max = Command::new(bin)
            .args(["tile", short_query, "maximize"])
            .status()
            .expect("Failed to run tile maximize");
        assert!(tile_max.success());
    }
}

#[test]
fn test_04_desktop_notification_monitor() {
    let _lock = GUI_LOCK.lock().unwrap();
    let bin = env!("CARGO_BIN_EXE_interact");

    let payload = format!("RustTestPayload_{}", SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_nanos());
    let payload_clone = payload.clone();

    // Trigger notification in background thread after small delay
    thread::spawn(move || {
        thread::sleep(Duration::from_millis(300));
        let _ = Command::new("notify-send")
            .args(["RustNotificationTest", &payload_clone])
            .status();
    });

    let output = Command::new(bin)
        .args(["wait-notification", "--timeout", "4000"])
        .output()
        .expect("Failed to run interact wait-notification");
    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("Notification"), "Expected notification output, got: {}", stdout);
}

#[test]
fn test_05_inotify_download_watcher() {
    let _lock = GUI_LOCK.lock().unwrap();
    let bin = env!("CARGO_BIN_EXE_interact");

    let watch_dir = temp_path("rust_dl_test", "");
    fs::create_dir_all(&watch_dir).expect("Failed to create watch dir");
    let watch_dir_clone = watch_dir.clone();

    // Trigger file creation in background thread
    thread::spawn(move || {
        thread::sleep(Duration::from_millis(300));
        let test_file = watch_dir_clone.join("downloaded_report.pdf");
        let _ = fs::write(&test_file, b"test PDF file content");
    });

    let output = Command::new(bin)
        .args(["wait-download", watch_dir.to_str().unwrap(), "--timeout", "4"])
        .output()
        .expect("Failed to run interact wait-download");
    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("Download completed"), "Expected download completed output, got: {}", stdout);
    assert!(stdout.contains("downloaded_report.pdf"));

    let _ = fs::remove_dir_all(watch_dir);
}

#[test]
fn test_06_capture_wait_change() {
    let _lock = GUI_LOCK.lock().unwrap();
    let capture_bin = env!("CARGO_BIN_EXE_capture");
    let interact_bin = env!("CARGO_BIN_EXE_interact");

    // Move mouse initially
    let _ = Command::new(interact_bin).args(["move", "100", "100"]).status();

    // Trigger screen change in background thread
    let interact_clone = interact_bin.to_string();
    thread::spawn(move || {
        thread::sleep(Duration::from_millis(200));
        let _ = Command::new(interact_clone).args(["move", "600", "600"]).status();
    });

    let output = Command::new(capture_bin)
        .args(["wait-change", "--timeout", "2000"])
        .output()
        .expect("Failed to run capture wait-change");
    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("Change detected"), "Expected change detected, got: {}", stdout);
}

#[test]
fn test_07_capture_wait_settle() {
    let _lock = GUI_LOCK.lock().unwrap();
    let bin = env!("CARGO_BIN_EXE_capture");

    let output = Command::new(bin)
        .args(["wait-settle", "--timeout", "2000", "--settle", "150"])
        .output()
        .expect("Failed to run capture wait-settle");
    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.to_lowercase().contains("settled"), "Expected settled message, got: {}", stdout);
}

#[test]
fn test_08_capture_ocr_json() {
    let _lock = GUI_LOCK.lock().unwrap();
    let bin = env!("CARGO_BIN_EXE_capture");
    let screen = temp_path("ocr_screen", "png");

    let cap_status = Command::new(bin)
        .arg(&screen)
        .status()
        .expect("Failed to capture screenshot for OCR");
    assert!(cap_status.success());

    let output = Command::new(bin)
        .args(["ocr", screen.to_str().unwrap(), "--json"])
        .output()
        .expect("Failed to run capture ocr --json");
    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("\"text\""), "OCR JSON output missing 'text' field: {}", stdout);
    assert!(stdout.contains("\"conf\""), "OCR JSON output missing 'conf' field: {}", stdout);

    let _ = fs::remove_file(screen);
}

#[test]
fn test_09_capture_record() {
    let _lock = GUI_LOCK.lock().unwrap();
    let bin = env!("CARGO_BIN_EXE_capture");
    let record_file = temp_path("record_test", "mp4");

    let status = Command::new(bin)
        .args(["record", record_file.to_str().unwrap(), "--duration", "1"])
        .status()
        .expect("Failed to run capture record");
    assert!(status.success());
    assert!(record_file.exists());

    let metadata = fs::metadata(&record_file).expect("Failed to read recording metadata");
    assert!(metadata.len() > 1000, "Recorded file was unexpectedly small: {} bytes", metadata.len());

    let _ = fs::remove_file(record_file);
}
