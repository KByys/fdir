pub mod dir;
pub mod file;
pub mod recover;
pub use self::{dir::DirectoryInfo, file::FileInfo};
use crate::push_file_name;
use std::{
    ffi::OsStr,
    fs::{self, metadata, remove_dir_all, remove_file, Metadata, Permissions},
    io::Result,
    path::Path,
};

use self::recover::TryRecoverResult;
pub trait Info: Sized {
    fn as_path(&self) -> &Path;
    fn file_name(&self) -> Option<&OsStr> {
        self.as_path().file_name()
    }
    fn metadata(&self) -> Result<Metadata> {
        metadata(self.as_path())
    }
    fn size(&self) -> u64;
    /// Return None if the path is a root directory
    fn parent(&self) -> Option<DirectoryInfo> {
        self.as_path()
            .parent()
            .and_then(|path| DirectoryInfo::open(path).ok())
    }

    fn permissions(&self) -> Result<Permissions> {
        self.metadata().map(|data| data.permissions())
    }

    fn read_only(&self) -> Result<bool> {
        self.metadata().map(|data| data.permissions().readonly())
    }
}

pub trait Action: Info {
    fn open<P: AsRef<Path>>(path: P) -> Result<Self>;
    /// # Safety
    /// This function is unsafe as it does not check or fix the path.
    /// please make sure the path is correct absolute path
    ///
    /// # Examples
    /// ```
    /// use fdir::*;
    /// let dir = unsafe { DirectoryInfo::open_uncheck(".") };
    /// assert_eq!(dir.to_string(), ".".to_string())
    /// ```
    unsafe fn open_uncheck<P: AsRef<Path>>(path: P) -> Self;

    /// Rename a file or directory
    fn rename<T: AsRef<OsStr>>(&mut self, name: T) -> Result<()>;
    fn set_readonly(&self, readonly: bool) -> Result<()> {
        let mut perm = self.metadata()?.permissions();
        perm.set_readonly(readonly);
        self.set_permissions(perm)
    }
    fn set_permissions(&self, perm: Permissions) -> Result<()> {
        fs::set_permissions(self.as_path(), perm)
    }
    fn delete(self) -> Result<()> {
        if self.read_only()? {
            self.set_readonly(false)?;
        }
        if self.as_path().is_dir() {
            remove_dir_all(self.as_path())
        } else {
            remove_file(self.as_path())
        }
    }
    fn copy_to<P: AsRef<Path>>(&self, path: P) -> TryRecoverResult<()> {
        let path = push_file_name(self.file_name(), path)?;
        self.copy_new(path)
    }
    fn copy_new<P: AsRef<Path>>(&self, path: P) -> TryRecoverResult<()>;
    fn move_to<P: AsRef<Path>>(&mut self, path: P) -> TryRecoverResult<()> {
        let path = push_file_name(self.file_name(), path)?;
        self.move_new(path)
    }
    fn move_new<P: AsRef<Path>>(&mut self, path: P) -> TryRecoverResult<()>;
}
#[inline]
fn _delete_file(file: &FileInfo) -> Result<()> {
    file.set_readonly(false)?;
    remove_file(file.as_path())
}

fn remove_file_any(path: &Path) -> Result<()> {
    let f = unsafe { FileInfo::open_uncheck(path) };
    f.delete()
}
