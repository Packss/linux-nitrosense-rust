/// Acer per-zone RGB keyboard backlight control.
///
/// Communicates through two character devices exposed by the
/// `acer-wmi` / `acer-gkbbl` kernel driver:
///   - `/dev/acer-gkbbl-0`        – dynamic modes & brightness
///   - `/dev/acer-gkbbl-static-0` – static per-zone colour

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

/// Apply a keyboard lighting mode.
///
/// `mode == 0` → static (per-zone colour).
/// `mode >= 1` → dynamic effect (breath, neon, wave, shifting, zoom).
pub fn set_mode(
    mode: u8,
    zone: u8,
    speed: u8,
    brightness: u8,
    direction: u8,
    color: Rgb,
) {
    if mode == 0 {
        set_static(zone, color);
    } else {
        set_dynamic(mode, speed, brightness, direction, color);
    }
}

// -- internals --------------------------------------------------------------

fn set_static(zone: u8, color: Rgb) {
    if zone == 0 {
        // "all" – write to zones 1..=4
        for z in 1..=4u8 {
            write_device(DEVICE_STATIC, &static_payload(z, color));
        }
    } else {
        write_device(DEVICE_STATIC, &static_payload(zone, color));
    }
    // Apply brightness payload after static colour change
    write_device(DEVICE_DYNAMIC, &brightness_payload());
}

fn set_dynamic(mode: u8, speed: u8, brightness: u8, direction: u8, color: Rgb) {
    let mut payload = [0u8; PAYLOAD_SIZE];
    payload[0] = mode;
    payload[1] = speed;
    payload[2] = brightness;
    payload[3] = if mode == 3 { 8 } else { 0 }; // wave special
    payload[4] = direction;
    payload[5] = color.r;
    payload[6] = color.g;
    payload[7] = color.b;
    payload[9] = 1;
    write_device(DEVICE_DYNAMIC, &payload);
}

fn static_payload(zone: u8, color: Rgb) -> [u8; PAYLOAD_SIZE_STATIC] {
    [1 << (zone - 1), color.r, color.g, color.b]
}

fn brightness_payload() -> [u8; PAYLOAD_SIZE] {
    let mut p = [0u8; PAYLOAD_SIZE];
    p[2] = 0; // default brightness
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
        Err(e) => eprintln!("Error opening {path}: {e}"),
    }
}
