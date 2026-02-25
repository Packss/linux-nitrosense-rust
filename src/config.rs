/// Persistent configuration for NitroSense and keyboard RGB.
///
/// Files are stored under `/etc/nitrosense/` as simple line-delimited values
/// (matching the original Python behaviour) so that existing configs remain
/// compatible.

use crate::utils::keyboard::Rgb;
use std::fs;
use std::io::{BufRead, BufReader, Write};
use std::path::Path;

const CONFIG_DIR: &str = "/etc/nitrosense";
const NITRO_CONF: &str = "nitrosense.conf";
const RGB_CONF: &str = "rbg.conf"; // keep original filename for compat

fn ensure_dir() {
    let _ = fs::create_dir_all(CONFIG_DIR);
}

fn conf_path(name: &str) -> String {
    format!("{CONFIG_DIR}/{name}")
}

// ---------------------------------------------------------------------------
// NitroSense system config
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
pub struct NitroConfig {
    pub cpu_mode: u8,
    pub gpu_mode: u8,
    pub kb_timeout: u8,
    pub usb_charging: u8,
    pub nitro_mode: u8,
    pub battery_charge_limit: u8,
}

impl NitroConfig {
    pub fn load_or_default() -> Self {
        Self::load().unwrap_or_else(|| Self {
            cpu_mode: 0, 
            gpu_mode: 0,
            kb_timeout: 0,
            usb_charging: 0,
            nitro_mode: 0,
            battery_charge_limit: 0,
        })
    }

    pub fn save(&self) {
        ensure_dir();
        let path = conf_path(NITRO_CONF);
        let mut f = match fs::File::create(&path) {
            Ok(f) => f,
            Err(e) => {
                eprintln!("Failed to write {path}: {e}");
                return;
            }
        };
        let _ = writeln!(f, "{}", self.cpu_mode);
        let _ = writeln!(f, "{}", self.gpu_mode);
        let _ = writeln!(f, "{}", self.kb_timeout);
        let _ = writeln!(f, "{}", self.usb_charging);
        let _ = writeln!(f, "{}", self.nitro_mode);
        let _ = writeln!(f, "{}", self.battery_charge_limit);
    }

    pub fn load() -> Option<Self> {
        let path = conf_path(NITRO_CONF);
        if !Path::new(&path).exists() {
            return None;
        }
        let f = fs::File::open(&path).ok()?;
        let mut lines = BufReader::new(f).lines();

        let mut next_u8 = || -> Option<u8> {
            lines
                .next()?
                .ok()?
                .trim()
                .parse()
                .ok()
        };

        Some(NitroConfig {
            cpu_mode: next_u8()?,
            gpu_mode: next_u8()?,
            kb_timeout: next_u8()?,
            usb_charging: next_u8()?,
            nitro_mode: next_u8()?,
            battery_charge_limit: next_u8()?,
        })
    }
}

// ---------------------------------------------------------------------------
// Keyboard RGB config
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
pub struct RgbConfig {
    pub mode: u8,
    pub zone: u8,
    pub speed: u8,
    pub brightness: u8,
    pub direction: u8,
    pub color: Rgb,
}

impl Default for RgbConfig {
    fn default() -> Self {
        Self {
            mode: 0,
            zone: 0,
            speed: 0,
            brightness: 0,
            direction: 0,
            color: Rgb::default(),
        }
    }
}

impl RgbConfig {
    pub fn save(&self) {
        ensure_dir();
        let path = conf_path(RGB_CONF);
        let mut f = match fs::File::create(&path) {
            Ok(f) => f,
            Err(e) => {
                eprintln!("Failed to write {path}: {e}");
                return;
            }
        };
        let _ = writeln!(f, "{}", self.mode);
        let _ = writeln!(f, "{}", self.zone);
        let _ = writeln!(f, "{}", self.speed);
        let _ = writeln!(f, "{}", self.brightness);
        let _ = writeln!(f, "{}", self.direction);
        let _ = writeln!(f, "{}", self.color.r);
        let _ = writeln!(f, "{}", self.color.g);
        let _ = writeln!(f, "{}", self.color.b);
    }

    pub fn load() -> Option<Self> {
        let path = conf_path(RGB_CONF);
        if !Path::new(&path).exists() {
            return None;
        }
        let f = fs::File::open(&path).ok()?;
        let mut lines = BufReader::new(f).lines();

        let mut next_u8 = || -> Option<u8> {
            lines
                .next()?
                .ok()?
                .trim()
                .parse()
                .ok()
        };

        Some(RgbConfig {
            mode: next_u8()?,
            zone: next_u8()?,
            speed: next_u8()?,
            brightness: next_u8()?,
            direction: next_u8()?,
            color: Rgb {
                r: next_u8()?,
                g: next_u8()?,
                b: next_u8()?,
            },
        })
    }
}
