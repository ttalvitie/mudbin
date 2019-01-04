use crate::prelude::*;

use crate::qemu::QemuConfig;

use std::path::Path;

pub fn create_image(_output_path: &Path) -> BoxFuture<()> {
    let preseed = Vec::from(
        "d-i debian-installer/locale select en_US.UTF-8
         d-i console-setup/ask_detect boolean false
         d-i keyboard-configuration/layout select us
         d-i keyboard-configuration/variant select us
         d-i mirror/country string manual
         d-i mirror/http/hostname string archive.ubuntu.com
         d-i mirror/http/directory string /ubuntu
         d-i mirror/http/proxy string
         d-i passwd/user-fullname string user
         d-i passwd/username string user
         d-i passwd/user-password password insecure
         d-i passwd/user-password-again password insecure
         d-i clock-setup/utc boolean true
         d-i time/zone string UTC
         ",
    );

    QemuConfig::new()
        .boot_kernel(
            "../../preseedtest/linux",
            "../../preseedtest/initrd.gz",
            "auto=true url=tftp://10.0.2.2/file hostname=mudbin domain=mudbin",
        )
        .network(Some(preseed))
        .spawn()
        .and_then(|(qemu, _)| qemu.wait())
        .map(|_| ())
        .into_box()
}
