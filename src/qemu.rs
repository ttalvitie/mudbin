use crate::prelude::*;
use crate::util::process::ChildExt;

use std::collections::{HashMap, HashSet};
use std::os::unix::fs::symlink;
use std::path::{Path, PathBuf};
use std::process::Command;

use tempfile::{tempdir, TempDir};

use tokio::net::{UnixListener, UnixStream};

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

struct TempDirBuilder {
    workdir: TempDir,
    next_link_idx: u64,
    next_socket_idx: u64,
}

impl TempDirBuilder {
    fn new() -> Result<TempDirBuilder> {
        let workdir = tempdir().chain_err(|| "Creating temporary directory for QEMU failed")?;
        Ok(TempDirBuilder {
            workdir,
            next_link_idx: 0,
            next_socket_idx: 0,
        })
    }

    fn link_path<P: AsRef<Path>>(&mut self, path: P) -> Result<String> {
        let name = format!("link{}", self.next_link_idx);
        self.next_link_idx += 1;

        let path = path
            .as_ref()
            .canonicalize()
            .chain_err(|| "Could not canonicalize path")?;
        if !path.exists() {
            return Err("Path does not exist".into());
        }

        symlink(path, self.workdir.path().join(&name)).chain_err(|| "Could not create symlink")?;
        Ok(name)
    }

    fn create_socket(&mut self) -> Result<(String, UnixListener)> {
        let name = format!("sock{}", self.next_socket_idx);
        self.next_socket_idx += 1;

        let listener = UnixListener::bind(self.workdir.path().join(&name))
            .chain_err(|| "Could not bind to UNIX socket")?;
        Ok((name, listener))
    }
}

pub struct QemuConfig {
    kernel: Option<(PathBuf, PathBuf)>,
    vsports: HashSet<String>,
}

impl QemuConfig {
    pub fn new() -> QemuConfig {
        QemuConfig {
            kernel: None,
            vsports: HashSet::new(),
        }
    }

    pub fn boot_kernel<P: AsRef<Path>, Q: AsRef<Path>>(
        &mut self,
        kernel_path: P,
        initrd_path: Q,
    ) -> &mut QemuConfig {
        let kernel_path = kernel_path.as_ref().to_path_buf();
        let initrd_path = initrd_path.as_ref().to_path_buf();
        self.kernel = Some((kernel_path, initrd_path));
        self
    }

    pub fn vsport(&mut self, name: &str) -> &mut QemuConfig {
        let allowed_char = |x: char| x.is_ascii_alphanumeric() || x == '_' || x == '-' || x == '.';
        if name.len() == 0 || name.len() > 64 || !name.chars().all(allowed_char) {
            panic!("Invalid virtual serial port name (must be 1-64 characters, allowed characters: ASCII-alphanumeric and '_-.')");
        }
        if !self.vsports.insert(name.to_string()) {
            panic!("Same virtual serial port name specified twice");
        }
        self
    }

    fn spawn_impl(&self) -> Result<BoxFuture<(Qemu, HashMap<String, UnixStream>)>> {
        let mut builder = TempDirBuilder::new()?;

        let mut command = Command::new("qemu-system-x86_64");
        command.current_dir(builder.workdir.path());
        command.arg("-nodefaults");
        command.arg("-accel").arg("kvm");
        command.arg("-vga").arg("cirrus");
        command.arg("-m").arg("1024");
        command.arg("-device").arg("virtio-serial");

        if let &Some((ref kernel_path, ref initrd_path)) = &self.kernel {
            let kernel_link = builder
                .link_path(kernel_path)
                .chain_err(|| "Could not link kernel image to QEMU")?;
            let initrd_link = builder
                .link_path(initrd_path)
                .chain_err(|| "Could not link initrd image to QEMU")?;
            command.arg("-kernel").arg(kernel_link);
            command.arg("-initrd").arg(initrd_link);
        }

        let mut vsports = Vec::new();
        for name in &self.vsports {
            let (sock, listener) = builder.create_socket()?;
            command.arg("-chardev");
            command.arg(format!("socket,id=mudbin.{},path={}", &sock, &sock));
            command.arg("-device");
            command.arg(format!(
                "virtserialport,chardev=mudbin.{},name=mudbin.vsport.{}",
                &sock, name
            ));

            let name = name.clone();
            let fut = listener
                .incoming()
                .into_future()
                .map_err(|(e, _)| e)
                .chain_err(|| "Error while accepting incoming connections")
                .and_then(|(stream, _)| {
                    future::result(stream.ok_or_else(|| "No incoming connections to accept".into()))
                })
                .chain_err(|| {
                    "Error while waiting for QEMU to connect to host socket for virtual serial port"
                })
                .map(move |stream| (name, stream));
            vsports.push(fut);
        }

        let process = command
            .spawn_async()
            .chain_err(|| "Could not spawn child QEMU process")?;

        let fut = future::join_all(vsports)
            .map(move |vsport_pairs| {
                let qemu = Qemu {
                    process,
                    workdir: builder.workdir,
                };
                let mut vsports = HashMap::new();
                for (name, stream) in vsport_pairs {
                    vsports.insert(name, stream);
                }
                (qemu, vsports)
            })
            .into_box();
        Ok(fut)
    }

    pub fn spawn(&self) -> BoxFuture<(Qemu, HashMap<String, UnixStream>)> {
        future::result(self.spawn_impl()).flatten().into_box()
    }
}
