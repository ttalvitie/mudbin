use crate::prelude::*;

use crate::qemu::QemuConfig;

use std::path::Path;

use log::warn;

use tokio::io::write_all;

pub fn create_image(_output_path: &Path) -> BoxFuture<()> {
    warn!("Image creation not implemented");
    QemuConfig::new()
        .boot_kernel("../../preseedtest/linux", "../../preseedtest/initrd.gz")
        .vsport("preseed")
        .spawn()
        .and_then(|(qemu, mut vsports)| {
            let preseed_stream = vsports.remove("preseed").unwrap();
            let mut preseed_buf = "PRESEED FILE\n".to_string();
            for _ in 0..10000 {
                preseed_buf.push_str("THIS IS A PLACEHOLDER\n");
            }
            qemu.wait().join(write_all(preseed_stream, preseed_buf).chain_err(|| "Writing to preseed virtual serial port failed"))
        })
        .map(|_| ())
        .into_box()
}
