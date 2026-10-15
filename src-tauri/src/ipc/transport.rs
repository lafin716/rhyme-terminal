//! Platform-specific transport; framing and daemon messages stay identical.
#[cfg(windows)]
mod native {
    use anyhow::Result;
    use tokio::net::windows::named_pipe::{
        ClientOptions, NamedPipeClient, NamedPipeServer, ServerOptions,
    };
    pub type Client = NamedPipeClient;
    pub type Server = NamedPipeServer;
    pub async fn connect(name: &str) -> Result<Client> {
        Ok(ClientOptions::new().open(name)?)
    }
    pub struct Listener {
        name: String,
        next: NamedPipeServer,
    }
    impl Listener {
        pub async fn bind(name: &str) -> Result<Self> {
            let next = ServerOptions::new()
                .first_pipe_instance(true)
                .create(name)?;
            Ok(Self {
                name: name.into(),
                next,
            })
        }
        pub async fn accept(&mut self) -> Result<Server> {
            self.next.connect().await?;
            let next = ServerOptions::new().create(&self.name)?;
            Ok(std::mem::replace(&mut self.next, next))
        }
    }
}
#[cfg(unix)]
mod native {
    use anyhow::{bail, Context, Result};
    use std::{
        fs::{self, File, OpenOptions},
        os::unix::{
            fs::{DirBuilderExt, MetadataExt, OpenOptionsExt},
            io::AsRawFd,
        },
        path::{Path, PathBuf},
    };
    use tokio::net::{UnixListener, UnixStream};
    pub type Client = UnixStream;
    pub type Server = UnixStream;
    pub async fn connect(name: &str) -> Result<Client> {
        Ok(UnixStream::connect(name).await?)
    }
    pub struct Listener {
        inner: UnixListener,
        path: PathBuf,
        _lock: File,
    }
    impl Listener {
        pub async fn bind(name: &str) -> Result<Self> {
            let path = Path::new(name);
            let parent = path.parent().context("socket needs a parent directory")?;
            match fs::DirBuilder::new().mode(0o700).create(parent) {
                Ok(()) => (),
                Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => (),
                Err(e) => return Err(e.into()),
            }
            let metadata = fs::symlink_metadata(parent)?;
            // Never follow a directory owned by another user or expose the IPC.
            if !metadata.is_dir()
                || metadata.uid() != unsafe { libc::geteuid() }
                || metadata.mode() & 0o077 != 0
            {
                bail!("IPC directory must be owned by this user with mode 0700");
            }
            let lock = OpenOptions::new()
                .read(true)
                .write(true)
                .create(true)
                .truncate(false)
                .mode(0o600)
                .custom_flags(libc::O_NOFOLLOW)
                .open(parent.join("daemon.lock"))?;
            if unsafe { libc::flock(lock.as_raw_fd(), libc::LOCK_EX | libc::LOCK_NB) } != 0 {
                bail!("another daemon already owns this IPC endpoint");
            }
            // The lock serializes stale socket recovery across concurrent GUI launches.
            if let Ok(metadata) = fs::symlink_metadata(path) {
                use std::os::unix::fs::FileTypeExt;
                if !metadata.file_type().is_socket() {
                    bail!("IPC endpoint is not a socket");
                }
                if UnixStream::connect(path).await.is_ok() {
                    bail!("daemon already listening");
                }
                fs::remove_file(path)?;
            }
            Ok(Self {
                inner: UnixListener::bind(path)?,
                path: path.into(),
                _lock: lock,
            })
        }
        pub async fn accept(&mut self) -> Result<Server> {
            Ok(self.inner.accept().await?.0)
        }
    }
    impl Drop for Listener {
        fn drop(&mut self) {
            let _ = fs::remove_file(&self.path);
        }
    }
}
pub use native::*;

#[cfg(all(test, unix))]
mod tests {
    use super::*;
    #[tokio::test]
    async fn exclusive_listener_recovers_and_preserves_frames() {
        let root = std::path::PathBuf::from("/tmp")
            .join(format!("rhyme-ipc-test-{}", uuid::Uuid::new_v4()));
        let path = root.join("ipc.sock");
        let name = path.to_str().unwrap();
        let mut listener = Listener::bind(name).await.unwrap();
        assert!(Listener::bind(name).await.is_err());
        let mut client = connect(name).await.unwrap();
        let mut server = listener.accept().await.unwrap();
        crate::ipc::write_frame(&mut client, b"hello")
            .await
            .unwrap();
        assert_eq!(crate::ipc::read_frame(&mut server).await.unwrap(), b"hello");
        drop(listener);
        let listener = Listener::bind(name).await.unwrap();
        drop(listener);
        std::fs::remove_file(root.join("daemon.lock")).unwrap();
        std::fs::remove_dir(root).unwrap();
    }
}
