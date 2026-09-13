use std::env;
use std::fs;
use std::path::PathBuf;
use std::process::Command;
use std::sync::Mutex;
use std::time::{SystemTime, UNIX_EPOCH};

// Global lock to prevent concurrent tests from conflicting over mouse/keyboard/screen
static GUI_LOCK: Mutex<()> = Mutex::new(());

fn temp_file(prefix: &str, ext: &str) -> PathBuf {
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
fn test_01_capture_standard() {
    let _lock = GUI_LOCK.lock().unwrap();
    let bin = env!("CARGO_BIN_EXE_capture");
    let out = temp_file("test_screen", "png");

    let status = Command::new(bin)
        .arg(&out)
        .status()
        .expect("Failed to run capture");
    assert!(status.success());
    assert!(out.exists());
    let metadata = fs::metadata(&out).expect("Failed to read metadata");
    assert!(metadata.len() > 1000);

    let _ = fs::remove_file(out);
}

#[test]
fn test_02_capture_grid() {
    let _lock = GUI_LOCK.lock().unwrap();
    let bin = env!("CARGO_BIN_EXE_capture");
    let out = temp_file("test_grid", "png");

    let status = Command::new(bin)
        .arg("--grid")
        .arg(&out)
        .status()
        .expect("Failed to run capture --grid");
    assert!(status.success());
    assert!(out.exists());
    let metadata = fs::metadata(&out).expect("Failed to read metadata");
    assert!(metadata.len() > 1000);

    let _ = fs::remove_file(out);
}

#[test]
fn test_03_capture_fine_grid() {
    let _lock = GUI_LOCK.lock().unwrap();
    let bin = env!("CARGO_BIN_EXE_capture");
    let out = temp_file("test_fine_grid", "png");

    let status = Command::new(bin)
        .args(["--grid", "50"])
        .arg(&out)
        .status()
        .expect("Failed to run capture --grid 50");
    assert!(status.success());
    assert!(out.exists());
    let metadata = fs::metadata(&out).expect("Failed to read metadata");
    assert!(metadata.len() > 1000);

    let _ = fs::remove_file(out);
}

#[test]
fn test_04_capture_region() {
    let _lock = GUI_LOCK.lock().unwrap();
    let bin = env!("CARGO_BIN_EXE_capture");
    let out = temp_file("test_region", "png");

    let status = Command::new(bin)
        .args(["--region", "100,100,400,200"])
        .arg(&out)
        .status()
        .expect("Failed to run capture --region");
    assert!(status.success());
    assert!(out.exists());
    let metadata = fs::metadata(&out).expect("Failed to read metadata");
    assert!(metadata.len() > 500);

    let _ = fs::remove_file(out);
}

#[test]
fn test_05_capture_no_extension() {
    let _lock = GUI_LOCK.lock().unwrap();
    let bin = env!("CARGO_BIN_EXE_capture");
    let out = temp_file("test_no_ext", "");

    let status = Command::new(bin)
        .arg(&out)
        .status()
        .expect("Failed to run capture without extension");
    assert!(status.success());
    assert!(out.exists());

    // Verify PNG magic header: 89 50 4E 47 0D 0A 1A 0A
    let bytes = fs::read(&out).expect("Failed to read output file");
    assert!(bytes.len() > 8);
    assert_eq!(&bytes[0..8], &[0x89, 0x50, 0x4E, 0x47, 0x0D, 0x0A, 0x1A, 0x0A]);

    let _ = fs::remove_file(out);
}

#[test]
fn test_06_capture_out_of_bounds_safety() {
    let _lock = GUI_LOCK.lock().unwrap();
    let bin = env!("CARGO_BIN_EXE_capture");
    let out = temp_file("test_oob", "png");

    let status = Command::new(bin)
        .args(["--region", "50000,50000,200,200"])
        .arg(&out)
        .status()
        .expect("Failed to run capture with OOB region");
    assert!(status.success());
    assert!(out.exists());

    let _ = fs::remove_file(out);
}

#[test]
fn test_07_interact_resolution() {
    let _lock = GUI_LOCK.lock().unwrap();
    let bin = env!("CARGO_BIN_EXE_interact");

    let output = Command::new(bin)
        .arg("resolution")
        .output()
        .expect("Failed to run interact resolution");
    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("Screen Resolution:"));
}

#[test]
fn test_08_interact_windows() {
    let _lock = GUI_LOCK.lock().unwrap();
    let bin = env!("CARGO_BIN_EXE_interact");

    let output = Command::new(bin)
        .arg("windows")
        .output()
        .expect("Failed to run interact windows");
    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("Open Windows"));
}

#[test]
fn test_09_interact_focus() {
    let _lock = GUI_LOCK.lock().unwrap();
    let bin = env!("CARGO_BIN_EXE_interact");

    let status = Command::new(bin)
        .args(["focus", "code"])
        .status()
        .expect("Failed to run interact focus");
    // Focus may succeed or report no window found, but must not crash
    assert!(status.success());
}

#[test]
fn test_10_interact_move() {
    let _lock = GUI_LOCK.lock().unwrap();
    let bin = env!("CARGO_BIN_EXE_interact");

    let output = Command::new(bin)
        .args(["move", "500", "500"])
        .output()
        .expect("Failed to run interact move");
    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("move (500, 500)"));
}

#[test]
fn test_11_interact_click() {
    let _lock = GUI_LOCK.lock().unwrap();
    let bin = env!("CARGO_BIN_EXE_interact");

    let output = Command::new(bin)
        .args(["click", "600", "600"])
        .output()
        .expect("Failed to run interact click");
    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("click (600, 600)"));
}

#[test]
fn test_12_interact_click_with_leading_flag() {
    let _lock = GUI_LOCK.lock().unwrap();
    let bin = env!("CARGO_BIN_EXE_interact");

    let output = Command::new(bin)
        .args(["click", "-b", "right", "650", "650"])
        .output()
        .expect("Failed to run interact click with leading flag");
    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("click (650, 650)"));
}

#[test]
fn test_13_interact_click_double() {
    let _lock = GUI_LOCK.lock().unwrap();
    let bin = env!("CARGO_BIN_EXE_interact");

    let output = Command::new(bin)
        .args(["click", "700", "700", "--double"])
        .output()
        .expect("Failed to run interact click --double");
    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("click (700, 700)"));
}

#[test]
fn test_14_interact_shorthand_coordinates() {
    let _lock = GUI_LOCK.lock().unwrap();
    let bin = env!("CARGO_BIN_EXE_interact");

    let output = Command::new(bin)
        .args(["750", "750"])
        .output()
        .expect("Failed to run interact shorthand coordinates");
    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("click (750, 750)"));
}

#[test]
fn test_15_interact_move_rel() {
    let _lock = GUI_LOCK.lock().unwrap();
    let bin = env!("CARGO_BIN_EXE_interact");

    let output = Command::new(bin)
        .args(["move-rel", "15", "-15"])
        .output()
        .expect("Failed to run interact move-rel");
    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("move-rel (15, -15)"));
}

#[test]
fn test_16_interact_scroll() {
    let _lock = GUI_LOCK.lock().unwrap();
    let bin = env!("CARGO_BIN_EXE_interact");

    let output = Command::new(bin)
        .args(["scroll", "-5"])
        .output()
        .expect("Failed to run interact scroll");
    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("scroll -5"));
}

#[test]
fn test_17_interact_drag() {
    let _lock = GUI_LOCK.lock().unwrap();
    let bin = env!("CARGO_BIN_EXE_interact");

    let output = Command::new(bin)
        .args(["drag", "400", "400", "600", "400", "10"])
        .output()
        .expect("Failed to run interact drag");
    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("drag (400, 400) -> (600, 400)"));
}

#[test]
fn test_18_interact_type_ascii() {
    let _lock = GUI_LOCK.lock().unwrap();
    let bin = env!("CARGO_BIN_EXE_interact");

    let output = Command::new(bin)
        .args(["type", "test"])
        .output()
        .expect("Failed to run interact type");
    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("type 'test'"));
}

#[test]
fn test_19_interact_type_unicode_fallback() {
    let _lock = GUI_LOCK.lock().unwrap();
    let bin = env!("CARGO_BIN_EXE_interact");

    let output = Command::new(bin)
        .args(["type", "Grüße €100 / Special: https://test.com"])
        .output()
        .expect("Failed to run interact type unicode");
    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("type 'Grüße €100 / Special: https://test.com'"));
}

#[test]
fn test_20_interact_paste() {
    let _lock = GUI_LOCK.lock().unwrap();
    let bin = env!("CARGO_BIN_EXE_interact");

    let output = Command::new(bin)
        .args(["paste", "https://news.ycombinator.com/item?id=12345"])
        .output()
        .expect("Failed to run interact paste");
    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("paste 'https://news.ycombinator.com/item?id=12345'"));
}

#[test]
fn test_21_interact_key_and_hotkey() {
    let _lock = GUI_LOCK.lock().unwrap();
    let bin = env!("CARGO_BIN_EXE_interact");

    let output = Command::new(bin)
        .args(["key", "esc"])
        .output()
        .expect("Failed to run interact key");
    assert!(output.status.success());

    let output = Command::new(bin)
        .args(["hotkey", "alt+tab"])
        .output()
        .expect("Failed to run interact hotkey");
    assert!(output.status.success());

    // Switch back
    let _ = Command::new(bin).args(["hotkey", "alt+tab"]).status();
}

#[test]
fn test_22_interact_run_chained_batch() {
    let _lock = GUI_LOCK.lock().unwrap();
    let bin = env!("CARGO_BIN_EXE_interact");

    let output = Command::new(bin)
        .args(["run", "move 500 500; sleep 50; click 500 500; scroll -2"])
        .output()
        .expect("Failed to run interact run chained batch");
    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("move (500, 500)"));
    assert!(stdout.contains("sleep 50ms"));
    assert!(stdout.contains("click (500, 500)"));
    assert!(stdout.contains("scroll -2"));
}
