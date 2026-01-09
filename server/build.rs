//! Build script for netpoke-server
//!
//! This script ensures the server is rebuilt when any static files change,
//! including the WASM client files built by wasm-pack.

fn main() {
    // Tell Cargo to rerun this build script if any file in the static directory changes
    println!("cargo:rerun-if-changed=static");
}
