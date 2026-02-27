/// CPU-specific voltage monitoring and undervolting.
///
/// Dispatches to the correct backend (`amd` / `intel`) based on the detected
/// [`CpuType`].  On unsupported CPUs every operation is a no-op that returns
/// a human-readable message.

use std::process::Command;

use serde::{Deserialize, Serialize};

use super::device_regs::CpuType;

// ---------------------------------------------------------------------------
// Public types shared by all backends
// ---------------------------------------------------------------------------

/// Snapshot of the current voltage state.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VoltageInfo {
    pub voltage: f64,
    pub min_recorded: f64,
    pub max_recorded: f64,
}

impl Default for VoltageInfo {
    fn default() -> Self {
        Self {
            voltage: 0.5,
            min_recorded: 2.0,
            max_recorded: 0.0,
        }
    }
}

impl VoltageInfo {
    /// Update min/max tracking after reading a new voltage.
    pub fn update(&mut self, v: f64) {
        self.voltage = v;
        if v < self.min_recorded {
            self.min_recorded = v;
        }
        if v > self.max_recorded {
            self.max_recorded = v;
        }
    }
}

// ---------------------------------------------------------------------------
// Helper – run a command and capture stdout
// ---------------------------------------------------------------------------

fn run_command(cmd: &str, args: &[&str]) -> String {
    Command::new(cmd)
        .args(args)
        .output()
        .map(|o| String::from_utf8_lossy(&o.stdout).to_string())
        .unwrap_or_default()
}

// ---------------------------------------------------------------------------
// AMD backend
// ---------------------------------------------------------------------------

mod amd {
    use super::*;

    pub fn check_undervolt_status() -> String {
        let raw = run_command("amdctl", &["-m", "-g", "-c0"]);
        let lines: Vec<&str> = raw.lines().collect();

        // Skip the first 3 header lines, extract relevant columns
        lines
            .iter()
            .skip(3)
            .filter_map(|line| {
                let cols: Vec<&str> = line.split_whitespace().collect();
                if cols.len() > 11 {
                    Some(format!(
                        "{}\t{}\t{}\t{}\t{}",
                        cols[0],
                        cols[5],
                        cols[6].replace(".00", ""),
                        cols[7],
                        cols[11],
                    ))
                } else {
                    None
                }
            })
            .collect::<Vec<_>>()
            .join("\n")
    }

    pub fn apply_undervolt(dropdown_index: usize) -> String {
        let vid = if dropdown_index == 0 {
            1
        } else {
            dropdown_index * 16
        };
        run_command("amdctl", &["-m", &format!("-v{vid}")]);
        check_undervolt_status()
    }

    pub fn check_voltage(info: &mut VoltageInfo) {
        let raw = run_command("amdctl", &["-g", "-c0"]);
        let mut voltages = Vec::new();

        for line in raw.lines() {
            for word in line.split_whitespace() {
                if word.ends_with("mV") {
                    if let Ok(mv) = word.trim_end_matches("mV").parse::<f64>() {
                        voltages.push(mv / 1000.0);
                    }
                }
            }
        }

        if !voltages.is_empty() {
            let avg = voltages.iter().sum::<f64>() / voltages.len() as f64;
            info.update(avg);
        }
    }
}

// ---------------------------------------------------------------------------
// Intel backend
// ---------------------------------------------------------------------------

mod intel {
    use super::*;

    pub fn check_undervolt_status() -> String {
        "Undervolt not supported for Intel CPUs.".to_string()
    }

    pub fn apply_undervolt(_dropdown_index: usize) -> String {
        "Undervolt not supported for Intel CPUs.".to_string()
    }

    pub fn check_voltage(info: &mut VoltageInfo) {
        // `rdmsr 0x198` – reads IA32_PERF_STATUS from all cores
        let raw = run_command("sudo", &["rdmsr", "0x198", "-a", "-u", "--bitfield", "47:32"]);

        let values: Vec<f64> = raw
            .lines()
            .filter_map(|l| l.trim().parse::<f64>().ok())
            .collect();

        if !values.is_empty() {
            let avg = values.iter().sum::<f64>() / values.len() as f64;
            let voltage = avg / 8192.0;
            info.update(voltage);
        }
    }
}

// ---------------------------------------------------------------------------
// Dispatcher
// ---------------------------------------------------------------------------

/// CPU control dispatcher – picks the right backend at construction time.
pub struct CpuController {
    cpu_type: CpuType,
    pub voltage_info: VoltageInfo,
    pub undervolt_status: String,
}

impl CpuController {
    pub fn new(cpu_type: CpuType) -> Self {
        let undervolt_status = match cpu_type {
            CpuType::Amd => amd::check_undervolt_status(),
            CpuType::Intel => intel::check_undervolt_status(),
            CpuType::Unknown => "Undervolt not supported for this CPU type.".into(),
        };

        Self {
            cpu_type,
            voltage_info: VoltageInfo::default(),
            undervolt_status,
        }
    }

    pub fn apply_undervolt(&mut self, dropdown_index: usize) {
        self.undervolt_status = match self.cpu_type {
            CpuType::Amd => amd::apply_undervolt(dropdown_index),
            CpuType::Intel => intel::apply_undervolt(dropdown_index),
            CpuType::Unknown => "Undervolt not supported for this CPU type.".into(),
        };
    }

    pub fn refresh_voltage(&mut self) {
        match self.cpu_type {
            CpuType::Amd => amd::check_voltage(&mut self.voltage_info),
            CpuType::Intel => intel::check_voltage(&mut self.voltage_info),
            CpuType::Unknown => {}
        }
    }
}
