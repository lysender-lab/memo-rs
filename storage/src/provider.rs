use std::path::Path;

use memo::dir::DirDto;
use memo::file::FileDto;

use crate::Result;
use crate::providers::GoogleStorageProvider;

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
    pub dir_name: &'a str,
    pub version: &'a str,
    pub orig_filename: &'a str,
    pub new_filename: &'a str,
    pub upload_dir: &'a Path,
}

pub struct UploadUrlRequest<'a> {
    pub bucket_name: &'a str,
    pub dir_name: &'a str,
    pub version: &'a str,
    pub filename: &'a str,
    pub content_type: &'a str,
}

pub enum ProviderClient {
    Google(GoogleStorageProvider),
}

impl ProviderClient {
    pub async fn upload(
        &self,
        bucket_name: &str,
        dir: &DirDto,
        source_dir: &Path,
        file: &FileDto,
    ) -> Result<()> {
        match self {
            Self::Google(provider) => provider.upload(bucket_name, dir, source_dir, file).await,
        }
    }

    pub async fn download(&self, req: DownloadRequest<'_>) -> Result<DownloadedFile> {
        match self {
            Self::Google(provider) => provider.download(req).await,
        }
    }

    pub async fn delete(&self, bucket_name: &str, dir_name: &str, file: &FileDto) -> Result<()> {
        match self {
            Self::Google(provider) => provider.delete(bucket_name, dir_name, file).await,
        }
    }

    pub async fn attach_urls(
        &self,
        bucket_name: &str,
        dir_name: &str,
        files: Vec<FileDto>,
    ) -> Result<Vec<FileDto>> {
        match self {
            Self::Google(provider) => provider.attach_urls(bucket_name, dir_name, files).await,
        }
    }

    pub async fn attach_url(
        &self,
        bucket_name: &str,
        dir_name: &str,
        file: FileDto,
    ) -> Result<FileDto> {
        match self {
            Self::Google(provider) => provider.attach_url(bucket_name, dir_name, file).await,
        }
    }

    pub async fn generate_upload_url(&self, req: UploadUrlRequest<'_>) -> Result<String> {
        match self {
            Self::Google(provider) => provider.generate_upload_url(req).await,
        }
    }
}
