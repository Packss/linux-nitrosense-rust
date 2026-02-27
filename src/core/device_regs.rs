/// EC register definitions for supported Acer Nitro laptop models.
///
/// Each variant maps a logical name (e.g. `CpuFanModeControl`) to the EC
/// register address used on that specific hardware revision.  The addresses
/// are discovered through reverse-engineering and are **hardware-specific** –
/// writing the wrong value to the wrong register can brick your firmware.

use std::collections::HashMap;
use std::fs;
use std::process;

// ---------------------------------------------------------------------------
// Register set
// ---------------------------------------------------------------------------

/// Complete set of EC register addresses for one laptop model.
#[derive(Debug, Clone)]
#[allow(dead_code)]
pub struct EcRegisters {
    // GPU fan
    pub gpu_fan_mode_control: u8,
    pub gpu_auto_mode: u8,
    pub gpu_turbo_mode: u8,
    pub gpu_manual_mode: u8,
    pub gpu_manual_speed_control: u8,

    // CPU fan
    pub cpu_fan_mode_control: u8,
    pub cpu_auto_mode: u8,
    pub cpu_turbo_mode: u8,
    pub cpu_manual_mode: u8,
    pub cpu_manual_speed_control: u8,

    // Keyboard backlight timeout
    pub kb_30_sec_auto: u8,
    pub kb_30_auto_off: u8,
    pub kb_30_auto_on: u8,

    // Fan speed readback
    pub cpu_fan_speed_high: u8,
    pub cpu_fan_speed_low: u8,
    pub gpu_fan_speed_high: u8,
    pub gpu_fan_speed_low: u8,

    // Temperatures
    pub cpu_temp: u8,
    pub gpu_temp: u8,
    pub sys_temp: u8,

    // Power / battery
    pub power_status: u8,
    pub power_plugged_in: u8,
    pub power_unplugged: u8,

    pub battery_charge_limit: u8,
    pub battery_limit_on: u8,
    pub battery_limit_off: u8,

    pub battery_status: u8,
    pub battery_charging: u8,
    pub battery_draining: u8,
    pub battery_off: u8,

    // USB charging while powered off
    pub usb_charging_reg: u8,
    pub usb_charging_on: u8,
    pub usb_charging_off: u8,

    // Nitro performance mode
    pub nitro_mode: u8,
    pub quiet_mode: u8,
    pub default_mode: u8,
    pub extreme_mode: u8,
}

// ---------------------------------------------------------------------------
// Known register maps
// ---------------------------------------------------------------------------

/// AN515-46 / AN515-54 / AN515-56 / AN515-58 register set.
pub const ECS_AN515_46: EcRegisters = EcRegisters {
    gpu_fan_mode_control: 0x21,
    gpu_auto_mode: 0x10,
    gpu_turbo_mode: 0x20,
    gpu_manual_mode: 0x30,
    gpu_manual_speed_control: 0x3A,

    cpu_fan_mode_control: 0x22,
    cpu_auto_mode: 0x04,
    cpu_turbo_mode: 0x08,
    cpu_manual_mode: 0x0C,
    cpu_manual_speed_control: 0x37,

    kb_30_sec_auto: 0x06,
    kb_30_auto_off: 0x00,
    kb_30_auto_on: 0x1E,

    cpu_fan_speed_high: 0x13,
    cpu_fan_speed_low: 0x14,
    gpu_fan_speed_high: 0x15,
    gpu_fan_speed_low: 0x16,

    cpu_temp: 0xB0,
    gpu_temp: 0xB6,
    sys_temp: 0xB3,

    power_status: 0x00,
    power_plugged_in: 0x01,
    power_unplugged: 0x00,

    battery_charge_limit: 0x03,
    battery_limit_on: 0x51,
    battery_limit_off: 0x11,

    battery_status: 0xC1,
    battery_charging: 0x02,
    battery_draining: 0x01,
    battery_off: 0x00,

    usb_charging_reg: 0x08,
    usb_charging_on: 0x0F,
    usb_charging_off: 0x1F,

    nitro_mode: 0x2C,
    quiet_mode: 0x00,
    default_mode: 0x01,
    extreme_mode: 0x04,
};

/// AN515-44 register set (some addresses differ).
pub const ECS_AN515_44: EcRegisters = EcRegisters {
    gpu_fan_mode_control: 0x21,
    gpu_auto_mode: 0x10,
    gpu_turbo_mode: 0x20,
    gpu_manual_mode: 0x30,
    gpu_manual_speed_control: 0x3A,

    cpu_fan_mode_control: 0x22,
    cpu_auto_mode: 0x04,
    cpu_turbo_mode: 0x08,
    cpu_manual_mode: 0x0C,
    cpu_manual_speed_control: 0x37,

    kb_30_sec_auto: 0x06,
    kb_30_auto_off: 0x00,
    kb_30_auto_on: 0x1E,

    cpu_fan_speed_high: 0x13,
    cpu_fan_speed_low: 0x14,
    gpu_fan_speed_high: 0x15,
    gpu_fan_speed_low: 0x16,

    cpu_temp: 0xB0,
    gpu_temp: 0xB4,
    sys_temp: 0xB0,

    power_status: 0x00,
    power_plugged_in: 0x01,
    power_unplugged: 0x00,

    battery_charge_limit: 0x03,
    battery_limit_on: 0x40,
    battery_limit_off: 0x00,

    battery_status: 0xC1,
    battery_charging: 0x02,
    battery_draining: 0x01,
    battery_off: 0x00,

    usb_charging_reg: 0x08,
    usb_charging_on: 0x0F,
    usb_charging_off: 0x1F,

    nitro_mode: 0x2C,
    quiet_mode: 0x00,
    default_mode: 0x01,
    extreme_mode: 0x04,
};

// ---------------------------------------------------------------------------
// CPU type detection
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CpuType {
    Amd,
    Intel,
    Unknown,
}

// ---------------------------------------------------------------------------
// Model → register map
// ---------------------------------------------------------------------------

fn model_to_ecs() -> HashMap<&'static str, EcRegisters> {
    let mut m = HashMap::new();
    m.insert("Nitro AN515-44", ECS_AN515_44);
    m.insert("Nitro AN515-46", ECS_AN515_46);
    m.insert("Nitro AN515-54", ECS_AN515_46);
    m.insert("Nitro AN515-56", ECS_AN515_46);
    m.insert("Nitro AN515-57", ECS_AN515_46);
    m.insert("Nitro AN515-58", ECS_AN515_46);
    m
}

// ---------------------------------------------------------------------------
// DMI helpers (reads directly from sysfs, no external crate needed)
// ---------------------------------------------------------------------------

fn read_dmi_field(field: &str) -> Option<String> {
    let path = format!("/sys/devices/virtual/dmi/id/{}", field);
    fs::read_to_string(path).ok().map(|s| s.trim().to_string())
}

fn detect_model() -> String {
    // product_name usually contains e.g. "Nitro AN515-46"
    read_dmi_field("product_name").unwrap_or_else(|| "Unknown".into())
}

fn detect_cpu_type() -> CpuType {
    if let Ok(cpuinfo) = fs::read_to_string("/proc/cpuinfo") {
        let lower = cpuinfo.to_lowercase();
        if lower.contains("amd") {
            return CpuType::Amd;
        } else if lower.contains("intel") {
            return CpuType::Intel;
        }
    }
    CpuType::Unknown
}

// ---------------------------------------------------------------------------
// Public API – detect hardware and return the register set
// ---------------------------------------------------------------------------

/// Detects the laptop model and CPU type.  Returns `(EcRegisters, CpuType)` or
/// terminates the process with a helpful message when the model is unsupported.
pub fn detect_device() -> (EcRegisters, CpuType) {
    let model = detect_model();
    let cpu = detect_cpu_type();

    println!("Detected model : {model}");
    println!("Detected CPU   : {cpu:?}");

    let map = model_to_ecs();

    // Try exact match first, then substring match
    if let Some(regs) = map.get(model.as_str()) {
        println!("Using registers for {model}");
        return (regs.clone(), cpu);
    }

    // Substring fallback – some BIOS strings include extra text
    for (name, regs) in &map {
        if model.contains(name) {
            println!("Using registers for {name} (matched from '{model}')");
            return (regs.clone(), cpu);
        }
    }

    eprintln!("Device '{model}' is not supported!");
    process::exit(1);
}
