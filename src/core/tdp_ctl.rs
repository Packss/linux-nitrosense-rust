/// TDP control via `ryzenadj` for AMD Ryzen APUs.
///
/// Exposes a simple interface to set STAPM / Fast / Slow power limits
/// (all unified to a single TDP value) and switch
/// `--power-saving` / `--max-performance` bias flags.

use std::process::Command;

use crate::protocol::PowerProfile;

// ---------------------------------------------------------------------------
// Public API
// ---------------------------------------------------------------------------

/// Apply a TDP limit (in milliwatts) using `ryzenadj`.
///
/// Sets `--stapm-limit`, `--fast-limit`, and `--slow-limit` to the same
/// value so the user only has to think about one number.
///
/// Returns `Ok(())` on success or an error description.
pub fn set_tdp(tdp_mw: u32) -> Result<(), String> {
    let mw = tdp_mw.to_string();

    let output = Command::new("ryzenadj")
        .args(["--stapm-limit", &mw, "--fast-limit", &mw, "--slow-limit", &mw])
        .output()
        .map_err(|e| format!("Failed to execute ryzenadj: {}", e))?;

    if output.status.success() {
        Ok(())
    } else {
        let stderr = String::from_utf8_lossy(&output.stderr);
        Err(format!("ryzenadj failed: {}", stderr.trim()))
    }
}

/// Apply a predefined power profile.
///
/// 1. Sets the TDP to the profile's default value.
/// 2. Applies the `--power-saving` or `--max-performance` bias flag
///    (balanced applies neither).
pub fn set_power_profile(profile: PowerProfile) -> Result<(), String> {
    let tdp_mw = profile.default_tdp_mw();
    set_tdp(tdp_mw)?;

    // Apply the performance-bias flag (if any)
    let bias_flag = match profile {
        PowerProfile::PowerSaving => Some("--power-saving"),
        PowerProfile::MaxPerformance => Some("--max-performance"),
        PowerProfile::Balanced => None,
    };

    if let Some(flag) = bias_flag {
        let output = Command::new("ryzenadj")
            .arg(flag)
            .output()
            .map_err(|e| format!("Failed to execute ryzenadj bias: {}", e))?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(format!("ryzenadj bias failed: {}", stderr.trim()));
        }
    }

    Ok(())
}

/// Apply both a custom TDP and a power profile bias.
pub fn apply_tdp_and_profile(tdp_mw: u32, profile: PowerProfile) -> Result<(), String> {
    set_tdp(tdp_mw)?;

    let bias_flag = match profile {
        PowerProfile::PowerSaving => Some("--power-saving"),
        PowerProfile::MaxPerformance => Some("--max-performance"),
        PowerProfile::Balanced => Some("--max-performance"),
    };

    if let Some(flag) = bias_flag {
        let output = Command::new("ryzenadj")
            .arg(flag)
            .output()
            .map_err(|e| format!("Failed to execute ryzenadj bias: {}", e))?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(format!("ryzenadj bias failed: {}", stderr.trim()));
        }
    }

    Ok(())
}

/// Check whether `ryzenadj` is available on the system.
pub fn is_available() -> bool {
    Command::new("which")
        .arg("ryzenadj")
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false)
}
