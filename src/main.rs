use clap::{
    Parser, Subcommand,
    builder::styling::{AnsiColor, Effects, Styles},
};
use std::fs;
use std::io;
use std::ptr;

// ---------------------------------------------------------------------------
// ANSI colors
// ---------------------------------------------------------------------------

const CLAP_STYLES: Styles = Styles::styled()
    .header(AnsiColor::Yellow.on_default().effects(Effects::BOLD))
    .usage(AnsiColor::Yellow.on_default().effects(Effects::BOLD))
    .literal(AnsiColor::Green.on_default().effects(Effects::BOLD))
    .placeholder(AnsiColor::Cyan.on_default())
    .valid(AnsiColor::Green.on_default())
    .invalid(AnsiColor::Red.on_default().effects(Effects::BOLD))
    .error(AnsiColor::Red.on_default().effects(Effects::BOLD));

// ---------------------------------------------------------------------------
// ANSI colors
// ---------------------------------------------------------------------------

mod color {
    pub const RESET: &str = "\x1b[0m";
    pub const BOLD: &str = "\x1b[1m";
    pub const DIM: &str = "\x1b[2m";
    pub const CYAN: &str = "\x1b[36m";
    pub const GREEN: &str = "\x1b[32m";
    pub const YELLOW: &str = "\x1b[33m";
    pub const BRIGHT_WHITE: &str = "\x1b[97m";
}

// ---------------------------------------------------------------------------
// CLI
// ---------------------------------------------------------------------------

#[derive(Parser)]
#[command(name = "mem", about = "Physical memory access via /dev/mem", styles = CLAP_STYLES)]
struct Cli {
    #[command(subcommand)]
    cmd: Cmd,
}

#[derive(Subcommand)]
enum Cmd {
    /// Read memory (hex dump)
    #[command(alias = "r")]
    Read {
        /// Physical address (hex, 0x prefix optional)
        addr: String,
        /// Number of bytes to read
        #[arg(default_value = "0x80")]
        len: String,
    },
    /// Write value(s) to address
    #[command(alias = "w")]
    Write {
        /// Physical address
        addr: String,
        /// Values to write (hex)
        values: Vec<String>,
    },
    /// Dump memory region to file
    #[command(alias = "d")]
    Dump {
        /// Physical address
        addr: String,
        /// Length in bytes
        len: String,
        /// Output file
        file: String,
    },
    /// Load file contents into memory
    #[command(alias = "l")]
    Load {
        /// Physical address
        addr: String,
        /// Input file
        file: String,
    },
    /// Read or set a specific bit (set if value is provided)
    #[command(alias = "b")]
    Bit {
        /// Physical address
        addr: String,
        /// Bit number (0-31)
        bit: u8,
        /// Value: 0 or 1 (omit to read)
        value: Option<u8>,
    },
}

// ---------------------------------------------------------------------------
// DevMem
// ---------------------------------------------------------------------------

struct DevMem {
    fd: i32,
    page_size: usize,
}

impl DevMem {
    fn open() -> io::Result<Self> {
        let fd = unsafe {
            libc::open(
                b"/dev/mem\0".as_ptr() as *const libc::c_char,
                libc::O_RDWR | libc::O_SYNC,
            )
        };
        if fd < 0 {
            return Err(io::Error::last_os_error());
        }
        let page_size = unsafe { libc::sysconf(libc::_SC_PAGESIZE) } as usize;
        Ok(Self { fd, page_size })
    }

    /// Map a region covering [phys_addr, phys_addr+len).
    /// Returns (ptr_to_page_start, offset_within_page, mapped_length).
    fn mmap(&self, phys_addr: usize, len: usize) -> io::Result<(*mut u8, usize, usize)> {
        let page_mask = self.page_size - 1;
        let base = phys_addr & !page_mask;
        let offset = phys_addr & page_mask;
        let map_len = offset + len;
        // round up to page size
        let map_len = (map_len + page_mask) & !page_mask;

        let ptr = unsafe {
            libc::mmap(
                ptr::null_mut(),
                map_len,
                libc::PROT_READ | libc::PROT_WRITE,
                libc::MAP_SHARED,
                self.fd,
                base as libc::off_t,
            )
        };
        if ptr == libc::MAP_FAILED {
            return Err(io::Error::last_os_error());
        }
        Ok((ptr as *mut u8, offset, map_len))
    }

    fn munmap(&self, ptr: *mut u8, len: usize) {
        unsafe {
            libc::munmap(ptr as *mut libc::c_void, len);
        }
    }
}

impl Drop for DevMem {
    fn drop(&mut self) {
        unsafe {
            libc::close(self.fd);
        }
    }
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn parse_addr(s: &str) -> usize {
    let s = s
        .strip_prefix("0x")
        .or_else(|| s.strip_prefix("0X"))
        .unwrap_or(s);
    usize::from_str_radix(s, 16).expect("invalid hex address")
}

fn parse_num(s: &str) -> usize {
    let s_trimmed = s.strip_prefix("0x").or_else(|| s.strip_prefix("0X"));
    if let Some(hex) = s_trimmed {
        usize::from_str_radix(hex, 16).expect("invalid hex number")
    } else {
        s.parse().expect("invalid number")
    }
}

fn parse_value(s: &str) -> u64 {
    let s_trimmed = s.strip_prefix("0x").or_else(|| s.strip_prefix("0X"));
    if let Some(hex) = s_trimmed {
        u64::from_str_radix(hex, 16).expect("invalid hex value")
    } else {
        s.parse().expect("invalid value")
    }
}

/// Hexdump with 4-byte (32-bit word) grouping, with colors:
/// Address in cyan, non-zero words bold, zero words dim, ASCII in green.
fn hexdump(base_addr: usize, data: &[u8]) {
    use color::*;
    for (line_off, chunk) in data.chunks(16).enumerate() {
        let addr = base_addr + line_off * 16;
        print!("{CYAN}{:08x}:{RESET}", addr);

        // Print 4-byte groups
        for word_idx in 0..4 {
            let start = word_idx * 4;
            if start < chunk.len() {
                let end = (start + 4).min(chunk.len());
                let bytes = &chunk[start..end];
                let all_zero = bytes.iter().all(|&b| b == 0);
                if all_zero {
                    print!(" {DIM}");
                } else {
                    print!(" {BOLD}{BRIGHT_WHITE}");
                }
                for b in bytes.iter().rev() {
                    print!("{:02x}", b);
                }
                // pad if partial word
                for _ in 0..(4 - bytes.len()) {
                    print!("  ");
                }
                print!("{RESET}");
            } else {
                print!("         ");
            }
        }

        // ASCII
        print!("  {DIM}|{RESET}");
        for b in chunk {
            if b.is_ascii_graphic() || *b == b' ' {
                print!("{GREEN}{}{RESET}", *b as char);
            } else {
                print!("{DIM}.{RESET}");
            }
        }
        for _ in chunk.len()..16 {
            print!(" ");
        }
        println!("{DIM}|{RESET}");
    }
}

/// Display a u32 value in binary with bit position ruler and colored bits.
/// 1-bits are bold yellow, 0-bits are dim.
fn print_binary_u32(val: u32) {
    use color::*;

    // Bit position ruler
    print!("{DIM}bit: ");
    for i in (0..32).rev() {
        if i % 4 == 3 && i != 31 {
            print!(" ");
        }
        if i % 4 == 0 {
            print!("{:>2}", i);
        } else {
            print!("  ");
        }
    }
    println!("{RESET}");

    // Binary value with colored bits
    print!("val: ");
    for i in (0..32).rev() {
        if i % 4 == 3 && i != 31 {
            print!(" ");
        }
        let bit = (val >> i) & 1;
        if bit == 1 {
            print!(" {BOLD}{YELLOW}1{RESET}");
        } else {
            print!(" {DIM}0{RESET}");
        }
    }
    println!();
}

// ---------------------------------------------------------------------------
// Subcommand handlers
// ---------------------------------------------------------------------------

fn cmd_read(addr: usize, len: usize) {
    let len = len.max(4);
    assert!(addr % 4 == 0, "address not aligned for 32-bit access");

    let dev = DevMem::open().expect("failed to open /dev/mem");
    let (ptr, offset, map_len) = dev.mmap(addr, len).expect("mmap failed");

    let mut buf = vec![0u8; len];
    let virt = unsafe { ptr.add(offset) };

    let mut pos = 0;
    while pos + 4 <= len {
        let val = unsafe { ptr::read_volatile(virt.add(pos) as *const u32) };
        buf[pos..pos + 4].copy_from_slice(&val.to_ne_bytes());
        pos += 4;
    }
    // trailing bytes
    while pos < len {
        buf[pos] = unsafe { ptr::read_volatile(virt.add(pos)) };
        pos += 1;
    }

    // Show first u32 in binary
    if buf.len() >= 4 {
        let first_word = u32::from_le_bytes([buf[0], buf[1], buf[2], buf[3]]);
        print_binary_u32(first_word);
    }

    hexdump(addr, &buf);

    dev.munmap(ptr, map_len);
}

fn cmd_write(addr: usize, values: &[u64]) {
    assert!(addr % 4 == 0, "address not aligned for 32-bit access");

    let total = values.len() * 4;
    let dev = DevMem::open().expect("failed to open /dev/mem");
    let (ptr, offset, map_len) = dev.mmap(addr, total).expect("mmap failed");
    let virt = unsafe { ptr.add(offset) };

    for (i, &val) in values.iter().enumerate() {
        let off = i * 4;
        unsafe { ptr::write_volatile(virt.add(off) as *mut u32, val as u32) };
    }

    dev.munmap(ptr, map_len);
}

fn cmd_dump(addr: usize, len: usize, file: &str) {
    let dev = DevMem::open().expect("failed to open /dev/mem");
    let (ptr, offset, map_len) = dev.mmap(addr, len).expect("mmap failed");
    let virt = unsafe { ptr.add(offset) };

    let mut buf = vec![0u8; len];
    for i in 0..len {
        buf[i] = unsafe { ptr::read_volatile(virt.add(i)) };
    }

    fs::write(file, &buf).expect("failed to write file");
    println!(
        "{}dumped{} {} bytes from {}{:08x}{} to {}{}{}",
        color::GREEN,
        color::RESET,
        len,
        color::CYAN,
        addr,
        color::RESET,
        color::BOLD,
        file,
        color::RESET,
    );
    dev.munmap(ptr, map_len);
}

fn cmd_load(addr: usize, file: &str) {
    let data = fs::read(file).expect("failed to read file");
    let len = data.len();

    let dev = DevMem::open().expect("failed to open /dev/mem");
    let (ptr, offset, map_len) = dev.mmap(addr, len).expect("mmap failed");
    let virt = unsafe { ptr.add(offset) };

    for (i, &b) in data.iter().enumerate() {
        unsafe { ptr::write_volatile(virt.add(i), b) };
    }

    println!(
        "{}loaded{} {} bytes from {}{}{} to {}{:08x}{}",
        color::GREEN,
        color::RESET,
        len,
        color::BOLD,
        file,
        color::RESET,
        color::CYAN,
        addr,
        color::RESET,
    );
    dev.munmap(ptr, map_len);
}

fn cmd_bit(addr: usize, bit: u8, value: Option<u8>) {
    assert!(bit < 32, "bit must be 0-31");
    assert!(addr % 4 == 0, "address not aligned for 32-bit access");

    let dev = DevMem::open().expect("failed to open /dev/mem");
    let (ptr, offset, map_len) = dev.mmap(addr, 4).expect("mmap failed");
    let virt = unsafe { ptr.add(offset) };

    let val = if let Some(value) = value {
        assert!(value <= 1, "value must be 0 or 1");
        let mut val = unsafe { ptr::read_volatile(virt as *const u32) };
        if value == 1 {
            val |= 1 << bit;
        } else {
            val &= !(1 << bit);
        }
        unsafe { ptr::write_volatile(virt as *mut u32, val) };
        val
    } else {
        unsafe { ptr::read_volatile(virt as *const u32) }
    };

    let bit_val = (val >> bit) & 1;
    let bit_color = if bit_val == 1 {
        color::YELLOW
    } else {
        color::DIM
    };
    println!(
        "{}{:08x}{}[{}{}{}] = {}{}{}{} (word = 0x{}{:08x}{})",
        color::CYAN,
        addr,
        color::RESET,
        color::BOLD,
        bit,
        color::RESET,
        color::BOLD,
        bit_color,
        bit_val,
        color::RESET,
        color::BOLD,
        val,
        color::RESET,
    );
    print_binary_u32(val);

    dev.munmap(ptr, map_len);
}

// ---------------------------------------------------------------------------
// Main
// ---------------------------------------------------------------------------

fn main() {
    let cli = Cli::parse();
    match cli.cmd {
        Cmd::Read { addr, len } => {
            cmd_read(parse_addr(&addr), parse_num(&len));
        }
        Cmd::Write { addr, values } => {
            let vals: Vec<u64> = values.iter().map(|v| parse_value(v)).collect();
            cmd_write(parse_addr(&addr), &vals);
        }
        Cmd::Dump { addr, len, file } => {
            cmd_dump(parse_addr(&addr), parse_num(&len), &file);
        }
        Cmd::Load { addr, file } => {
            cmd_load(parse_addr(&addr), &file);
        }
        Cmd::Bit { addr, bit, value } => {
            cmd_bit(parse_addr(&addr), bit, value);
        }
    }
}
