use crate::prelude::*;

use crate::qemu::{create_disk_image, QemuConfig};

use std::path::Path;

use tokio::codec::{FramedRead, LinesCodec};
use tokio::spawn;

use log::debug;

pub fn create_image<P: AsRef<Path>>(output_path: P) -> BoxFuture<()> {
    let output_path = output_path.as_ref().to_path_buf();

    let mut preseed = String::new();
    preseed.push_str("d-i debian-installer/locale select en_US.UTF-8\n");
    preseed.push_str("d-i console-setup/ask_detect boolean false\n");
    preseed.push_str("d-i keyboard-configuration/layout select us\n");
    preseed.push_str("d-i keyboard-configuration/variant select us\n");
    preseed.push_str("d-i mirror/country string manual\n");
    preseed.push_str("d-i mirror/http/hostname string archive.ubuntu.com\n");
    preseed.push_str("d-i mirror/http/directory string /ubuntu\n");
    preseed.push_str("d-i mirror/http/proxy string\n");
    preseed.push_str("d-i passwd/user-fullname string user\n");
    preseed.push_str("d-i passwd/username string user\n");
    preseed.push_str("d-i passwd/user-password password insecure\n");
    preseed.push_str("d-i passwd/user-password-again password insecure\n");
    preseed.push_str("d-i clock-setup/utc boolean true\n");
    preseed.push_str("d-i time/zone string UTC\n");
    preseed.push_str("d-i preseed/early_command string tail -n0 -f /var/log/syslog > /dev/virtio-ports/mudbin.vsport.log &\n");
    let preseed = Vec::from(preseed);

    create_disk_image(&output_path)
        .and_then(move |()| {
            QemuConfig::new()
                .boot_kernel(
                    "../../preseedtest/linux",
                    "../../preseedtest/initrd.gz",
                    "auto=true url=tftp://10.0.2.2/file hostname=mudbin domain=mudbin",
                )
                .vsport("log")
                .unrestricted_net()
                .tftp_file(preseed)
                .drive(output_path, true)
                .spawn()
        })
        .and_then(|(qemu, mut vsports)| {
            let log_port = FramedRead::new(vsports.remove("log").unwrap(), LinesCodec::new());
            spawn(
                log_port
                    .for_each(|line| {
                        debug!("installer log: {}", line);
                        future::result(Ok(()))
                    })
                    .map_err(|_| ()),
            );
            qemu.wait()
        })
        .map(|_| ())
        .into_box()
}
