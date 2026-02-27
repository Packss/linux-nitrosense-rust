use std::fs;
use std::io::{self, BufRead, BufReader, Write};
use std::os::unix::fs::PermissionsExt;
use std::os::unix::net::{UnixListener, UnixStream};
use std::path::Path;

use crate::config::{NitroConfig, RgbConfig};
use crate::core::cpu_ctl::CpuController;
use crate::core::device_regs::{detect_device, EcRegisters};
use crate::core::ec_writer::EcWriter;
use crate::protocol::{BatteryStatus, EcData, FanMode, NitroMode, Request, Response, SOCKET_PATH};
use crate::utils::keyboard::{self, Rgb};

struct DaemonState {
    ec: EcWriter,
    regs: EcRegisters,
    cpu_ctl: CpuController,
}

impl DaemonState {
    fn new() -> io::Result<Self> {
        let (regs, cpu_type) = detect_device();
        let ec = EcWriter::new().map_err(|e| io::Error::new(io::ErrorKind::Other, e.to_string()))?;
        
        Ok(Self {
            ec,
            regs,
            cpu_ctl: CpuController::new(cpu_type),
        })
    }

    fn get_fan_mode(&self, val: u8, auto: u8, turbo: u8, manual: u8) -> FanMode {
        if val == auto { FanMode::Auto }
        else if val == turbo { FanMode::Turbo }
        else if val == manual { FanMode::Manual }
        else { FanMode::Unknown(val) }
    }

    fn get_nitro_mode(&self, val: u8) -> NitroMode {
         if val == self.regs.quiet_mode { NitroMode::Quiet }
         else if val == self.regs.default_mode { NitroMode::Default }
         else if val == self.regs.extreme_mode { NitroMode::Extreme }
         else { NitroMode::Unknown(val) }
    }

    fn get_battery_status(&self, val: u8) -> BatteryStatus {
         if val == self.regs.battery_charging { BatteryStatus::Charging }
         else if val == self.regs.battery_draining { BatteryStatus::Discharging }
         else if val == self.regs.battery_off { BatteryStatus::NotInUse }
         else { BatteryStatus::Unknown(val) }
    }

    fn handle_request(&mut self, req: Request) -> Response {
        match req {
            Request::GetStatus => {
                self.ec.refresh();
                
                // Refresh voltage info (this might be slow)
                self.cpu_ctl.refresh_voltage();
                
                let cpu_mode_val = self.ec.read(self.regs.cpu_fan_mode_control);
                let gpu_mode_val = self.ec.read(self.regs.gpu_fan_mode_control);
                let nitro_mode_val = self.ec.read(self.regs.nitro_mode);
                let battery_status_val = self.ec.read(self.regs.battery_status);

                let data = EcData {
                    cpu_temp: self.ec.read(self.regs.cpu_temp),
                    gpu_temp: self.ec.read(self.regs.gpu_temp),
                    sys_temp: self.ec.read(self.regs.sys_temp),
                    cpu_fan_speed: {
                        let hi = self.ec.read(self.regs.cpu_fan_speed_high) as u16;
                        let lo = self.ec.read(self.regs.cpu_fan_speed_low) as u16;
                        (lo << 8) | hi
                    },
                    gpu_fan_speed: {
                        let hi = self.ec.read(self.regs.gpu_fan_speed_high) as u16;
                        let lo = self.ec.read(self.regs.gpu_fan_speed_low) as u16;
                        (lo << 8) | hi
                    },
                    power_plugged_in: self.ec.read(self.regs.power_status) == self.regs.power_plugged_in,
                    battery_status: self.get_battery_status(battery_status_val),
                    cpu_mode: self.get_fan_mode(cpu_mode_val, self.regs.cpu_auto_mode, self.regs.cpu_turbo_mode, self.regs.cpu_manual_mode),
                    gpu_mode: self.get_fan_mode(gpu_mode_val, self.regs.gpu_auto_mode, self.regs.gpu_turbo_mode, self.regs.gpu_manual_mode),
                    nitro_mode: self.get_nitro_mode(nitro_mode_val),
                    kb_timeout: self.ec.read(self.regs.kb_30_sec_auto) == self.regs.kb_30_auto_on,
                    usb_charging: self.ec.read(self.regs.usb_charging_reg) == self.regs.usb_charging_on,
                    battery_charge_limit: self.ec.read(self.regs.battery_charge_limit) == self.regs.battery_limit_on,
                    voltage_info: self.cpu_ctl.voltage_info.clone(),
                    undervolt_status: self.cpu_ctl.undervolt_status.clone(),
                    cpu_manual_level: self.ec.read(self.regs.cpu_manual_speed_control),
                    gpu_manual_level: self.ec.read(self.regs.gpu_manual_speed_control),
                };
                Response::Status(data)
            }
            Request::SetCpuFanMode(mode) => {
                let val = match mode {
                    FanMode::Auto => self.regs.cpu_auto_mode,
                    FanMode::Turbo => self.regs.cpu_turbo_mode,
                    FanMode::Manual => self.regs.cpu_manual_mode,
                    _ => return Response::Error("Invalid mode".into()),
                };
                self.ec.write(self.regs.cpu_fan_mode_control, val);
                let mut cfg = NitroConfig::load_or_default();
                cfg.cpu_mode = val;
                cfg.save();
                Response::Ok
            }
            Request::SetGpuFanMode(mode) => {
                let val = match mode {
                    FanMode::Auto => self.regs.gpu_auto_mode,
                    FanMode::Turbo => self.regs.gpu_turbo_mode,
                    FanMode::Manual => self.regs.gpu_manual_mode,
                    _ => return Response::Error("Invalid mode".into()),
                };
                self.ec.write(self.regs.gpu_fan_mode_control, val);
                let mut cfg = NitroConfig::load_or_default();
                cfg.gpu_mode = val;
                cfg.save();
                Response::Ok
            }
            Request::SetCpuFanSpeed(val) => {
                self.ec.write(self.regs.cpu_manual_speed_control, val);
                Response::Ok
            }
            Request::SetGpuFanSpeed(val) => {
                self.ec.write(self.regs.gpu_manual_speed_control, val);
                Response::Ok
            }
            Request::SetNitroMode(mode) => {
                let val = match mode {
                    NitroMode::Quiet => self.regs.quiet_mode,
                    NitroMode::Default => self.regs.default_mode,
                    NitroMode::Extreme => self.regs.extreme_mode,
                     _ => return Response::Error("Invalid mode".into()),
                };
               
                self.ec.write(self.regs.nitro_mode, val);
                let mut cfg = NitroConfig::load_or_default();
                cfg.nitro_mode = val;
                cfg.save();
                Response::Ok
            }
            Request::SetKbTimeout(val) => {
                let reg_val = if val { self.regs.kb_30_auto_on } else { self.regs.kb_30_auto_off };
                self.ec.write(self.regs.kb_30_sec_auto, reg_val);
                
                let mut cfg = NitroConfig::load_or_default();
                cfg.kb_timeout = reg_val;
                cfg.save();
                Response::Ok
            }
            Request::SetUsbCharging(val) => {
                let v = if val { self.regs.usb_charging_on } else { self.regs.usb_charging_off };
                self.ec.write(self.regs.usb_charging_reg, v);
                let mut cfg = NitroConfig::load_or_default();
                cfg.usb_charging = v;
                cfg.save();
                Response::Ok
            }
            Request::SetBatteryLimit(val) => {
                let v = if val { self.regs.battery_limit_on } else { self.regs.battery_limit_off };
                self.ec.write(self.regs.battery_charge_limit, v);
                let mut cfg = NitroConfig::load_or_default();
                cfg.battery_charge_limit = v;
                cfg.save();
                Response::Ok
            }
            Request::SetKeyboardColor(zone, r, g, b) => {
                let color = Rgb { r, g, b };
                keyboard::set_mode(0, zone, 0, 0, 0, color);
                
                let mut rgb_cfg = RgbConfig::load().unwrap_or_default();
                rgb_cfg.mode = 0;
                rgb_cfg.zone = zone;
                rgb_cfg.color = color;
                rgb_cfg.save();
                
                Response::Ok
            }
            Request::ApplyUndervolt(idx) => {
                self.cpu_ctl.apply_undervolt(idx);
                Response::Ok
            }
        }
    }
}

pub fn run_daemon() {
    println!("Starting NitroSense daemon...");
    
    // Always force remove socket if it exists.
    if Path::new(SOCKET_PATH).exists() {
        if let Err(e) = fs::remove_file(SOCKET_PATH) {
            eprintln!("Error removing existing socket {}: {}. Is another instance running?", SOCKET_PATH, e);
            // If we can't remove it, we probably can't bind.
            // But let's try anyway, or exit.
        } else {
             println!("Removed stale socket file.");
        }
    }

    // Set up Ctrl+C handler
    if let Err(e) = ctrlc::set_handler(move || {
        println!("\nReceived shutdown signal. Cleaning up...");
        if Path::new(SOCKET_PATH).exists() {
            let _ = fs::remove_file(SOCKET_PATH);
            println!("Socket removed.");
        }
        std::process::exit(0);
    }) {
        eprintln!("Error setting Ctrl-C handler: {}", e);
    }

    let listener = match UnixListener::bind(SOCKET_PATH) {
        Ok(l) => l,
        Err(e) => {
             eprintln!("Failed to bind to socket: {}", e);
             return;
        }
    };

    // Set permissions to 666 so any user can connect (read/write to socket)
    if let Err(e) = fs::set_permissions(SOCKET_PATH, fs::Permissions::from_mode(0o666)) {
         eprintln!("Failed to set socket permissions: {}", e);
    }

    println!("NitroSense Daemon started.");
    
    // Simple restore
    if let Ok(mut state) = DaemonState::new() {
        if let Some(cfg) = NitroConfig::load() {
             let _ = state.ec.write(state.regs.nitro_mode, cfg.nitro_mode);
        }

        for stream in listener.incoming() {
            match stream {
                Ok(stream) => handle_client(stream, &mut state),
                Err(e) => eprintln!("Connection failed: {}", e),
            }
        }
    } else {
        eprintln!("Failed to initialize daemon hardware interface (are you root?)");
    }
}

fn handle_client(mut stream: UnixStream, state: &mut DaemonState) {
    let mut reader = BufReader::new(stream.try_clone().unwrap());
    loop {
        let mut line = String::new();
        match reader.read_line(&mut line) {
            Ok(0) => break, // EOF connection closed
            Ok(_) => {
                if line.trim().is_empty() { continue; }
                let req: Request = match serde_json::from_str(&line) {
                     Ok(r) => r,
                     Err(e) => {
                         let _ = writeln!(stream, "{}", serde_json::to_string(&Response::Error(e.to_string())).unwrap());
                         continue;
                     }
                };
                let resp = state.handle_request(req);
                if let Ok(resp_str) = serde_json::to_string(&resp) {
                    if let Err(_) = writeln!(stream, "{}", resp_str) {
                        break;
                    }
                }
            }
            Err(_) => break,
        }
    }
}
