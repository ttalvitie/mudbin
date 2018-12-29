use crate::prelude::*;

use std::os::unix::process::ExitStatusExt;
use std::process::ExitStatus;

use tokio_process::Child;

fn check_success(status: ExitStatus) -> Result<()> {
    if status.success() {
        Ok(())
    } else {
        match status.code() {
            Some(code) => Err(format!("Child process returned failure exit status {}", code).into()),
            None => match status.signal() {
                Some(signal) => Err(format!("Child process terminated by signal {}", signal).into()),
                None => Err("Child process terminated unsuccessfully for unknown reason".into()),
            },
        }
    }
}

pub trait ChildExt {
    fn wait_success(self) -> BoxFuture<()>;
}

impl ChildExt for Child {
    fn wait_success(self) -> BoxFuture<()> {
        self.chain_err(|| "Error while waiting for child process to finish")
            .and_then(|status| future::result(check_success(status)))
            .into_box()
    }
}
