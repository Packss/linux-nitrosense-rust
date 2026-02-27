# Copilot Instructions for Linux NitroSense Rust

## Project Overview
This project is a Rust-based system utility for Acer Nitro laptops on Linux. It provides fan control, keyboard RGB configuration, and power management by interfacing directly with the laptop's Embedded Controller (EC).
The UI is built with GTK4.

## Architecture & Codebase Structure
- **Core Hardware Logic (`src/core/`)**: This is the "driver" layer.
    - `ec_writer.rs`: Manages low-level I/O to the EC. It dynamically loads kernel modules (`ec_sys` or `acpi_ec`) to gain access.
    - `device_regs.rs`: Contains the hardware-specific register mappings (offsets) for diffent laptop models. **Critical Safety**: Incorrect values can brick firmware.
    - `cpu_ctl.rs`: Handles CPU-specific control logic.
- **User Interface (`src/ui/`)**:
    - `gui.rs`: Builds the entire GTK4 interface programmatically (no `.ui` XML files). The UI state is shared via `Rc<RefCell<AppState>>`.
- **Configuration (`src/config.rs`)**:
    - Persists settings to `/etc/nitrosense/`.
    - Handles reading/writing legacy-compatible config files (`nitrosense.conf`, `rbg.conf`).

## Critical Developer Workflows
- **Environment**: Use `nix-shell` or ensure `pkg-config`, `gtk4`, and `glib` development headers are installed.
- **Privileges**: The application **MUST** run with `sudo` (root privileges) to access `/sys/kernel/debug/ec/...` or `/dev/ec` and to write to `/etc/`.
- **Building**: `cargo build` works in the correct environment (Shell.nix).
- **Testing**: No extensive unit tests for hardware interaction due to dependency on physical EC.

## Project-Specific patterns
- **EC Communication**:
    - The `EcWriter` struct is the single point of entry for hardware changes.
    - Use `poll_ec()` method in `AppState` to refresh all sensor data before reading.
- **GTK4 Usage**:
    - UI components are created in code (`gtk4::Application::builder()`, `gtk4::Box::new()`, etc).
    - State updates use a shared `AppState` object wrapped in `Rc<RefCell<_>>` passed to closures.
- **Error Handling**:
    - EC initialization failures should be handled gracefully at startup (`main.rs`) to avoid panics in FFI callbacks.

## Key Files
- `src/main.rs`: Entry point. Initializes EC, Config, and GTK App.
- `src/core/device_regs.rs`: **Read this** before adding support for new hardware models.
- `shell.nix`: Defines the build environment dependencies.
