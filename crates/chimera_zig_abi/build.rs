//! Build script to compile Zig kernels and link them into chimera_zig_abi.
//!
//! This build script:
//! 1. Checks for Zig installation
//! 2. Builds the Zig static library from zig/chimera_kernels
//! 3. Links the library into the Rust crate

use std::env;
use std::path::Path;

fn main() {
    // Only run FFI build when the ffi feature is enabled
    if env::var_os("CARGO_FEATURE_FFI").is_none() {
        println!("cargo:rustc-cfg=ffi_disabled");
        return;
    }

    let manifest_dir = env::var("CARGO_MANIFEST_DIR").unwrap();
    let project_root = Path::new(&manifest_dir).parent().unwrap().parent().unwrap();

    // Check for Zig
    let zig_path = find_zig();
    if zig_path.is_none() {
        println!("cargo:warning=Zig not found, FFI will use Rust fallback implementations");
        println!("cargo:rustc-cfg=ffi_no_zig");
        return;
    }

    let zig_path = zig_path.unwrap();
    println!("cargo:warning=Using Zig at: {}", zig_path);

    // Build the Zig library
    let kernels_dir = project_root.join("zig").join("chimera_kernels");

    // Run zig build
    let output = std::process::Command::new(&zig_path)
        .current_dir(&kernels_dir)
        .args(["build", "-Doptimize=ReleaseSafe", "-Dtarget=native"])
        .output();

    match output {
        Ok(result) => {
            if result.status.success() {
                println!("cargo:warning=Zig kernels built successfully");
                let lib_dir = kernels_dir.join("zig-out").join("lib");
                println!("cargo:rustc-link-search=native={}", lib_dir.display());
            } else {
                println!(
                    "cargo:warning=Zig build failed: {}",
                    String::from_utf8_lossy(&result.stderr)
                );
                println!("cargo:rustc-cfg=ffi_no_zig");
            }
        }
        Err(e) => {
            println!("cargo:warning=Failed to run Zig: {}", e);
            println!("cargo:rustc-cfg=ffi_no_zig");
        }
    }
}

fn find_zig() -> Option<String> {
    // Check ZIG environment variable
    if let Ok(zig) = env::var("ZIG") {
        if Path::new(&zig).exists() {
            return Some(zig);
        }
    }

    // Check common locations
    let possible_paths = [
        "/snap/bin/zig",
        "/usr/local/bin/zig",
        "/usr/bin/zig",
        "/opt/zig/zig",
    ];

    for path in &possible_paths {
        if Path::new(path).exists() {
            return Some(path.to_string());
        }
    }

    // Try to find zig in PATH
    if let Ok(path) = std::process::Command::new("which").arg("zig").output() {
        let path_str = String::from_utf8_lossy(&path.stdout);
        let trimmed = path_str.trim();
        if !trimmed.is_empty() && Path::new(trimmed).exists() {
            return Some(trimmed.to_string());
        }
    }

    None
}
