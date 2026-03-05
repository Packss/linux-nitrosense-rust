/// Low-level read/write access to the laptop Embedded Controller (EC).
///
/// Three backends are tried in order:
///   1. `ec_sys`  → `/sys/kernel/debug/ec/ec0/io`
///   2. `acpi_ec` → `/dev/ec`
///   3. raw I/O ports → `/dev/port`  (uses EC command protocol on ports 0x62/0x66)

use std::fs::{self, File, OpenOptions};
use std::io::{self, Read, Seek, SeekFrom, Write};
use std::process::Command;
use std::thread;
use std::time::{Duration, Instant};

/// Which backend is in use — determines how reads/writes are performed.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum EcBackend {
    /// Memory-mapped EC file (`ec_sys` or `acpi_ec`): seek + read/write.
    MappedFile,
    /// Raw I/O port access (`/dev/port`): must use EC command protocol.
    DevPort,
}

/// Handle for communicating with the EC.
pub struct EcWriter {
    file: File,
    buffer: Vec<u8>,
    backend: EcBackend,
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

/// EC command bytes written to port 0x66.
const EC_CMD_READ: u8 = 0x80;
const EC_CMD_WRITE: u8 = 0x81;

/// EC I/O port addresses.
const EC_DATA_PORT: u64 = 0x62;
const EC_CMD_PORT: u64 = 0x66;

/// Status-register bit masks (read from the command port).
const EC_STATUS_OBF: u8 = 0x01; // Output Buffer Full
const EC_STATUS_IBF: u8 = 0x02; // Input Buffer Full

/// Maximum time to wait for the EC to become ready.
const EC_TIMEOUT: Duration = Duration::from_millis(500);

impl EcWriter {
    /// Open the EC device file.
    /// Tries `ec_sys` first, then `acpi_ec`, then raw `/dev/port`.
    pub fn new() -> Result<Self, EcError> {
        if let Some(f) = Self::load_ec_sys() {
            return Ok(EcWriter { file: f, buffer: Vec::new(), backend: EcBackend::MappedFile });
        }
        if let Some(f) = Self::load_acpi_ec() {
            return Ok(EcWriter { file: f, buffer: Vec::new(), backend: EcBackend::MappedFile });
        }
        if let Some(f) = Self::load_dev_port() {
            return Ok(EcWriter { file: f, buffer: Vec::new(), backend: EcBackend::DevPort });
        }
        Err(EcError::NoDevice)
    }

    // -- kernel module helpers ----------------------------------------------

    fn load_ec_sys() -> Option<File> {
        // First, check if the file already exists and is writable
        if fs::metadata("/sys/kernel/debug/ec/ec0/io").is_ok() {
            if let Ok(f) = OpenOptions::new().read(true).write(true).open("/sys/kernel/debug/ec/ec0/io") {
                println!("'ec_sys' interface found and writable.");
                return Some(f);
            }
        }

        // Unload then reload with write support
        println!("Reloading 'ec_sys' with write support...");
        let _ = Command::new("/usr/bin/env").args(["modprobe", "-r", "ec_sys"]).status();
        let _ = Command::new("/usr/bin/env")
            .args(["modprobe", "ec_sys", "write_support=on"])
            .status();

        let path = "/sys/kernel/debug/ec/ec0/io";
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
        let _ = Command::new("/usr/bin/env").args(["modprobe", "acpi_ec"]).status();

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

    fn load_dev_port() -> Option<File> {
        if fs::metadata("/dev/port").is_ok() {
            match OpenOptions::new().read(true).write(true).open("/dev/port") {
                Ok(f) => {
                    println!("'/dev/port' interface found.");
                    return Some(f);
                }
                Err(e) => {
                    eprintln!("Error: failed to open /dev/port: {e}");
                }
            }
        }
        None
    }

    // -- /dev/port EC protocol helpers --------------------------------------

    /// Read one byte from an x86 I/O port via `/dev/port`.
    fn port_read_byte(file: &mut File, port: u64) -> io::Result<u8> {
        file.seek(SeekFrom::Start(port))?;
        let mut buf = [0u8; 1];
        file.read_exact(&mut buf)?;
        Ok(buf[0])
    }

    /// Write one byte to an x86 I/O port via `/dev/port`.
    fn port_write_byte(file: &mut File, port: u64, value: u8) -> io::Result<()> {
        file.seek(SeekFrom::Start(port))?;
        file.write_all(&[value])
    }

    /// Spin-wait until the EC Input Buffer Full (IBF) flag clears.
    fn wait_ibf_clear(file: &mut File) -> io::Result<()> {
        let start = Instant::now();
        loop {
            let status = Self::port_read_byte(file, EC_CMD_PORT)?;
            if status & EC_STATUS_IBF == 0 {
                return Ok(());
            }
            if start.elapsed() > EC_TIMEOUT {
                return Err(io::Error::new(io::ErrorKind::TimedOut, "EC IBF timeout"));
            }
            thread::sleep(Duration::from_micros(10));
        }
    }

    /// Spin-wait until the EC Output Buffer Full (OBF) flag sets.
    fn wait_obf_set(file: &mut File) -> io::Result<()> {
        let start = Instant::now();
        loop {
            let status = Self::port_read_byte(file, EC_CMD_PORT)?;
            if status & EC_STATUS_OBF != 0 {
                return Ok(());
            }
            if start.elapsed() > EC_TIMEOUT {
                return Err(io::Error::new(io::ErrorKind::TimedOut, "EC OBF timeout"));
            }
            thread::sleep(Duration::from_micros(10));
        }
    }

    /// Read a single EC register using the command protocol over `/dev/port`.
    fn ec_port_read(&mut self, address: u8) -> io::Result<u8> {
        Self::wait_ibf_clear(&mut self.file)?;
        Self::port_write_byte(&mut self.file, EC_CMD_PORT, EC_CMD_READ)?;
        Self::wait_ibf_clear(&mut self.file)?;
        Self::port_write_byte(&mut self.file, EC_DATA_PORT, address)?;
        Self::wait_obf_set(&mut self.file)?;
        Self::port_read_byte(&mut self.file, EC_DATA_PORT)
    }

    /// Write a single EC register using the command protocol over `/dev/port`.
    fn ec_port_write(&mut self, address: u8, value: u8) -> io::Result<()> {
        Self::wait_ibf_clear(&mut self.file)?;
        Self::port_write_byte(&mut self.file, EC_CMD_PORT, EC_CMD_WRITE)?;
        Self::wait_ibf_clear(&mut self.file)?;
        Self::port_write_byte(&mut self.file, EC_DATA_PORT, address)?;
        Self::wait_ibf_clear(&mut self.file)?;
        Self::port_write_byte(&mut self.file, EC_DATA_PORT, value)
    }

    // -- public interface ---------------------------------------------------

    /// Write a single byte to an EC register.
    pub fn write(&mut self, address: u8, value: u8) {
        match self.backend {
            EcBackend::MappedFile => {
                if let Err(e) = self.file.seek(SeekFrom::Start(address as u64)) {
                    eprintln!("Error seeking EC to 0x{address:02X}: {e}");
                    return;
                }
                if let Err(e) = self.file.write_all(&[value]) {
                    eprintln!("Error writing 0x{value:02X} to EC 0x{address:02X}: {e}");
                }
            }
            EcBackend::DevPort => {
                if let Err(e) = self.ec_port_write(address, value) {
                    eprintln!("Error writing 0x{value:02X} to EC 0x{address:02X} via /dev/port: {e}");
                }
            }
        }
    }

    /// Re-read the entire EC address space into an internal buffer.
    pub fn refresh(&mut self) {
        match self.backend {
            EcBackend::MappedFile => {
                if let Err(e) = self.file.seek(SeekFrom::Start(0)) {
                    eprintln!("Error seeking EC to start: {e}");
                    return;
                }
                self.buffer.clear();
                if let Err(e) = self.file.read_to_end(&mut self.buffer) {
                    eprintln!("Error reading EC buffer: {e}");
                    return;
                }
            }
            EcBackend::DevPort => {
                self.buffer.clear();
                self.buffer.resize(256, 0);
                for addr in 0u8..=255u8 {
                    match self.ec_port_read(addr) {
                        Ok(val) => self.buffer[addr as usize] = val,
                        Err(e) => {
                            eprintln!("Error reading EC 0x{addr:02X} via /dev/port: {e}");
                            // Keep going — partial data is better than none
                        }
                    }
                    // We stop early if we wrapped to 0 (u8 overflow), but the
                    // for-loop over 0..=255 handles this correctly.
                }
            }
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
