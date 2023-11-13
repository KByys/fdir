use super::recover::{Status, TryRecover, TryRecoverResult};
use super::{remove_file_any, AsyncAction, AsyncInfo};
use crate::error::{already_exist, INVALID_PATH};
use crate::web::content_type;
use crate::{fix_path, get_file_path, is_same_root};
use async_trait::async_trait;
use std::ffi::OsStr;
use std::fmt::{Debug, Display};
use std::fs::Metadata;
use std::io::Result;
use std::path::{Path, PathBuf};
use tokio::fs::{copy, create_dir_all, metadata, rename, File};

#[derive(Debug, Clone)]
pub struct AsyncFileInfo {
    path: PathBuf,
}
unsafe impl Send for AsyncFileInfo {}

unsafe impl Sync for AsyncFileInfo {}

impl TryFrom<File> for AsyncFileInfo {
    type Error = std::io::Error;

    fn try_from(value: File) -> std::result::Result<Self, Self::Error> {
        let path = fix_path(get_file_path(format!("{:?}", value)))?;
        Ok(Self { path })
    }
}
impl Display for AsyncFileInfo {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_fmt(format_args!("{}", self.path.display()))
    }
}

impl AsyncFileInfo {
    pub async fn response_with_name(&self, name: impl AsRef<str>) -> hyper::Response<hyper::Body> {
        response(self, name).await
    }
    pub async fn response(&self) -> hyper::Response<hyper::Body> {
        let name = self
            .file_name()
            .and_then(|name| name.to_str())
            .unwrap_or("unknown_name");
        response(self, name).await
    }
}

#[async_trait]
impl AsyncInfo for AsyncFileInfo {
    fn as_path(&self) -> &Path {
        &self.path
    }

    async fn metadata(&self) -> Result<Metadata> {
        metadata(self.as_path()).await
    }

    async fn size(&self) -> u64 {
        self.metadata().await.map_or(0, |f| f.len())
    }
}

#[async_trait]
impl AsyncAction for AsyncFileInfo {
    async fn open<P: AsRef<Path> + Send + Sync>(path: P) -> Result<Self> {
        AsyncFileInfo::try_from(File::open(path).await?)
    }
    unsafe fn open_uncheck<P: AsRef<Path>>(path: P) -> Self {
        Self {
            path: path.as_ref().to_path_buf(),
        }
    }
    async fn rename<T: AsRef<OsStr> + Send + Sync>(&mut self, name: T) -> Result<()> {
        let mut new_path = self.path.clone();
        new_path.set_file_name(name.as_ref());
        if let Some(ext) = self.as_path().extension() {
            new_path.set_extension(ext);
        }
        rename(self.as_path(), &new_path).await?;
        self.path = new_path;
        Ok(())
    }
    async fn copy_new<P: AsRef<Path> + Send + Sync>(&self, path: P) -> TryRecoverResult<()> {
        let path = fix_path(path)?;
        if path.try_exists()? {
            return Err(TryRecover::new(
                already_exist(&path),
                Status::CopyFile(self, path),
            ));
        }
        copy(self.as_path(), &path).await?;
        Ok(())
    }
    async fn move_new<P: AsRef<Path> + Send + Sync>(&mut self, path: P) -> TryRecoverResult<()> {
        let path = fix_path(path)?;
        if path.try_exists()? {
            return Err(TryRecover::new(
                already_exist(&path),
                Status::MoveFile(self, path),
            ));
        }
        if let Some(parent) = path.parent() {
            create_dir_all(parent).await?;
        } else {
            INVALID_PATH()?;
        }
        if is_same_root(self.as_path(), &path) {
            rename(self.as_path(), &path).await?;
        } else {
            copy(self.as_path(), &path).await?;
            remove_file_any(self.as_path()).await?;
        }
        self.path = path;
        Ok(())
    }
}

async fn response(f: &AsyncFileInfo, file_name: impl AsRef<str>) -> hyper::Response<hyper::Body> {
    use hyper::{
        header::{HeaderValue, ACCESS_CONTROL_EXPOSE_HEADERS, CONTENT_DISPOSITION, CONTENT_TYPE},
        Body, Response, StatusCode,
    };
    match tokio::fs::read(f.as_path()).await {
        Ok(buf) => {
            let file_name = file_name.as_ref().to_string();
            let content_type = content_type(Some(file_name.as_ref())).unwrap_or("text/plain");

            let file_name: String =
                url::form_urlencoded::byte_serialize(file_name.as_bytes()).collect();
            let content_disposition = format!("attachment; filename={}", file_name);
            Response::builder()
                .status(StatusCode::OK)
                .header(CONTENT_TYPE, HeaderValue::from_static(content_type))
                .header(
                    CONTENT_DISPOSITION,
                    HeaderValue::from_str(content_disposition.as_str()).unwrap(),
                )
                .header(ACCESS_CONTROL_EXPOSE_HEADERS, CONTENT_DISPOSITION)
                .body(Body::from(buf))
                .unwrap()
        }
        Err(e) => Response::builder()
            .status(StatusCode::INTERNAL_SERVER_ERROR)
            .body(Body::from(e.to_string()))
            .unwrap(),
    }
}
