use crate::prelude::*;
use crate::util::process::ChildExt;

use std::collections::{HashMap, HashSet};
use std::fs::{create_dir, File};
use std::io::Write;
use std::os::unix::fs::symlink;
use std::path::{Path, PathBuf};
use std::process::Command;

use tempfile::{tempdir, TempDir};

use tokio::net::{UnixListener, UnixStream};

use tokio_process::CommandExt;

pub struct Qemu {
    wait_child: BoxFuture<()>,
}

impl Qemu {
    pub fn wait(self) -> BoxFuture<()> {
        self.wait_child
    }
}

pub fn create_disk_image<P: AsRef<Path>>(path: P) -> BoxFuture<()> {
    let child_fut = future::result(
        Command::new("qemu-img")
            .arg("create")
            .arg("-f")
            .arg("qcow2")
            .arg("--")
            .arg(path.as_ref())
            .arg("1T")
            .spawn_async()
            .chain_err(|| "Spawning qemu-img child process to create disk image failed"),
    );
    child_fut
        .and_then(|child| child.wait_success())
        .chain_err(|| "Creating disk image with qemu-img failed")
        .into_box()
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

struct NetConfig {
    restrict: bool,
    tftp_file: Option<Vec<u8>>,
}

impl NetConfig {
    fn apply(&self, command: &mut Command, builder: &mut TempDirBuilder) -> Result<()> {
        if !self.restrict || self.tftp_file.is_some() {
            let restrict_arg = format!(",restrict={}", if self.restrict { "y" } else { "n" });

            let mut tftp_arg = "";
            if let &Some(ref tftp_file) = &self.tftp_file {
                tftp_arg = ",tftp=tftp";

                let tftp_dir = builder.workdir.path().join("tftp");
                create_dir(&tftp_dir).chain_err(|| {
                    "Could not create directory 'tftp' in QEMU temporary directory"
                })?;

                File::create(tftp_dir.join("file"))
                    .and_then(|mut file| file.write_all(tftp_file).map(move |()| file))
                    .and_then(|file| file.sync_all())
                    .chain_err(|| "Could not write file 'tftp/file' in QEMU temporary directory")?;
            }

            command.arg("-net").arg("nic,model=virtio");
            command
                .arg("-net")
                .arg(format!("user{}{}", restrict_arg, tftp_arg));
        }
        Ok(())
    }
}

pub struct QemuConfig {
    kernel: Option<(PathBuf, PathBuf, String)>,
    vsports: HashSet<String>,
    net: NetConfig,
    drives: Vec<(PathBuf, bool)>,
}

impl QemuConfig {
    pub fn new() -> QemuConfig {
        QemuConfig {
            kernel: None,
            vsports: HashSet::new(),
            net: NetConfig {
                restrict: true,
                tftp_file: None,
            },
            drives: Vec::new(),
        }
    }

    pub fn boot_kernel<P: AsRef<Path>, Q: AsRef<Path>>(
        &mut self,
        kernel_path: P,
        initrd_path: Q,
        append: &str,
    ) -> &mut QemuConfig {
        let kernel_path = kernel_path.as_ref().to_path_buf();
        let initrd_path = initrd_path.as_ref().to_path_buf();
        let append = append.to_string();
        self.kernel = Some((kernel_path, initrd_path, append));
        self
    }

    pub fn vsport(&mut self, name: &str) -> &mut QemuConfig {
        let allowed_char = |x: char| x.is_ascii_alphanumeric() || x == '_' || x == '-' || x == '.';
        if name.len() == 0 || name.len() > 64 || !name.chars().all(allowed_char) {
            panic!("Invalid virtual serial port name (must be 1-64 characters, allowed characters: ASCII-alphanumeric and '_-.')");
        }
        self.vsports.insert(name.to_string());
        self
    }

    pub fn unrestricted_net(&mut self) -> &mut QemuConfig {
        self.net.restrict = false;
        self
    }

    pub fn tftp_file(&mut self, tftp_file: Vec<u8>) -> &mut QemuConfig {
        self.net.tftp_file = Some(tftp_file);
        self
    }

    pub fn drive<P: AsRef<Path>>(&mut self, path: P, allow_write: bool) -> &mut QemuConfig {
        self.drives.push((path.as_ref().to_path_buf(), allow_write));
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

        if let &Some((ref kernel_path, ref initrd_path, ref append)) = &self.kernel {
            let kernel_link = builder
                .link_path(kernel_path)
                .chain_err(|| "Could not link kernel image to QEMU")?;
            let initrd_link = builder
                .link_path(initrd_path)
                .chain_err(|| "Could not link initrd image to QEMU")?;
            command.arg("-kernel").arg(kernel_link);
            command.arg("-initrd").arg(initrd_link);
            command.arg("-append").arg(append);
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

        self.net.apply(&mut command, &mut builder)?;

        for &(ref path, allow_write) in &self.drives {
            let link = builder
                .link_path(path)
                .chain_err(|| "Could not link drive path to QEMU")?;

            let mut readonly_arg = ",read-only";
            if allow_write {
                readonly_arg = "";
            }
            command.arg("-drive");
            command.arg(format!("file={},if=virtio{}", link, readonly_arg));
        }

        let workdir = builder.workdir;
        let wait_child = command
            .spawn_async()
            .chain_err(|| "Could not spawn child QEMU process")?
            .wait_success()
            .chain_err(|| "Error in child QEMU process")
            .and_then(move |()| {
                future::result(
                    workdir
                        .close()
                        .chain_err(|| "Closing temporary directory for QEMU failed"),
                )
            })
            .into_box();

        let init = future::join_all(vsports).map(move |vsport_pairs| {
            let mut vsports = HashMap::new();
            for (name, stream) in vsport_pairs {
                vsports.insert(name, stream);
            }
            vsports
        });

        let fut = init
            .select2(wait_child)
            .map_err(|x| x.split().0)
            .and_then(|x| match x {
                A((vsports, wait_child)) => future::ok((Qemu { wait_child }, vsports)),
                B(_) => future::err("QEMU child process exited during initialization".into()),
            })
            .into_box();
        Ok(fut)
    }

    pub fn spawn(&self) -> BoxFuture<(Qemu, HashMap<String, UnixStream>)> {
        future::result(self.spawn_impl()).flatten().into_box()
    }
}
