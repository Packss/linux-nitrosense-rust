/// Low-level read/write access to the laptop Embedded Controller (EC).
///
/// Two kernel modules are tried in order:
///   1. `ec_sys`  → `/sys/kernel/debug/ec/ec0/io`
///   2. `acpi_ec` → `/dev/ec`

use std::fs::{self, File, OpenOptions};
use std::io::{self, Read, Seek, SeekFrom, Write};
use std::process::Command;

/// Handle for communicating with the EC.
pub struct EcWriter {
    file: File,
    buffer: Vec<u8>,
}

/// Errors that can occur during EC operations.
#[derive(Debug)]
pub enum EcError {
    NoDevice,
    Io(io::Error),
    EmptyBuffer,
}

impl std::fmt::Display for EcError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            EcError::NoDevice => write!(f, "failed to open any EC device file"),
            EcError::Io(e) => write!(f, "EC I/O error: {e}"),
            EcError::EmptyBuffer => write!(f, "empty EC buffer – call refresh() first"),
        }
    }
}

impl From<io::Error> for EcError {
    fn from(e: io::Error) -> Self {
        EcError::Io(e)
    }
}

impl EcWriter {
    /// Open the EC device file.  Tries `ec_sys` first, falls back to `acpi_ec`.
    pub fn new() -> Result<Self, EcError> {
        let file = Self::load_ec_sys()
            .or_else(|| Self::load_acpi_ec())
            .ok_or(EcError::NoDevice)?;

        Ok(EcWriter {
            file,
            buffer: Vec::new(),
        })
    }

    // -- kernel module helpers ----------------------------------------------

    fn load_ec_sys() -> Option<File> {
        let path = "/sys/kernel/debug/ec/ec0/io";

        // First, check if the file exists and is writable
        if fs::metadata(path).is_ok() {
            if let Ok(f) = OpenOptions::new().read(true).write(true).open(path) {
                println!("'ec_sys' interface found and writable.");
                return Some(f);
            }
        }

        // Unload then reload with write support
        println!("Reloading 'ec_sys' with write support...");
        let _ = Command::new("modprobe").args(["-r", "ec_sys"]).status();
        let _ = Command::new("modprobe")
            .args(["ec_sys", "write_support=1"])
            .status();

        if fs::metadata(path).is_ok() {
            match OpenOptions::new().read(true).write(true).open(path) {
                Ok(f) => {
                    println!("Loaded 'ec_sys' module successfully.");
                    return Some(f);
                }
                Err(e) => {
                    eprintln!("Opening EC as rw failed: {e}");
                    eprintln!("Trying to load acpi_ec…");
                }
            }
        } else {
            eprintln!("Failed to load 'ec_sys' module. Attempting 'acpi_ec'…");
        }
        None
    }

    fn load_acpi_ec() -> Option<File> {
        let _ = Command::new("modprobe").arg("acpi_ec").status();

        let path = "/dev/ec";
        if fs::metadata(path).is_ok() {
            match OpenOptions::new().read(true).write(true).open(path) {
                Ok(f) => {
                    println!("Loaded 'acpi_ec' module successfully.");
                    return Some(f);
                }
                Err(e) => {
                    eprintln!("Error: failed to open {path}: {e}");
                }
            }
        }
        None
    }

    // -- public interface ---------------------------------------------------

    /// Write a single byte to an EC register.
    pub fn write(&mut self, address: u8, value: u8) {
        if let Err(e) = self.file.seek(SeekFrom::Start(address as u64)) {
            eprintln!("Error seeking EC to 0x{address:02X}: {e}");
            return;
        }
        if let Err(e) = self.file.write_all(&[value]) {
            eprintln!("Error writing 0x{value:02X} to EC 0x{address:02X}: {e}");
        }
    }

    /// Re-read the entire EC address space into an internal buffer.
    pub fn refresh(&mut self) {
        if let Err(e) = self.file.seek(SeekFrom::Start(0)) {
            eprintln!("Error seeking EC to start: {e}");
            return;
        }
        self.buffer.clear();
        if let Err(e) = self.file.read_to_end(&mut self.buffer) {
            eprintln!("Error reading EC buffer: {e}");
            return;
        }
        if self.buffer.is_empty() {
            eprintln!("Warning: empty EC buffer after refresh!");
        }
    }

    /// Read a value from the buffered EC data.  Call [`refresh`] first.
    /// Returns 0 if the buffer is empty or address is out of range.
    pub fn read(&self, address: u8) -> u8 {
        self.buffer.get(address as usize).copied().unwrap_or_else(|| {
            eprintln!("Warning: EC read at 0x{address:02X} out of range (buffer len={})", self.buffer.len());
            0
        })
    }

    /// Gracefully close the EC file handle.
    pub fn shutdown(&mut self) {
        // `File` is closed on drop, but we print a message for parity.
        println!("EC access successfully terminated.");
    }
}

impl Drop for EcWriter {
    fn drop(&mut self) {
        println!("EC handle dropped.");
    }
}
