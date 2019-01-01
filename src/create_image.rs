use crate::prelude::*;

use crate::qemu::QemuConfig;

use std::path::Path;

use log::warn;

pub fn create_image(_output_path: &Path) -> BoxFuture<()> {
    warn!("Image creation not implemented");
    future::result(
        QemuConfig::new()
            .and_then(|x| x.boot_kernel("../../preseedtest/linux", "../../preseedtest/initrd.gz")),
    )
    .and_then(|x| x.spawn())
    .and_then(|x| x.wait())
    .into_box()
}
