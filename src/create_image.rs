use crate::prelude::*;

use crate::qemu::QemuCommand;

use std::path::Path;

use log::warn;

pub fn create_image(_output_path: &Path) -> BoxFuture<()> {
    warn!("Image creation not implemented");
    future::result(QemuCommand::new().spawn())
        .and_then(|x| x.wait())
        .into_box()
}
