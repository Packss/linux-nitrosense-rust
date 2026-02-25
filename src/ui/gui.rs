/// GTK 4 user interface for Linux NitroSense.
///
/// The UI is built entirely in Rust code (no  XML) so the structure is
/// self-contained and easy to reason about.

use gtk4::gdk;
use gtk4::glib;
use gtk4::prelude::*;
use gtk4::{
    Align, Box as GtkBox, Button, CheckButton, CssProvider, DropDown, Grid, Label,
    LevelBar, Orientation, Scale, Stack, StackSwitcher, StringList, StyleContext, TextView, Window,
};

use std::cell::RefCell;
use std::rc::Rc;

use crate::client::Client;
use crate::config::{NitroConfig, RgbConfig};
use crate::core::cpu_ctl::VoltageInfo;
use crate::protocol::{BatteryStatus, EcData, FanMode, NitroMode, Request, Response};
use crate::utils::keyboard::{self, Rgb};

// ---------------------------------------------------------------------------
// Shared application state
// ---------------------------------------------------------------------------

pub struct AppState {
    pub client: Client,

    // Runtime state (mirrored from Daemon)
    pub turbo_enabled: bool,
    
    // Values read from Daemon
    pub cpu_temp: u8,
    pub gpu_temp: u8,
    pub sys_temp: u8,
    pub cpu_fan_speed: u16,
    pub gpu_fan_speed: u16,
    
    pub cpu_mode: FanMode,
    pub gpu_mode: FanMode,
    pub nitro_mode: NitroMode,
    
    pub power_plugged_in: bool,
    pub battery_status: BatteryStatus,
    pub kb_timeout: bool, // true = timeout enabled (auto_off)
    pub usb_charging: bool,
    pub battery_charge_limit: bool,
    
    pub cpu_manual_level: u8,
    pub gpu_manual_level: u8,
    
    pub voltage_info: VoltageInfo,
    pub undervolt_status: String,

    // Keyboard RGB (Client side state for UI)
    pub rgb_config: RgbConfig,
    pub selected_color: Rgb,
}

impl AppState {
    pub fn new() -> Self {
        // Try to connect
        let client = match Client::new() {
            Ok(c) => c,
            Err(e) => {
                eprintln!("Failed to connect to daemon: {}", e);
                // We might want to panic or show error dialog.
                // For now, panic to simplicity as app cannot run without daemon.
                panic!("Could not connect to daemon. Is it running?");
            }
        };

        Self {
            client,
            turbo_enabled: false,
            cpu_mode: FanMode::Auto,
            gpu_mode: FanMode::Auto,
            nitro_mode: NitroMode::Default,
            cpu_temp: 0,
            gpu_temp: 0,
            sys_temp: 0,
            cpu_fan_speed: 0,
            gpu_fan_speed: 0,
            power_plugged_in: false,
            battery_status: BatteryStatus::Unknown(0),
            kb_timeout: false,
            usb_charging: false,
            battery_charge_limit: false,
            cpu_manual_level: 0,
            gpu_manual_level: 0,
            rgb_config: RgbConfig::load().unwrap_or_default(),
            selected_color: Rgb::default(),
            voltage_info: VoltageInfo { voltage: 0.0, min_recorded: 0.0, max_recorded: 0.0 },
            undervolt_status: String::new(),
        }
    }

    /// Refresh EC buffer and read all sensor / status registers via Daemon.
    pub fn poll_ec(&mut self) {
        match self.client.send(Request::GetStatus) {
            Ok(Response::Status(data)) => {
                self.cpu_temp = data.cpu_temp;
                self.gpu_temp = data.gpu_temp;
                self.sys_temp = data.sys_temp;
                
                self.cpu_fan_speed = data.cpu_fan_speed;
                self.gpu_fan_speed = data.gpu_fan_speed;
                
                self.cpu_mode = data.cpu_mode;
                self.gpu_mode = data.gpu_mode;
                self.nitro_mode = data.nitro_mode;
                
                self.power_plugged_in = data.power_plugged_in;
                self.battery_status = data.battery_status;
                self.kb_timeout = data.kb_timeout;
                self.usb_charging = data.usb_charging;
                self.battery_charge_limit = data.battery_charge_limit;
                
                self.cpu_manual_level = data.cpu_manual_level;
                self.gpu_manual_level = data.gpu_manual_level;
                
                self.voltage_info = data.voltage_info;
                self.undervolt_status = data.undervolt_status;
            }
            Ok(Response::Error(e)) => eprintln!("Daemon error: {}", e),
            Ok(_) => eprintln!("Unexpected response"),
            Err(e) => eprintln!("IPC error: {}", e),
        }
    }

    // -- fan mode commands --------------------------------------------------

    pub fn set_cpu_auto(&mut self) {
        let _ = self.client.send(Request::SetCpuFanMode(FanMode::Auto));
    }

    pub fn set_cpu_turbo(&mut self) {
        let _ = self.client.send(Request::SetCpuFanMode(FanMode::Turbo));
    }

    pub fn set_cpu_manual(&mut self) {
        let _ = self.client.send(Request::SetCpuFanMode(FanMode::Manual));
    }

    pub fn set_cpu_speed(&mut self, level: u8) {
        // level is 0-20. Register expects level * 5?
        let val = level * 5;
        let _ = self.client.send(Request::SetCpuFanSpeed(val));
    }

    pub fn set_gpu_auto(&mut self) {
        let _ = self.client.send(Request::SetGpuFanMode(FanMode::Auto));
    }

    pub fn set_gpu_turbo(&mut self) {
        let _ = self.client.send(Request::SetGpuFanMode(FanMode::Turbo));
    }

    pub fn set_gpu_manual(&mut self) {
        let _ = self.client.send(Request::SetGpuFanMode(FanMode::Manual));
    }

    pub fn set_gpu_speed(&mut self, level: u8) {
        let val = level * 5;
        let _ = self.client.send(Request::SetGpuFanSpeed(val));
    }

    // -- nitro mode ---------------------------------------------------------

    pub fn set_quiet_mode(&mut self) {
        let _ = self.client.send(Request::SetNitroMode(NitroMode::Quiet));
        self.global_auto();
    }

    pub fn set_default_mode(&mut self) {
        let _ = self.client.send(Request::SetNitroMode(NitroMode::Default));
        self.global_auto();
    }

    pub fn set_extreme_mode(&mut self) {
        let _ = self.client.send(Request::SetNitroMode(NitroMode::Extreme));
        self.global_auto();
    }

    pub fn set_turbo_mode(&mut self) {
        let _ = self.client.send(Request::SetNitroMode(NitroMode::Extreme));
        self.global_turbo();
    }

    fn global_auto(&mut self) {
        if self.turbo_enabled {
            self.turbo_enabled = false;
            self.set_cpu_auto();
            self.set_gpu_auto();
        }
    }

    fn global_turbo(&mut self) {
        if !self.turbo_enabled {
            self.turbo_enabled = true;
            self.set_cpu_turbo();
            self.set_gpu_turbo();
        }
    }

    // -- toggles ------------------------------------------------------------

    pub fn toggle_kb_timeout(&mut self, on: bool) {
        let _ = self.client.send(Request::SetKbTimeout(on));
    }

    pub fn toggle_usb_charging(&mut self, on: bool) {
        let _ = self.client.send(Request::SetUsbCharging(on));
    }

    pub fn toggle_charge_limit(&mut self, on: bool) {
        let _ = self.client.send(Request::SetBatteryLimit(on));
    }

    pub fn apply_undervolt(&mut self, idx: usize) {
        let _ = self.client.send(Request::ApplyUndervolt(idx));
    }
    
    pub fn refresh_voltage(&mut self) {
    }

    // -- config persistence -------------------------------------------------

    pub fn load_config(&mut self) {
        self.poll_ec();
    }

    // -- battery status string ----------------------------------------------

    pub fn battery_status_text(&self) -> &str {
        match self.battery_status {
            BatteryStatus::Charging => "Charging",
            BatteryStatus::Discharging => "Discharging",
            BatteryStatus::NotInUse => "Not In Use",
            BatteryStatus::Unknown(_) => "Unknown",
        }
    }

    pub fn nitro_mode_text(&self) -> &str {
        match self.nitro_mode {
            NitroMode::Quiet => "Quiet",
            NitroMode::Default => "Default",
            NitroMode::Extreme => "Extreme",
            NitroMode::Unknown(_) => "Unknown",
        }
    }

    pub fn charge_limit_text(&self) -> &str {
        if self.battery_charge_limit { "On" } else { "Off" }
    }

    pub fn shutdown(&mut self) {
        // Nothing to do
    }
}

// ---------------------------------------------------------------------------
// UI builder
// ---------------------------------------------------------------------------

const APP_CSS: &str = r#"
window {
    background-color: #1a1311;
    color: #e0d7d5;
}

.card {
    background-color: #2a201d;
    border-radius: 12px;
    padding: 16px;
    border: 1px solid rgba(255, 255, 255, 0.05);
    margin: 8px;
}

.header-btn {
    background-color: transparent;
    border: none;
    color: #e0d7d5;
    font-weight: bold;
    border-bottom: 2px solid transparent;
    border-radius: 0;
}

.header-btn:checked {
    color: #60a5fa; /* blue-400 */
    border-bottom: 2px solid #60a5fa;
}

.mode-btn {
    background-color: transparent;
    color: #e0d7d5;
    border: 1px solid rgba(255, 255, 255, 0.1);
    padding: 4px 12px;
    border-radius: 6px;
}

.mode-btn:checked {
    background-color: #2563eb; /* blue-600 */
    color: white;
    border-color: #2563eb;
}

.section-title {
    font-size: 14px;
    font-weight: bold;
    color: #60a5fa; /* blue-400 */
    margin-bottom: 12px;
}

.label-secondary {
    color: #9ca3af; /* gray-400 */
    font-size: 12px;
}

.value-text {
    font-family: monospace;
    font-size: 14px;
}

scale trough {
    background-color: rgba(255, 255, 255, 0.1);
}

scale highlight {
    background-color: #3b82f6;
}
"#;

pub fn build_ui(app: &gtk4::Application, state: Rc<RefCell<AppState>>) -> Window {
    let window = Window::builder()
        .application(app)
        .title("NitroSense")
        .default_width(780)
        .default_height(520)
        .resizable(true)
        .build();

    // Load CSS
    let provider = CssProvider::new();
    provider.load_from_data(APP_CSS);
    StyleContext::add_provider_for_display(
        &gdk::Display::default().expect("Could not connect to a display."),
        &provider,
        gtk4::STYLE_PROVIDER_PRIORITY_APPLICATION,
    );

    let main_vbox = GtkBox::new(Orientation::Vertical, 0);
    main_vbox.set_margin_top(20);
    main_vbox.set_margin_bottom(20);
    main_vbox.set_margin_start(20);
    main_vbox.set_margin_end(20);

    // --- Header ---
    let header = GtkBox::new(Orientation::Horizontal, 0);
    header.set_margin_bottom(20);

    // Left: Tabs (Home / Keyboard)
    let stack = Stack::new();
    let switcher = StackSwitcher::new();
    switcher.set_stack(Some(&stack));
    // We want custom styling for switcher buttons to match "header-btn"
    // But StackSwitcher creates buttons automatically. 
    // Let's just use the stack and switcher for now, effectively "Home" and "Keyboard" tabs.
    header.append(&switcher);

    // Spacer
    let spacer = GtkBox::new(Orientation::Horizontal, 0);
    spacer.set_hexpand(true);
    header.append(&spacer);

    // Right: Mode Selectors (Quiet, Default, Extreme)
    let mode_box = GtkBox::new(Orientation::Horizontal, 4);
    mode_box.add_css_class("card"); // mini card look or just transparent? React used bg-[#2a201d] p-1 rounded-lg
    // Actually the React code put this in headers.
    
    let mode_quiet = CheckButton::builder().label("Quiet").css_classes(["mode-btn"]).build();
    let mode_default = CheckButton::builder().label("Default").css_classes(["mode-btn"]).build();
    let mode_extreme = CheckButton::builder().label("Extreme").css_classes(["mode-btn"]).build();
    mode_default.set_group(Some(&mode_quiet));
    mode_extreme.set_group(Some(&mode_quiet));
    
    // Logic to set initial state... handled in poll? 
    // We need to sync initial state.
    {
        let s = state.borrow();
         match s.nitro_mode {
            NitroMode::Quiet => mode_quiet.set_active(true),
            NitroMode::Extreme => mode_extreme.set_active(true),
            _ => mode_default.set_active(true),
        }
    }
    
    // Callbacks
    { let st = Rc::clone(&state); mode_quiet.connect_toggled(move |btn| if btn.is_active() { if let Ok(mut s) = st.try_borrow_mut() { s.set_quiet_mode(); } }); }
    { let st = Rc::clone(&state); mode_default.connect_toggled(move |btn| if btn.is_active() { if let Ok(mut s) = st.try_borrow_mut() { s.set_default_mode(); } }); }
    { let st = Rc::clone(&state); mode_extreme.connect_toggled(move |btn| if btn.is_active() { if let Ok(mut s) = st.try_borrow_mut() { s.set_extreme_mode(); } }); }

    mode_box.append(&mode_quiet);
    mode_box.append(&mode_default);
    mode_box.append(&mode_extreme);
    header.append(&mode_box);
    main_vbox.append(&header);

    // --- Content ---
    let home_tab = build_home_tab(&state);
    stack.add_titled(&home_tab.container, Some("home"), "Home");

    let kbd_tab = build_keyboard_tab(&state);
    stack.add_titled(&kbd_tab, Some("keyboard"), "Keyboard");

    main_vbox.append(&stack);
    window.set_child(Some(&main_vbox));

    // Poll timer
    glib::timeout_add_local(std::time::Duration::from_millis(1500), move || {
        let mut s = state.borrow_mut();
        s.poll_ec();
        // Update widgets
        home_tab.update(&s);
        glib::ControlFlow::Continue
    });

    window
}

struct HomeTab {
    container: GtkBox,
    update_fn: Rc<RefCell<Box<dyn FnMut(&AppState)>>>,
}

impl HomeTab {
    fn update(&self, state: &AppState) {
        (self.update_fn.borrow_mut())(state);
    }
}

fn build_home_tab(state: &Rc<RefCell<AppState>>) -> HomeTab {
    // We use a Grid to emulate the "grid-cols-3" layout
    // Col 1: Power Card
    // Col 2-3: Telemetry Card
    // Row 2 (Col 1-3): Tuning Card
    
    let grid = Grid::new();
    grid.set_column_spacing(20);
    grid.set_row_spacing(20);
    grid.set_margin_bottom(20);

    // --- Power Card (Col 0, Row 0) ---
    let power_card = GtkBox::new(Orientation::Vertical, 12);
    power_card.add_css_class("card");
    
    let title = Label::new(Some("POWER STATUS"));
    title.add_css_class("section-title");
    title.set_halign(Align::Start);
    power_card.append(&title);
    
    let power_val = Label::new(None);
    power_val.set_halign(Align::End);
    power_val.add_css_class("value-text");
    power_card.append(&make_row("Power State", &power_val));
    
    let batt_val = Label::new(None);
    batt_val.set_halign(Align::End);
    batt_val.add_css_class("value-text");
    power_card.append(&make_row("Battery", &batt_val));
    
    let charge_val = Label::new(None);
    charge_val.set_halign(Align::End);
    charge_val.add_css_class("value-text");
    power_card.append(&make_row("Charge Limit", &charge_val));

    // Also add toggles here for convenience?
    // React UI has "80% Enabled" text.
    // Let's add small switches next to them? Or just click to toggle?
    // Following React design strictly: just text.
    // But we need controls. Let's add switches at bottom of card.
    let switches_box = GtkBox::new(Orientation::Vertical, 6);
    let limit_sw = CheckButton::with_label("Limit 80%");
    let usb_sw = CheckButton::with_label("USB Charge");
    let kb_sw = CheckButton::with_label("KB Timeout");
    
    { let st = Rc::clone(state); limit_sw.connect_toggled(move |btn| if let Ok(mut s) = st.try_borrow_mut() { s.toggle_charge_limit(btn.is_active()); }); }
    { let st = Rc::clone(state); usb_sw.connect_toggled(move |btn| if let Ok(mut s) = st.try_borrow_mut() { s.toggle_usb_charging(btn.is_active()); }); }
    { let st = Rc::clone(state); kb_sw.connect_toggled(move |btn| if let Ok(mut s) = st.try_borrow_mut() { s.toggle_kb_timeout(btn.is_active()); }); }

    switches_box.append(&limit_sw);
    switches_box.append(&usb_sw);
    switches_box.append(&kb_sw);
    power_card.append(&switches_box);

    grid.attach(&power_card, 0, 0, 1, 1);

    // --- Telemetry Card (Col 1-2, Row 0) ---
    let stats_card = GtkBox::new(Orientation::Vertical, 12);
    stats_card.add_css_class("card");
    stats_card.set_hexpand(true);

    let stats_title = Label::new(Some("SYSTEM HEALTH"));
    stats_title.add_css_class("section-title");
    stats_title.set_halign(Align::Start);
    stats_card.append(&stats_title);
    
    let stats_content = Grid::new();
    stats_content.set_column_spacing(40);
    
    // Temp Bars (Left side of card)
    let temps_box = GtkBox::new(Orientation::Vertical, 16);
    temps_box.set_hexpand(true);
    
    let cpu_temp_lbl = Label::new(None); 
    cpu_temp_lbl.set_halign(Align::End);
    let cpu_bar = LevelBar::new();
    cpu_bar.set_min_value(0.0);
    cpu_bar.set_max_value(100.0);
    temps_box.append(&make_row_multi("CPU Temp", &cpu_temp_lbl));
    temps_box.append(&cpu_bar);

    let gpu_temp_lbl = Label::new(None);
    gpu_temp_lbl.set_halign(Align::End);
    let gpu_bar = LevelBar::new();
    gpu_bar.set_min_value(0.0);
    gpu_bar.set_max_value(100.0);
    temps_box.append(&make_row_multi("GPU Temp", &gpu_temp_lbl));
    temps_box.append(&gpu_bar);
    
    stats_content.attach(&temps_box, 0, 0, 1, 1);

    // Fan RPMs (Right side)
    let fans_box = GtkBox::new(Orientation::Vertical, 16);
    fans_box.set_margin_start(20); // Border left basically
    
    let cpu_rpm = Label::new(Some("0 RPM"));
    cpu_rpm.add_css_class("value-text");
    cpu_rpm.set_markup("<span size='x-large'>0</span> <span size='small' color='gray'>RPM</span>");
    
    let gpu_rpm = Label::new(Some("0 RPM"));
    gpu_rpm.add_css_class("value-text");
    
    fans_box.append(&Label::new(Some("CPU FAN")));
    fans_box.append(&cpu_rpm);
    fans_box.append(&Label::new(Some("GPU FAN")));
    fans_box.append(&gpu_rpm);
    
    stats_content.attach(&fans_box, 1, 0, 1, 1);
    
    stats_card.append(&stats_content);
    grid.attach(&stats_card, 1, 0, 2, 1); // Span 2 cols

    // --- Tuning Card (Row 1, Span 3) ---
    let tune_card = GtkBox::new(Orientation::Vertical, 12);
    tune_card.add_css_class("card");
    
    let tune_title = Label::new(Some("PERFORMANCE TUNING"));
    tune_title.add_css_class("section-title");
    tune_title.set_halign(Align::Start);
    tune_card.append(&tune_title);
    
    let tune_grid = Grid::new();
    tune_grid.set_column_spacing(40);
    tune_grid.set_column_homogeneous(true);

    // 1. Undervolt
    let uv_box = GtkBox::new(Orientation::Vertical, 8);
    let uv_msg = Label::new(Some("Voltage Offset (CPU)"));
    uv_msg.set_halign(Align::Start);
    uv_msg.add_css_class("label-secondary");
    
    let uv_items = StringList::new(&["0mV", "-100mV", "-200mV"]); // Todo: more fine grained?
    let uv_dd = DropDown::new(Some(uv_items), gtk4::Expression::NONE);
    let uv_apply = Button::with_label("Apply Offset");
    let uv_status = Label::new(None);
    
    {
         let st = Rc::clone(state); 
         let dd = uv_dd.clone(); 
         let status = uv_status.clone();
         uv_apply.connect_clicked(move |_| {
             let idx = dd.selected() as usize;
             let mut s = st.borrow_mut();
             s.apply_undervolt(idx);
             status.set_text(&s.undervolt_status);
         });
    }

    uv_box.append(&uv_msg);
    uv_box.append(&uv_dd);
    uv_box.append(&uv_apply);
    uv_box.append(&uv_status);
    tune_grid.attach(&uv_box, 0, 0, 1, 1);

    // 2. CPU Fan Control
    let cpu_ctl = build_fan_column("CPU Control", state, true);
    tune_grid.attach(&cpu_ctl.widget, 1, 0, 1, 1);
    
    // 3. GPU Fan Control
    let gpu_ctl = build_fan_column("GPU Control", state, false);
    tune_grid.attach(&gpu_ctl.widget, 2, 0, 1, 1);

    tune_card.append(&tune_grid);
    grid.attach(&tune_card, 0, 1, 3, 1);

    // Wrapper for home tab
    let container = GtkBox::new(Orientation::Vertical, 0);
    container.append(&grid);

    // Create update function closure
    let update_fn = Rc::new(RefCell::new(Box::new(move |s: &AppState| {
        // Power Card
        power_val.set_label(if s.power_plugged_in { "ON" } else { "OFF" });
        batt_val.set_label(s.battery_status_text());
        charge_val.set_label(s.charge_limit_text());
        
        limit_sw.set_active(s.battery_charge_limit);
        usb_sw.set_active(s.usb_charging);
        kb_sw.set_active(s.kb_timeout);

        // Stats Card
        cpu_temp_lbl.set_label(&format!("{}°C", s.cpu_temp));
        cpu_bar.set_value(s.cpu_temp as f64);
        gpu_temp_lbl.set_label(&format!("{}°C", s.gpu_temp));
        gpu_bar.set_value(s.gpu_temp as f64);
        
        cpu_rpm.set_markup(&format!("<span size='x-large'>{}</span> <span size='small' color='gray'>RPM</span>", s.cpu_fan_speed));
        gpu_rpm.set_markup(&format!("<span size='x-large'>{}</span> <span size='small' color='gray'>RPM</span>", s.gpu_fan_speed));
        
        // Fan Controls
        // Sync sliders and checkbuttons state
        (cpu_ctl.update)(s);
        (gpu_ctl.update)(s);
        
        // UV status
        uv_status.set_text(&s.undervolt_status);
    }) as Box<dyn FnMut(&AppState)>));

    HomeTab { container, update_fn }
}

struct FanCol {
    widget: GtkBox,
    update: Box<dyn Fn(&AppState)>,
}

fn build_fan_column(title: &str, state: &Rc<RefCell<AppState>>, is_cpu: bool) -> FanCol {
    let vbox = GtkBox::new(Orientation::Vertical, 8);
    
    // Header row
    let header = GtkBox::new(Orientation::Horizontal, 0);
    let lbl = Label::new(Some(title));
    lbl.add_css_class("label-secondary");
    header.append(&lbl);
    
    let manual_badge = Label::new(Some("Manual"));
    manual_badge.add_css_class("mode-btn"); // reuse badge style
    manual_badge.set_halign(Align::End);
    manual_badge.set_hexpand(true);
    // header.append(&manual_badge); // Dynamically show?
    vbox.append(&header);
    
    // Slider
    let slider = Scale::with_range(Orientation::Horizontal, 0.0, 20.0, 1.0);
    
    // Mode Buttons (Radio behavior)
    let modes_box = GtkBox::new(Orientation::Horizontal, 2);
    let auto_btn = CheckButton::with_label("Auto");
    let max_btn = CheckButton::with_label("Max");
    // Ideally these look like the segment control in the React screenshot bottom
    // "Power Save | Balanced | Turbo" -> mapped to Auto | ? | Turbo/Max
    // Let's stick to CheckButtons for clarity
    let manual_btn = CheckButton::with_label("Custom");
    max_btn.set_group(Some(&auto_btn));
    manual_btn.set_group(Some(&auto_btn));
    
    modes_box.append(&auto_btn);
    modes_box.append(&max_btn);
    modes_box.append(&manual_btn);
    
    vbox.append(&slider);
    vbox.append(&modes_box);
    
    // Logic
    {
        let st = Rc::clone(state);
        auto_btn.connect_toggled(move |btn| if btn.is_active() { 
            if let Ok(mut s) = st.try_borrow_mut() {
                if is_cpu { s.set_cpu_auto(); } else { s.set_gpu_auto(); }
            }
        });
        
        let st = Rc::clone(state);
        max_btn.connect_toggled(move |btn| if btn.is_active() { 
             if let Ok(mut s) = st.try_borrow_mut() {
                 if is_cpu { s.set_cpu_turbo(); } else { s.set_gpu_turbo(); }
             }
        });
        
        let st = Rc::clone(state);
        manual_btn.connect_toggled(move |btn| if btn.is_active() { 
             if let Ok(mut s) = st.try_borrow_mut() {
                 if is_cpu { s.set_cpu_manual(); } else { s.set_gpu_manual(); }
             }
        });

        let st = Rc::clone(state);
        slider.connect_change_value(move |_, _, val| {
             if let Ok(mut s) = st.try_borrow_mut() {
                 if is_cpu { s.set_cpu_speed(val as u8); } else { s.set_gpu_speed(val as u8); }
             }
             glib::Propagation::Proceed
        });
    }
    
    let update = Box::new(move |s: &AppState| {
        let (mode, level) = if is_cpu { (s.cpu_mode, s.cpu_manual_level) } else { (s.gpu_mode, s.gpu_manual_level) };
        
        // Update selection without triggering signals? 
        // Signal blocks needed or check if active changes?
        // Gtk4 checkbuttons fire toggled only on user interaction? No, on set_active too.
        // We need to suppress or handle efficiently.
        // For simplicity, we just set. The signal handler calls set_mode, which is idempotent mostly.
        
        match mode {
            FanMode::Auto => auto_btn.set_active(true),
            FanMode::Turbo => max_btn.set_active(true),
            FanMode::Manual => manual_btn.set_active(true),
            _ => {},
        }
        
        slider.set_value(level as f64 / 5.0);
    });

    FanCol { widget: vbox, update }
}

fn make_row(label: &str, widget: &impl IsA<gtk4::Widget>) -> GtkBox {
    let box_ = GtkBox::new(Orientation::Horizontal, 10);
    let lbl = Label::new(Some(label));
    lbl.add_css_class("label-secondary");
    box_.append(&lbl);
    widget.set_hexpand(true);
    box_.append(widget);
    box_
}

fn make_row_multi(label: &str, widget: &impl IsA<gtk4::Widget>) -> GtkBox {
    let box_ = GtkBox::new(Orientation::Horizontal, 0);
    let lbl = Label::new(Some(label));
    lbl.add_css_class("label-secondary");
    box_.append(&lbl);
    
    let spacer = GtkBox::new(Orientation::Horizontal, 0);
    spacer.set_hexpand(true);
    box_.append(&spacer);
    
    box_.append(widget);
    box_
}

fn build_keyboard_tab(state: &Rc<RefCell<AppState>>) -> GtkBox {
    // Keep placeholder for now or move existing simple logic here
    let container = GtkBox::new(Orientation::Vertical, 6);
    container.set_margin_top(20);
    let label = Label::new(Some("Keyboard RGB Settings"));
    container.append(&label);
    
    // Zone selection, color picker logic...
    // Requires sending Request::SetKeyboardColor(zone, r, g, b)
    
    container
}
