/// Acer per-zone RGB keyboard backlight control.

use std::fs::OpenOptions;
use std::io::Write;

const PAYLOAD_SIZE: usize = 16;
const PAYLOAD_SIZE_STATIC: usize = 4;

const DEVICE_DYNAMIC: &str = "/dev/acer-gkbbl-0";
const DEVICE_STATIC: &str = "/dev/acer-gkbbl-static-0";

/// RGB colour.
#[derive(Debug, Clone, Copy, serde::Serialize, serde::Deserialize)]
pub struct Rgb {
    pub r: u8,
    pub g: u8,
    pub b: u8,
}

impl Default for Rgb {
    fn default() -> Self {
        Self {
            r: 255,
            g: 255,
            b: 255,
        }
    }
}

pub fn set_mode(
    mode: u8,
    zone: u8,
    speed: u8,
    brightness: u8,
    direction: u8,
    color: Rgb,
) {
    if mode == 0 {
        set_static(zone, color, brightness);
    } else {
        set_dynamic(mode, speed, brightness, direction, color);
    }
}

fn set_static(zone: u8, color: Rgb, brightness: u8) {
    if zone == 0 {
        // "all" – write to zones 1..=4
        for z in 1..=4u8 {
            write_device(DEVICE_STATIC, &static_payload(z, color));
        }
    } else {
        write_device(DEVICE_STATIC, &static_payload(zone, color));
    }
    // Apply brightness payload after static colour change
    write_device(DEVICE_DYNAMIC, &brightness_payload(brightness));
}

fn set_dynamic(mode: u8, speed: u8, brightness: u8, direction: u8, color: Rgb) {
    let mut payload = [0u8; PAYLOAD_SIZE];
    payload[0] = mode;
    payload[1] = speed;
    payload[2] = brightness;
    payload[3] = if mode == 3 { 8 } else { 0 }; // Wave mode requires special flag
    payload[4] = direction;
    payload[5] = color.r;
    payload[6] = color.g;
    payload[7] = color.b;
    payload[9] = 1;
    write_device(DEVICE_DYNAMIC, &payload);
}

fn static_payload(zone: u8, color: Rgb) -> [u8; PAYLOAD_SIZE_STATIC] {
    // Zone 1-4. Bitmask for zone selection.
    [1 << (zone - 1), color.r, color.g, color.b]
}

fn brightness_payload(brightness: u8) -> [u8; PAYLOAD_SIZE] {
    let mut p = [0u8; PAYLOAD_SIZE];
    p[2] = brightness; 
    p[9] = 1; 
    p
}

fn write_device(path: &str, payload: &[u8]) {
    match OpenOptions::new().write(true).open(path) {
        Ok(mut f) => {
            if let Err(e) = f.write_all(payload) {
                eprintln!("Error writing to {path}: {e}");
            }
        }
        Err(e) => {
             // Silently fail if device doesn't exist (e.g. testing)
             // Log error but don't panic if device missing (e.g. not root) to open {path}: {e}");
        }
    }
}
