// SPDX-License-Identifier: GPL-2.0

//! Minimal out-of-tree Rust kernel module smoke test.
//!
//! This does not bind WMI. Its job is to prove that the target Ubuntu kernel
//! headers include enough Rust support to compile and link an external module.

use kernel::prelude::*;

module! {
    type: CorsairWmiRustProbe,
    name: "corsair_wmi_rust_probe",
    authors: ["Local driver prototype"],
    description: "CORSAIR WMI Rust kernel build smoke test",
    license: "GPL",
}

struct CorsairWmiRustProbe;

impl kernel::Module for CorsairWmiRustProbe {
    fn init(_module: &'static ThisModule) -> Result<Self> {
        pr_info!("corsair_wmi_rust_probe: loaded\n");
        Ok(Self)
    }
}

impl Drop for CorsairWmiRustProbe {
    fn drop(&mut self) {
        pr_info!("corsair_wmi_rust_probe: unloaded\n");
    }
}
