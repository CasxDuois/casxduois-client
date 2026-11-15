//! Build script for casx_client (CasxDuois.exe)
//!
//! Tich hop Windows Application Icon va Manifest metadata khi build tren target Windows.

fn main() {
    // Chi thuc thi tren nen tang Windows
    #[cfg(target_os = "windows")]
    {
        // Khi co file resource .rc hoac .ico, winres se nhung truc tiep vao binary
        println!("cargo:rerun-if-changed=build.rs");
    }
}
