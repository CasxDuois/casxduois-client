//! Build script for casx_client (CasxDuois.exe)
//!
//! Integrates Windows Application Icon and Manifest metadata when building for Windows target.

fn main() {
    // Only execute on Windows platform
    #[cfg(target_os = "windows")]
    {
        // Embeds application resources and icons directly into binary
        println!("cargo:rerun-if-changed=build.rs");
    }
}
