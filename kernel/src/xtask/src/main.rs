//! Build orchestration for rustboot.
//!
//! Usage:
//!   cargo xtask build <target>
//!   cargo xtask build all
//!   cargo xtask clean
//!   cargo xtask flash <target>
//!
//! Targets: pi-zero2, pi5, kickpi-k2b, rp2040

use std::env;
use std::path::PathBuf;
use std::process::{exit, Command};

const TARGETS: &[(&str, &str, &str)] = &[
    // (name, rust target, binary name)
    ("pi-zero2", "aarch64-unknown-none", "kernel8"),
    ("pi5", "aarch64-unknown-none", "kernel8"),
    ("kickpi-k2b", "aarch64-unknown-none", "boot0"),
    ("rp2040", "thumbv6m-none-eabi", "bootloader"),
];

fn main() {
    let args: Vec<String> = env::args().collect();

    if args.len() < 2 {
        print_usage();
        exit(1);
    }

    match args[1].as_str() {
        "build" => {
            if args.len() < 3 {
                println!("Error: specify target or 'all'");
                print_usage();
                exit(1);
            }
            if args[2] == "all" {
                for (name, _, _) in TARGETS {
                    build_target(name);
                }
            } else {
                build_target(&args[2]);
            }
        }
        "clean" => clean(),
        "flash" => {
            if args.len() < 3 {
                println!("Error: specify target");
                print_usage();
                exit(1);
            }
            flash_target(&args[2]);
        }
        "objdump" => {
            if args.len() < 3 {
                println!("Error: specify target");
                print_usage();
                exit(1);
            }
            objdump_target(&args[2]);
        }
        _ => {
            println!("Unknown command: {}", args[1]);
            print_usage();
            exit(1);
        }
    }
}

fn print_usage() {
    println!("rustboot build system");
    println!();
    println!("Usage:");
    println!("  cargo xtask build <target>   Build specific target");
    println!("  cargo xtask build all        Build all targets");
    println!("  cargo xtask clean            Clean build artifacts");
    println!("  cargo xtask flash <target>   Flash target to device");
    println!("  cargo xtask objdump <target> Disassemble target binary");
    println!();
    println!("Targets:");
    for (name, arch, _) in TARGETS {
        println!("  {:<15} ({})", name, arch);
    }
}

fn project_root() -> PathBuf {
    let manifest_dir = env!("CARGO_MANIFEST_DIR");
    PathBuf::from(manifest_dir).parent().unwrap().to_path_buf()
}

fn find_target(name: &str) -> Option<(&'static str, &'static str, &'static str)> {
    TARGETS.iter().find(|(n, _, _)| *n == name).copied()
}

fn build_target(name: &str) {
    let Some((_, rust_target, bin_name)) = find_target(name) else {
        println!("Unknown target: {}", name);
        println!("Available: {:?}", TARGETS.iter().map(|t| t.0).collect::<Vec<_>>());
        exit(1);
    };

    println!("Building {} for {}...", name, rust_target);

    let root = project_root();
    let platform_dir = root.join("platform").join(name);
    let linker_script = platform_dir.join("linker.ld");

    // Build with cargo
    let status = Command::new("cargo")
        .current_dir(&root)
        .args([
            "build",
            "--release",
            "--package",
            &format!("rustboot-{}", name),
            "--target",
            rust_target,
        ])
        .env(
            "RUSTFLAGS",
            format!("-C link-arg=-T{}", linker_script.display()),
        )
        .status()
        .expect("Failed to run cargo");

    if !status.success() {
        println!("Build failed!");
        exit(1);
    }

    // Convert to binary
    let elf_path = root
        .join("target")
        .join(rust_target)
        .join("release")
        .join(bin_name);

    let bin_path = root
        .join("target")
        .join(rust_target)
        .join("release")
        .join(format!("{}.bin", bin_name));

    println!("Converting ELF to binary...");

    // Try rust-objcopy first, fall back to llvm-objcopy
    let objcopy = find_objcopy();

    let status = Command::new(&objcopy)
        .args(["-O", "binary"])
        .arg(&elf_path)
        .arg(&bin_path)
        .status()
        .expect("Failed to run objcopy");

    if !status.success() {
        println!("objcopy failed!");
        exit(1);
    }

    // Print size info
    let metadata = std::fs::metadata(&bin_path).expect("Failed to read binary");
    println!();
    println!("Build complete!");
    println!("  ELF: {}", elf_path.display());
    println!("  BIN: {}", bin_path.display());
    println!("  Size: {} bytes ({:.1} KB)", metadata.len(), metadata.len() as f64 / 1024.0);
}

fn find_objcopy() -> String {
    // Try rust-objcopy from cargo-binutils
    if Command::new("rust-objcopy")
        .arg("--version")
        .output()
        .is_ok()
    {
        return "rust-objcopy".to_string();
    }

    // Try llvm-objcopy
    if Command::new("llvm-objcopy")
        .arg("--version")
        .output()
        .is_ok()
    {
        return "llvm-objcopy".to_string();
    }

    // Try aarch64 toolchain objcopy
    if Command::new("aarch64-none-elf-objcopy")
        .arg("--version")
        .output()
        .is_ok()
    {
        return "aarch64-none-elf-objcopy".to_string();
    }

    println!("Warning: Could not find objcopy. Install cargo-binutils:");
    println!("  cargo install cargo-binutils");
    println!("  rustup component add llvm-tools");

    "rust-objcopy".to_string()
}

fn clean() {
    println!("Cleaning build artifacts...");
    let root = project_root();

    let status = Command::new("cargo")
        .current_dir(&root)
        .args(["clean"])
        .status()
        .expect("Failed to run cargo clean");

    if !status.success() {
        println!("Clean failed!");
        exit(1);
    }

    println!("Clean complete!");
}

fn flash_target(name: &str) {
    let Some((_, rust_target, bin_name)) = find_target(name) else {
        println!("Unknown target: {}", name);
        exit(1);
    };

    let root = project_root();
    let bin_path = root
        .join("target")
        .join(rust_target)
        .join("release")
        .join(format!("{}.bin", bin_name));

    if !bin_path.exists() {
        println!("Binary not found. Run 'cargo xtask build {}' first.", name);
        exit(1);
    }

    println!("Flashing {}...", name);
    println!("Binary: {}", bin_path.display());

    match name {
        "pi-zero2" | "pi5" => {
            println!();
            println!("For Raspberry Pi:");
            println!("1. Mount the SD card boot partition");
            println!("2. Copy {}.bin as kernel8.img", bin_name);
            println!("3. Ensure config.txt has arm_64bit=1");
            println!();
            println!("Example:");
            println!("  cp {} /media/boot/kernel8.img", bin_path.display());
        }
        "kickpi-k2b" => {
            println!();
            println!("For KickPi K2B (Allwinner H618):");
            println!("Option 1 - SD card:");
            println!("  sudo dd if={} of=/dev/sdX bs=1024 seek=8", bin_path.display());
            println!();
            println!("Option 2 - FEL mode (USB boot):");
            println!("  sunxi-fel spl {}", bin_path.display());
        }
        "rp2040" => {
            println!();
            println!("For RP2040:");
            println!("1. Hold BOOTSEL and plug in USB");
            println!("2. Copy UF2 file to the mounted drive");
            println!();
            println!("Note: You need to convert to UF2 format first.");
            println!("Consider using elf2uf2-rs or picotool.");
        }
        _ => {
            println!("Flash instructions not available for {}", name);
        }
    }
}

fn objdump_target(name: &str) {
    let Some((_, rust_target, bin_name)) = find_target(name) else {
        println!("Unknown target: {}", name);
        exit(1);
    };

    let root = project_root();
    let elf_path = root
        .join("target")
        .join(rust_target)
        .join("release")
        .join(bin_name);

    if !elf_path.exists() {
        println!("ELF not found. Run 'cargo xtask build {}' first.", name);
        exit(1);
    }

    // Try rust-objdump
    let objdump = if Command::new("rust-objdump")
        .arg("--version")
        .output()
        .is_ok()
    {
        "rust-objdump"
    } else {
        "llvm-objdump"
    };

    let _ = Command::new(objdump)
        .args(["-d", "--no-show-raw-insn"])
        .arg(&elf_path)
        .status()
        .expect("Failed to run objdump");
}
