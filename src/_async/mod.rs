pub mod dir;
pub mod file;
pub mod recover;
use async_trait::async_trait;
use std::ffi::OsStr;
use std::fs::{Metadata, Permissions};
use std::path::Path;

use tokio::{
    fs::{self, remove_dir_all, remove_file},
    io::Result,
};

use crate::push_file_name;

use self::dir::AsyncDirectoryInfo;
use self::file::AsyncFileInfo;
use self::recover::TryRecoverResult;
#[async_trait]
pub trait AsyncInfo: Sized + Send + Sync {
    fn as_path(&self) -> &Path;
    fn file_name(&self) -> Option<&OsStr> {
        self.as_path().file_name()
    }
    async fn metadata(&self) -> Result<Metadata>;
    async fn size(&self) -> u64;
    /// Return None if the path is a root directory
    async fn parent(&self) -> Option<AsyncDirectoryInfo> {
        let parent = self.as_path().parent()?;

        AsyncDirectoryInfo::open(parent).await.ok()
    }

    async fn permissions(&self) -> Result<Permissions> {
        self.metadata().await.map(|data| data.permissions())
    }

    async fn read_only(&self) -> Result<bool> {
        self.metadata()
            .await
            .map(|data| data.permissions().readonly())
    }
}
#[async_trait]
pub trait AsyncAction: AsyncInfo {
    async fn open<P: AsRef<Path> + Send + Sync>(path: P) -> Result<Self>;

    /// # Safety
    /// This function is unsafe as it does not check or fix the path.
    /// please make sure the path is correct absolute path 
    /// 
    /// # Example
    unsafe fn open_uncheck<P: AsRef<Path>>(path: P) -> Self;
    /// Rename a file or directory
    async fn rename<T: AsRef<OsStr> + Send + Sync>(&mut self, name: T) -> Result<()>;
    async fn set_readonly(&self, readonly: bool) -> Result<()> {
        let mut perm = self.metadata().await?.permissions();
        perm.set_readonly(readonly);
        self.set_permissions(perm).await
    }
    async fn set_permissions(&self, perm: Permissions) -> Result<()> {
        fs::set_permissions(self.as_path(), perm).await
    }
    async fn delete(self) -> Result<()> {
        if self.read_only().await? {
            self.set_readonly(false).await?;
        }
        if self.as_path().is_dir() {
            remove_dir_all(self.as_path()).await
        } else {
            remove_file(self.as_path()).await
        }
    }
    async fn copy_to<P: AsRef<Path> + Send + Sync>(&self, path: P) -> TryRecoverResult<()> {
        let path = push_file_name(self.file_name(), path)?;
        self.copy_new(path).await
    }
    async fn copy_new<P: AsRef<Path> + Send + Sync>(&self, path: P) -> TryRecoverResult<()>;
    async fn move_to<P: AsRef<Path> + Send + Sync>(&mut self, path: P) -> TryRecoverResult<()> {
        let path = push_file_name(self.file_name(), path)?;
        self.move_new(path).await
    }
    async fn move_new<P: AsRef<Path> + Send + Sync>(&mut self, path: P) -> TryRecoverResult<()>;
}

async fn remove_file_any(path: &Path) -> Result<()> {
    let f = unsafe { AsyncFileInfo::open_uncheck(path) };
    f.delete().await
}
