use ashpd::desktop::screenshot::Screenshot;
use image::imageops::FilterType;
use image::GenericImageView;
use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use std::process;
use std::str::FromStr;
use std::time::{Duration, Instant};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ResolutionMode {
    Fit(u32, u32),
    Exact(u32, u32),
    Original,
}

impl FromStr for ResolutionMode {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let normalized = s.trim().to_lowercase();
        match normalized.as_str() {
            "1080p" | "1080" | "fhd" => Ok(ResolutionMode::Fit(1920, 1080)),
            "720p" | "720" | "hd" => Ok(ResolutionMode::Fit(1280, 720)),
            "1440p" | "1440" | "2k" | "qhd" => Ok(ResolutionMode::Fit(2560, 1440)),
            "2160p" | "2160" | "4k" | "uhd" => Ok(ResolutionMode::Fit(3840, 2160)),
            "original" | "native" | "none" | "raw" => Ok(ResolutionMode::Original),
            custom => {
                if let Some((w_str, h_str)) = custom.split_once('x') {
                    let w: u32 = w_str
                        .trim()
                        .parse()
                        .map_err(|_| format!("Invalid width in resolution '{}'", s))?;
                    let h: u32 = h_str
                        .trim()
                        .parse()
                        .map_err(|_| format!("Invalid height in resolution '{}'", s))?;
                    if w == 0 || h == 0 {
                        return Err("Resolution dimensions must be greater than 0".to_string());
                    }
                    Ok(ResolutionMode::Exact(w, h))
                } else {
                    Err(format!(
                        "Unrecognized resolution '{}'. Supported: 1080p, 720p, 1440p, 4k, WIDTHxHEIGHT, original",
                        s
                    ))
                }
            }
        }
    }
}

const FONT_3X5: [[u8; 5]; 10] = [
    [0b111, 0b101, 0b101, 0b101, 0b111], // 0
    [0b010, 0b110, 0b010, 0b010, 0b111], // 1
    [0b111, 0b001, 0b111, 0b100, 0b111], // 2
    [0b111, 0b001, 0b111, 0b001, 0b111], // 3
    [0b101, 0b101, 0b111, 0b001, 0b001], // 4
    [0b111, 0b100, 0b111, 0b001, 0b111], // 5
    [0b111, 0b100, 0b111, 0b101, 0b111], // 6
    [0b111, 0b001, 0b010, 0b010, 0b010], // 7
    [0b111, 0b101, 0b111, 0b101, 0b111], // 8
    [0b111, 0b101, 0b111, 0b001, 0b111], // 9
];

fn blend_pixel(dest: &mut image::Rgba<u8>, src: image::Rgba<u8>) {
    let a = src[3] as u32;
    let inv_a = 255 - a;
    dest[0] = ((src[0] as u32 * a + dest[0] as u32 * inv_a) / 255) as u8;
    dest[1] = ((src[1] as u32 * a + dest[1] as u32 * inv_a) / 255) as u8;
    dest[2] = ((src[2] as u32 * a + dest[2] as u32 * inv_a) / 255) as u8;
    dest[3] = 255;
}

fn draw_text(img: &mut image::RgbaImage, start_x: u32, start_y: u32, text: &str, fg: image::Rgba<u8>, bg: image::Rgba<u8>) {
    let (w, h) = img.dimensions();
    let text_len = text.len() as u32;
    let box_w = text_len * 4 + 2;
    let box_h = 7;

    if start_x + box_w >= w || start_y + box_h >= h {
        return;
    }

    for px in start_x..start_x + box_w {
        for py in start_y..start_y + box_h {
            img.put_pixel(px, py, bg);
        }
    }

    let mut cur_x = start_x + 1;
    for ch in text.chars() {
        if ch.is_ascii_digit() {
            let digit = ch.to_digit(10).unwrap() as usize;
            let glyph = FONT_3X5[digit];
            for row in 0..5 {
                let bits = glyph[row];
                for col in 0..3 {
                    if (bits & (1 << (2 - col))) != 0 {
                        img.put_pixel(cur_x + col, start_y + 1 + row as u32, fg);
                    }
                }
            }
        } else if ch == ',' {
            img.put_pixel(cur_x + 1, start_y + 4, fg);
            img.put_pixel(cur_x, start_y + 5, fg);
        }
        cur_x += 4;
    }
}

fn draw_grid(img: &mut image::RgbaImage, spacing: u32) {
    let (width, height) = img.dimensions();
    let grid_color = image::Rgba([255, 60, 60, 160]);
    let label_bg = image::Rgba([0, 0, 0, 220]);
    let label_fg = image::Rgba([255, 255, 0, 255]);

    for x in (spacing..width).step_by(spacing as usize) {
        for y in 0..height {
            if (y / 4) % 2 == 0 {
                let p = img.get_pixel_mut(x, y);
                blend_pixel(p, grid_color);
            }
        }
    }

    for y in (spacing..height).step_by(spacing as usize) {
        for x in 0..width {
            if (x / 4) % 2 == 0 {
                let p = img.get_pixel_mut(x, y);
                blend_pixel(p, grid_color);
            }
        }
    }

    for x in (spacing..width).step_by(spacing as usize) {
        for y in (spacing..height).step_by(spacing as usize) {
            let label = format!("{},{}", x, y);
            let box_w = (label.len() as u32) * 4 + 2;
            let box_h = 7;
            let start_x = if x + 2 + box_w < width {
                x + 2
            } else {
                x.saturating_sub(box_w + 2)
            };
            let start_y = if y + 2 + box_h < height {
                y + 2
            } else {
                y.saturating_sub(box_h + 2)
            };
            draw_text(img, start_x, start_y, &label, label_fg, label_bg);
        }
    }
}

async fn capture_screenshot() -> Result<image::DynamicImage, Box<dyn std::error::Error>> {
    // Primary method: XDG Desktop Portal via ashpd
    let portal_attempt = async {
        let response = Screenshot::request()
            .interactive(false)
            .send()
            .await?
            .response()?;

        let uri = response.uri();
        let portal_path = uri
            .to_file_path()
            .map_err(|_| format!("Portal returned non-file URI: {}", uri))?;

        let img = image::open(&portal_path)?;
        let _ = fs::remove_file(&portal_path);
        Ok::<_, Box<dyn std::error::Error>>(img)
    }
    .await;

    if let Ok(img) = portal_attempt {
        return Ok(img);
    }

    // Fallback: ffmpeg single frame capture via x11grab
    let fallback_file = env::temp_dir().join(format!("capture_fallback_{}.png", process::id()));
    let status = process::Command::new("ffmpeg")
        .args([
            "-y",
            "-f", "x11grab",
            "-video_size", "1920x1080",
            "-i", ":0",
            "-vframes", "1",
        ])
        .arg(&fallback_file)
        .status();

    if let Ok(s) = status {
        if s.success() && fallback_file.exists() {
            let img = image::open(&fallback_file)?;
            let _ = fs::remove_file(&fallback_file);
            return Ok(img);
        }
    }
    let _ = fs::remove_file(&fallback_file);

    Err("Failed to capture screenshot via both XDG Desktop Portal and ffmpeg fallback.".into())
}

fn crop_if_needed(img: image::DynamicImage, region: Option<(u32, u32, u32, u32)>) -> image::DynamicImage {
    if let Some((rx, ry, rw, rh)) = region {
        let (img_w, img_h) = img.dimensions();
        if rx >= img_w || ry >= img_h {
            img
        } else {
            let crop_x = rx;
            let crop_y = ry;
            let crop_w = rw.min(img_w.saturating_sub(crop_x)).max(1);
            let crop_h = rh.min(img_h.saturating_sub(crop_y)).max(1);
            img.crop_imm(crop_x, crop_y, crop_w, crop_h)
        }
    } else {
        img
    }
}

fn images_differ(img1: &image::DynamicImage, img2: &image::DynamicImage, threshold_ratio: f64) -> bool {
    let (w1, h1) = img1.dimensions();
    let (w2, h2) = img2.dimensions();
    if w1 != w2 || h1 != h2 {
        return true;
    }
    let total_pixels = (w1 * h1) as f64;
    let rgba1 = img1.to_rgba8();
    let rgba2 = img2.to_rgba8();
    let mut diff_count = 0u64;

    for (p1, p2) in rgba1.pixels().step_by(4).zip(rgba2.pixels().step_by(4)) {
        let dr = (p1[0] as i32 - p2[0] as i32).abs();
        let dg = (p1[1] as i32 - p2[1] as i32).abs();
        let db = (p1[2] as i32 - p2[2] as i32).abs();
        if dr > 15 || dg > 15 || db > 15 {
            diff_count += 1;
        }
    }
    let sampled_pixels = (total_pixels / 4.0).max(1.0);
    (diff_count as f64 / sampled_pixels) > threshold_ratio
}

fn parse_region(s: &str) -> Result<(u32, u32, u32, u32), String> {
    let parts: Vec<&str> = s.split(',').map(|p| p.trim()).collect();
    if parts.len() != 4 {
        return Err(format!("Invalid region format '{}'. Expected X,Y,W,H", s));
    }
    let x = parts[0].parse().map_err(|_| format!("Invalid X in region '{}'", s))?;
    let y = parts[1].parse().map_err(|_| format!("Invalid Y in region '{}'", s))?;
    let w = parts[2].parse().map_err(|_| format!("Invalid W in region '{}'", s))?;
    let h = parts[3].parse().map_err(|_| format!("Invalid H in region '{}'", s))?;
    if w == 0 || h == 0 {
        return Err("Region width and height must be greater than 0".to_string());
    }
    Ok((x, y, w, h))
}

fn serde_json_escape(s: &str) -> String {
    let mut out = String::from("\"");
    for c in s.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            ch => out.push(ch),
        }
    }
    out.push('"');
    out
}

fn run_ocr(image_path: &Path, as_json: bool) -> Result<(), Box<dyn std::error::Error>> {
    let output = process::Command::new("tesseract")
        .arg(image_path)
        .arg("stdout")
        .arg("tsv")
        .output()?;

    if !output.status.success() {
        return Err(format!("Tesseract failed: {}", String::from_utf8_lossy(&output.stderr)).into());
    }

    let text = String::from_utf8(output.stdout)?;
    struct OcrWord {
        x: i32,
        y: i32,
        w: i32,
        h: i32,
        conf: i32,
        text: String,
    }
    let mut words: Vec<OcrWord> = Vec::new();

    for (idx, line) in text.lines().enumerate() {
        if idx == 0 {
            continue;
        }
        let cols: Vec<&str> = line.split('\t').collect();
        if cols.len() >= 12 {
            let level: i32 = cols[0].parse().unwrap_or(0);
            if level == 5 {
                let left: i32 = cols[6].parse().unwrap_or(0);
                let top: i32 = cols[7].parse().unwrap_or(0);
                let width: i32 = cols[8].parse().unwrap_or(0);
                let height: i32 = cols[9].parse().unwrap_or(0);
                let conf: f64 = cols[10].parse().unwrap_or(-1.0);
                let word_text = cols[11].trim().to_string();
                if conf >= 30.0 && !word_text.is_empty() {
                    words.push(OcrWord {
                        x: left,
                        y: top,
                        w: width,
                        h: height,
                        conf: conf as i32,
                        text: word_text,
                    });
                }
            }
        }
    }

    if as_json {
        let mut json_items = Vec::new();
        for w in &words {
            json_items.push(format!(
                "{{\"x\":{},\"y\":{},\"w\":{},\"h\":{},\"conf\":{},\"text\":{}}}",
                w.x, w.y, w.w, w.h, w.conf, serde_json_escape(&w.text)
            ));
        }
        println!("[{}]", json_items.join(","));
    } else {
        println!("Detected {} text element(s):", words.len());
        for w in &words {
            println!("[{}, {}, {}, {}] ({}%) \"{}\"", w.x, w.y, w.w, w.h, w.conf, w.text);
        }
    }

    Ok(())
}

fn print_help(bin_name: &str) {
    println!("Usage: {} [COMMAND] [OPTIONS] [OUTPUT_FILE]", bin_name);
    println!();
    println!("Advanced screenshot & vision perception toolkit for Linux (KDE Plasma 6 / Wayland).");
    println!("Zero temporary files left on disk, zero junk, native portal & ffmpeg backends.");
    println!();
    println!("Commands:");
    println!("  [file.png]                          Capture screenshot (default)");
    println!("  wait-change [OPTIONS]               Block until on-screen pixels change");
    println!("  wait-settle [OPTIONS]               Block until pixels change and then settle (stop animating)");
    println!("  ocr [IMAGE_FILE] [--json]           Extract on-screen text with bounding boxes using local Tesseract");
    println!("  record <FILE.mp4> [--duration <N>]  Record desktop video snippet via ffmpeg");
    println!();
    println!("Options for capture:");
    println!("  -o, --output <FILE>                 Specify output file path (default: screenshot.png)");
    println!("  -r, --resolution <RES>              Target resolution: 1080p, 720p, 1440p, 4k, original, WxH");
    println!("  -g, --grid [SPACING]                Overlay visual coordinate grid (default: 100px)");
    println!("  -c, --region <X,Y,W,H>              Crop screenshot to bounding box");
    println!("  --ocr                               Capture and run OCR immediately on the screenshot");
    println!();
    println!("Options for wait-change / wait-settle:");
    println!("  -c, --region <X,Y,W,H>              Watch only specific bounding box region");
    println!("  --timeout <MS>                      Maximum wait timeout in ms (default: 5000ms)");
    println!("  --settle <MS>                       Settle duration with no motion in ms (default: 200ms)");
    println!("  --threshold <PCT>                   Sensitivity ratio (default: 0.001 = 0.1% pixels)");
    println!();
    println!("Options for record:");
    println!("  -d, --duration <SEC>                Duration to record in seconds (default: 5s)");
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<String> = env::args().collect();
    let bin_name = args.first().map(|s| s.as_str()).unwrap_or("capture");

    if args.len() > 1 && (args[1] == "-h" || args[1] == "--help") {
        print_help(bin_name);
        process::exit(0);
    }

    let cmd = args.get(1).map(|s| s.as_str()).unwrap_or("");

    // --- SUBCOMMAND: wait-change ---
    if cmd == "wait-change" {
        let mut region = None;
        let mut timeout_ms = 5000u64;
        let mut threshold = 0.001f64;
        let mut i = 2;
        while i < args.len() {
            match args[i].as_str() {
                "-c" | "--region" => {
                    i += 1;
                    if i < args.len() { region = Some(parse_region(&args[i])?); }
                }
                arg if arg.starts_with("--region=") => {
                    region = Some(parse_region(arg.strip_prefix("--region=").unwrap())?);
                }
                "--timeout" => {
                    i += 1;
                    if i < args.len() { timeout_ms = args[i].parse().unwrap_or(5000); }
                }
                "--threshold" => {
                    i += 1;
                    if i < args.len() { threshold = args[i].parse().unwrap_or(0.001); }
                }
                _ => {}
            }
            i += 1;
        }

        let start = Instant::now();
        let timeout = Duration::from_millis(timeout_ms);
        let initial_img = crop_if_needed(capture_screenshot().await?, region);

        println!("Waiting for screen change (timeout: {}ms)...", timeout_ms);
        while start.elapsed() < timeout {
            tokio::time::sleep(Duration::from_millis(60)).await;
            let current_img = crop_if_needed(capture_screenshot().await?, region);
            if images_differ(&initial_img, &current_img, threshold) {
                println!("Change detected after {}ms", start.elapsed().as_millis());
                return Ok(());
            }
        }
        eprintln!("Timeout after {}ms with no change detected", timeout_ms);
        process::exit(1);
    }

    // --- SUBCOMMAND: wait-settle ---
    if cmd == "wait-settle" {
        let mut region = None;
        let mut timeout_ms = 5000u64;
        let mut settle_ms = 200u64;
        let mut threshold = 0.001f64;
        let mut i = 2;
        while i < args.len() {
            match args[i].as_str() {
                "-c" | "--region" => {
                    i += 1;
                    if i < args.len() { region = Some(parse_region(&args[i])?); }
                }
                "--timeout" => {
                    i += 1;
                    if i < args.len() { timeout_ms = args[i].parse().unwrap_or(5000); }
                }
                "--settle" => {
                    i += 1;
                    if i < args.len() { settle_ms = args[i].parse().unwrap_or(200); }
                }
                "--threshold" => {
                    i += 1;
                    if i < args.len() { threshold = args[i].parse().unwrap_or(0.001); }
                }
                _ => {}
            }
            i += 1;
        }

        let start = Instant::now();
        let timeout = Duration::from_millis(timeout_ms);
        let settle_duration = Duration::from_millis(settle_ms);

        let mut prev_img = crop_if_needed(capture_screenshot().await?, region);
        let mut change_observed = false;
        let mut last_change = Instant::now();

        println!("Waiting for screen to settle for {}ms (timeout: {}ms)...", settle_ms, timeout_ms);
        while start.elapsed() < timeout {
            tokio::time::sleep(Duration::from_millis(50)).await;
            let current_img = crop_if_needed(capture_screenshot().await?, region);

            if images_differ(&prev_img, &current_img, threshold) {
                change_observed = true;
                last_change = Instant::now();
                prev_img = current_img;
            } else if last_change.elapsed() >= settle_duration {
                if change_observed {
                    println!("Screen changed and settled after {}ms", start.elapsed().as_millis());
                } else {
                    println!("Screen settled (stable for {}ms)", last_change.elapsed().as_millis());
                }
                return Ok(());
            }
        }

        println!("Screen settled at timeout ({}ms)", timeout_ms);
        return Ok(());
    }

    // --- SUBCOMMAND: ocr ---
    if cmd == "ocr" {
        let mut target_image: Option<PathBuf> = None;
        let mut as_json = false;
        for arg in args.iter().skip(2) {
            if arg == "--json" {
                as_json = true;
            } else if !arg.starts_with('-') {
                target_image = Some(PathBuf::from(arg));
            }
        }

        let temp_screenshot;
        let img_path = match target_image {
            Some(p) => p,
            None => {
                let img = capture_screenshot().await?;
                temp_screenshot = env::temp_dir().join(format!("capture_ocr_{}.png", process::id()));
                img.save_with_format(&temp_screenshot, image::ImageFormat::Png)?;
                temp_screenshot
            }
        };

        let res = run_ocr(&img_path, as_json);
        if img_path.to_string_lossy().contains("capture_ocr_") {
            let _ = fs::remove_file(&img_path);
        }
        return res;
    }

    // --- SUBCOMMAND: record ---
    if cmd == "record" {
        let mut out_file = PathBuf::from("recording.mp4");
        let mut duration = 5u32;
        let mut i = 2;
        while i < args.len() {
            match args[i].as_str() {
                "-d" | "--duration" => {
                    i += 1;
                    if i < args.len() { duration = args[i].parse().unwrap_or(5); }
                }
                pos if !pos.starts_with('-') => {
                    out_file = PathBuf::from(pos);
                }
                _ => {}
            }
            i += 1;
        }

        println!("Recording desktop for {}s to {}...", duration, out_file.display());
        let status = process::Command::new("ffmpeg")
            .args([
                "-y",
                "-f", "x11grab",
                "-video_size", "1920x1080",
                "-i", ":0",
                "-t", &duration.to_string(),
                "-c:v", "libx264",
                "-preset", "ultrafast",
            ])
            .arg(&out_file)
            .status()?;

        if !status.success() {
            return Err("ffmpeg screen recording failed".into());
        }
        println!("Recording saved successfully to: {}", out_file.display());
        return Ok(());
    }

    // --- STANDARD CAPTURE MODE ---
    let mut output: Option<PathBuf> = None;
    let mut resolution = None;
    let mut region: Option<(u32, u32, u32, u32)> = None;
    let mut grid: Option<u32> = None;
    let mut run_ocr_flag = false;
    let mut i = 1;

    while i < args.len() {
        match args[i].as_str() {
            "-o" | "--output" => {
                i += 1;
                if i < args.len() { output = Some(PathBuf::from(&args[i])); }
            }
            "-r" | "--resolution" | "--res" => {
                i += 1;
                if i < args.len() { resolution = Some(args[i].parse().unwrap_or(ResolutionMode::Fit(1920, 1080))); }
            }
            "-c" | "--region" | "--crop" => {
                i += 1;
                if i < args.len() { region = Some(parse_region(&args[i])?); }
            }
            "-g" | "--grid" => {
                let mut spacing = 100u32;
                if i + 1 < args.len() && !args[i + 1].starts_with('-') {
                    if let Ok(val) = args[i + 1].parse::<u32>() {
                        spacing = val;
                        i += 1;
                    }
                }
                grid = Some(spacing.max(20));
            }
            "--ocr" => {
                run_ocr_flag = true;
            }
            pos if !pos.starts_with('-') => {
                output = Some(PathBuf::from(pos));
            }
            _ => {}
        }
        i += 1;
    }

    let final_res = resolution.unwrap_or(if region.is_some() {
        ResolutionMode::Original
    } else {
        ResolutionMode::Fit(1920, 1080)
    });

    let final_dest = output.unwrap_or_else(|| PathBuf::from("screenshot.png"));

    println!("Capturing screenshot...");
    let img = capture_screenshot().await?;

    let final_path = if final_dest.is_absolute() {
        final_dest
    } else {
        env::current_dir()?.join(&final_dest)
    };

    if let Some(parent) = final_path.parent() {
        if !parent.as_os_str().is_empty() && !parent.exists() {
            fs::create_dir_all(parent)?;
        }
    }

    let cropped = crop_if_needed(img, region);
    let orig_dimensions = cropped.dimensions();

    let mut processed = match final_res {
        ResolutionMode::Original => cropped.to_rgba8(),
        ResolutionMode::Fit(max_w, max_h) => {
            if orig_dimensions == (max_w, max_h) {
                cropped.to_rgba8()
            } else {
                cropped.resize(max_w, max_h, FilterType::Lanczos3).to_rgba8()
            }
        }
        ResolutionMode::Exact(exact_w, exact_h) => {
            if orig_dimensions == (exact_w, exact_h) {
                cropped.to_rgba8()
            } else {
                cropped.resize_exact(exact_w, exact_h, FilterType::Lanczos3).to_rgba8()
            }
        }
    };

    if let Some(spacing) = grid {
        draw_grid(&mut processed, spacing);
    }

    let final_dimensions = processed.dimensions();
    let ext = final_path
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("")
        .to_lowercase();
    if ext != "png" && ext != "jpg" && ext != "jpeg" {
        processed.save_with_format(&final_path, image::ImageFormat::Png)?;
    } else {
        processed.save(&final_path)?;
    }

    println!(
        "Screenshot saved to: {} ({}x{})",
        final_path.display(),
        final_dimensions.0,
        final_dimensions.1
    );

    if run_ocr_flag {
        run_ocr(&final_path, false)?;
    }

    Ok(())
}
