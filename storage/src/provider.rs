use std::path::Path;

use memo::dir::DirMeta;
use memo::file::FileDto;

use crate::Result;
use crate::providers::AwsStorageProvider;

#[derive(Debug, Clone)]
pub struct DownloadedFile {
    pub upload_dir: std::path::PathBuf,
    pub name: String,
    pub filename: String,
    pub path: std::path::PathBuf,
    pub size: i64,
}

pub struct DownloadRequest<'a> {
    pub bucket_name: &'a str,
    pub org_id: &'a str,
    pub dir_type: &'a str,
    pub dir_name: &'a str,
    pub version: &'a str,
    pub orig_filename: &'a str,
    pub new_filename: &'a str,
    pub upload_dir: &'a Path,
}

pub struct UploadUrlRequest<'a> {
    pub bucket_name: &'a str,
    pub org_id: &'a str,
    pub dir_type: &'a str,
    pub dir_name: &'a str,
    pub version: &'a str,
    pub filename: &'a str,
    pub content_type: &'a str,
}

pub enum ProviderClient {
    Aws(AwsStorageProvider),
}

impl ProviderClient {
    pub async fn upload(&self, dir: &DirMeta, source_dir: &Path, file: &FileDto) -> Result<()> {
        match self {
            Self::Aws(provider) => provider.upload(dir, source_dir, file).await,
        }
    }

    pub async fn download(&self, req: DownloadRequest<'_>) -> Result<DownloadedFile> {
        match self {
            Self::Aws(provider) => provider.download(req).await,
        }
    }

    pub async fn delete(&self, dir: &DirMeta, file: &FileDto) -> Result<()> {
        match self {
            Self::Aws(provider) => provider.delete(dir, file).await,
        }
    }

    pub async fn attach_urls(&self, dir: &DirMeta, files: Vec<FileDto>) -> Result<Vec<FileDto>> {
        match self {
            Self::Aws(provider) => provider.attach_urls(dir, files).await,
        }
    }

    pub async fn attach_url(&self, dir: &DirMeta, file: FileDto) -> Result<FileDto> {
        match self {
            Self::Aws(provider) => provider.attach_url(dir, file).await,
        }
    }

    pub async fn generate_upload_url(&self, req: UploadUrlRequest<'_>) -> Result<String> {
        match self {
            Self::Aws(provider) => provider.generate_upload_url(req).await,
        }
    }
}
