use crate::prelude::*;

use crate::qemu::{create_disk_image, shrink_disk_image, QemuConfig};

use std::path::Path;

use tempfile::TempDir;

use tokio::codec::{FramedRead, LinesCodec};
use tokio::spawn;

use log::{debug, info};

pub fn create_image<P: AsRef<Path>>(output_path: P) -> BoxFuture<()> {
    let output_path = output_path.as_ref().to_path_buf();

    let mut preseed = String::new();
    preseed.push_str("d-i preseed/early_command string tail -n0 -f /var/log/syslog > /dev/virtio-ports/mudbin.vsport.log &\n");
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
    preseed.push_str("d-i partman-auto/disk string /dev/vda\n");
    preseed.push_str("d-i partman-auto/method string regular\n");
    preseed.push_str("d-i partman-auto/choose_recipe select atomic\n");
    preseed.push_str("d-i partman/default_filesystem string ext4\n");
    preseed.push_str("d-i partman-partitioning/confirm_write_new_label boolean true\n");
    preseed.push_str("d-i partman/choose_partition select finish\n");
    preseed.push_str("d-i partman/confirm boolean true\n");
    preseed.push_str("d-i partman/confirm_nooverwrite boolean true\n");
    preseed.push_str("d-i partman-swapfile/size string 0\n");
    preseed.push_str("d-i base-installer/kernel/image string linux-virtual\n");
    preseed.push_str("tasksel tasksel/first multiselect ");
    preseed.push_str("d-i pkgsel/upgrade select safe-upgrade\n");
    preseed.push_str("d-i pkgsel/update-policy select none\n");
    preseed.push_str("d-i grub-installer/only_debian boolean true\n");
    preseed.push_str("d-i grub-installer/with_other_os boolean true\n");
    preseed.push_str("d-i grub-installer/bootdev string /dev/vda\n");
    preseed.push_str("d-i preseed/late_command string in-target apt-get clean ; in-target fstrim -a\n");
    preseed.push_str("d-i finish-install/reboot_in_progress note\n");
    preseed.push_str("d-i debian-installer/exit/poweroff boolean true\n");
    let preseed = Vec::from(preseed);

    future::result(TempDir::new())
        .chain_err(|| "Creating temporary directory for disk image failed")
        .and_then(move |tmp_dir| {
            info!("Creating disk image");
            create_disk_image(tmp_dir.path().join("image")).map(move |()| tmp_dir)
        })
        .and_then(|tmp_dir| {
            info!("Starting installer in QEMU");
            QemuConfig::new()
                .boot_kernel(
                    "../../preseedtest/linux",
                    "../../preseedtest/initrd.gz",
                    "auto=true url=tftp://10.0.2.2/file hostname=mudbin domain=mudbin",
                )
                .vsport("log")
                .unrestricted_net()
                .tftp_file(preseed)
                .drive(tmp_dir.path().join("image"), true)
                .spawn()
                .map(move |(qemu, vsports)| (qemu, vsports, tmp_dir))
        })
        .and_then(|(qemu, mut vsports, tmp_dir)| {
            let log_port = FramedRead::new(vsports.remove("log").unwrap(), LinesCodec::new());
            spawn(
                log_port
                    .for_each(|line| {
                        debug!("installer log: {}", line);
                        future::result(Ok(()))
                    })
                    .map_err(|_| ()),
            );
            qemu.wait().map(move |()| tmp_dir)
        })
        .and_then(move |tmp_dir| {
            info!("Shrinking disk image");
            shrink_disk_image(tmp_dir.path().join("image"), output_path)
                .map(move |()| drop(tmp_dir))
        })
        .map(|()| {
            info!("Installation complete");
        })
        .into_box()
}
