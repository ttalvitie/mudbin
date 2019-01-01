use crate::prelude::*;
use crate::util::process::ChildExt;

use std::os::unix::fs::symlink;
use std::path::Path;
use std::process::Command;

use tempfile::{tempdir, TempDir};

use tokio_process::{Child, CommandExt};

pub struct Qemu {
    process: Child,
    workdir: TempDir,
}

impl Qemu {
    pub fn wait(self) -> BoxFuture<()> {
        let workdir = self.workdir;
        self.process
            .wait_success()
            .chain_err(|| "Error in child QEMU process")
            .and_then(move |()| {
                future::result(
                    workdir
                        .close()
                        .chain_err(|| "Closing temporary directory for QEMU failed"),
                )
            })
            .into_box()
    }
}

pub struct QemuConfig {
    command: Command,
    workdir: TempDir,
    next_binding_idx: u64,
}

impl QemuConfig {
    pub fn new() -> Result<QemuConfig> {
        let workdir = tempdir().chain_err(|| "Creating temporary directory for QEMU failed")?;

        let mut command = Command::new("qemu-system-x86_64");
        command.current_dir(workdir.path());
        command.arg("-nodefaults");
        command.arg("-accel").arg("kvm");
        command.arg("-vga").arg("cirrus");
        command.arg("-m").arg("1024");
        command.arg("-device").arg("virtio-serial");

        Ok(QemuConfig {
            command,
            workdir,
            next_binding_idx: 0,
        })
    }

    fn alloc_binding(&mut self) -> String {
        let binding_idx = self.next_binding_idx;
        self.next_binding_idx += 1;
        format!("binding{}", binding_idx)
    }

    fn bind_file<P: AsRef<Path>>(&mut self, src: P) -> Result<String> {
        let src = src.as_ref();
        if !src.is_file() {
            return Err("QEMU binding source file is not a regular file".into());
        }
        let src = src
            .canonicalize()
            .chain_err(|| "Could not canonicalize QEMU binding source file path")?;
        let binding = self.alloc_binding();
        symlink(src, self.workdir.path().join(&binding))
            .chain_err(|| "Could not create symlink for QEMU file binding")?;
        Ok(binding)
    }

    pub fn boot_kernel<P: AsRef<Path>, Q: AsRef<Path>>(
        mut self,
        kernel: P,
        initrd: Q,
    ) -> Result<Self> {
        let kernel_binding = self.bind_file(kernel)?;
        let initrd_binding = self.bind_file(initrd)?;
        self.command.arg("-kernel").arg(kernel_binding);
        self.command.arg("-initrd").arg(initrd_binding);
        Ok(self)
    }

    pub fn spawn(mut self) -> Result<Qemu> {
        let process = self
            .command
            .spawn_async()
            .chain_err(|| "Could not spawn child QEMU process")?;
        Ok(Qemu {
            process,
            workdir: self.workdir,
        })
    }
}
