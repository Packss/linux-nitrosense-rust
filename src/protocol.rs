use serde::{Deserialize, Serialize};

use crate::core::cpu_ctl::VoltageInfo;

#[derive(Debug, Serialize, Deserialize)]
pub struct EcData {
    pub cpu_temp: u8,
    pub gpu_temp: u8,
    pub sys_temp: u8,
    pub cpu_fan_speed: u16,
    pub gpu_fan_speed: u16,
    pub power_plugged_in: bool,
    pub battery_status: BatteryStatus,
    pub cpu_mode: FanMode,
    pub gpu_mode: FanMode,
    pub nitro_mode: NitroMode,
    pub kb_timeout: bool,
    pub usb_charging: bool,
    pub battery_charge_limit: bool,
    pub voltage_info: VoltageInfo,
    pub undervolt_status: String,
    pub cpu_manual_level: u8,
    pub gpu_manual_level: u8,
}

#[derive(Debug, Serialize, Deserialize, PartialEq, Eq, Clone, Copy)]
pub enum FanMode {
    Auto,
    Turbo,
    Manual,
    Unknown(u8),
}

#[derive(Debug, Serialize, Deserialize, PartialEq, Eq, Clone, Copy)]
pub enum NitroMode {
    Quiet,
    Default,
    Extreme,
    Unknown(u8),
}

#[derive(Debug, Serialize, Deserialize, PartialEq, Eq, Clone, Copy)]
pub enum BatteryStatus {
    Charging,
    Discharging,
    NotInUse,
    Unknown(u8),
}

#[derive(Debug, Serialize, Deserialize)]
pub enum Request {
    GetStatus,
    SetCpuFanMode(FanMode),
    SetGpuFanMode(FanMode),
    SetCpuFanSpeed(u8), // Raw value for now, or percentage?
    SetGpuFanSpeed(u8),
    SetNitroMode(NitroMode),
    SetKbTimeout(bool),
    SetUsbCharging(bool),
    SetBatteryLimit(bool),
    SetKeyboardColor(u8, u8, u8, u8), // zone, r, g, b
    ApplyUndervolt(usize),
}

#[derive(Debug, Serialize, Deserialize)]
pub enum Response {
    Status(EcData),
    Ok,
    Error(String),
}
