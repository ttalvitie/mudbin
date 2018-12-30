use crate::prelude::*;
use crate::util::process::ChildExt;

use std::process::Command;

use tokio_process::{Child, CommandExt};

pub struct Qemu {
    process: Child,
}

impl Qemu {
    pub fn wait(self) -> BoxFuture<()> {
        self.process
            .wait_success()
            .chain_err(|| "Error in child QEMU process")
            .into_box()
    }
}

pub struct QemuCommand;

impl QemuCommand {
    pub fn new() -> QemuCommand {
        QemuCommand
    }

    pub fn spawn(&self) -> Result<Qemu> {
        let mut command = Command::new("qemu-system-x86_64");
        command.arg("-accel").arg("kvm");
        let process = command
            .spawn_async()
            .chain_err(|| "Could not spawn child QEMU process")?;
        Ok(Qemu { process })
    }
}
