use std::path::Path;

use crate::Result;
use crate::provider::{DownloadRequest, DownloadedFile, ProviderClient, UploadUrlRequest};
use crate::providers::GoogleStorageProvider;
use memo::dir::DirDto;
use memo::file::FileDto;

pub struct StorageClient {
    provider: ProviderClient,
}

impl StorageClient {
    pub async fn new_google() -> Result<Self> {
        let provider = ProviderClient::Google(GoogleStorageProvider::new().await?);
        Ok(Self { provider })
    }

    pub async fn upload(
        &self,
        bucket_name: &str,
        dir: &DirDto,
        source_dir: &Path,
        file: &FileDto,
    ) -> Result<()> {
        self.provider
            .upload(bucket_name, dir, source_dir, file)
            .await
    }

    pub async fn download(
        &self,
        bucket_name: &str,
        dir_name: &str,
        version: &str,
        orig_filename: &str,
        new_filename: &str,
        upload_dir: &Path,
    ) -> Result<DownloadedFile> {
        self.provider
            .download(DownloadRequest {
                bucket_name,
                dir_name,
                version,
                orig_filename,
                new_filename,
                upload_dir,
            })
            .await
    }

    pub async fn delete(&self, bucket_name: &str, dir_name: &str, file: &FileDto) -> Result<()> {
        self.provider.delete(bucket_name, dir_name, file).await
    }

    pub async fn attach_urls(
        &self,
        bucket_name: &str,
        dir_name: &str,
        files: Vec<FileDto>,
    ) -> Result<Vec<FileDto>> {
        self.provider
            .attach_urls(bucket_name, dir_name, files)
            .await
    }

    pub async fn attach_url(
        &self,
        bucket_name: &str,
        dir_name: &str,
        file: FileDto,
    ) -> Result<FileDto> {
        self.provider.attach_url(bucket_name, dir_name, file).await
    }

    pub async fn generate_upload_url(
        &self,
        bucket_name: &str,
        dir_name: &str,
        version: &str,
        filename: &str,
        content_type: &str,
    ) -> Result<String> {
        self.provider
            .generate_upload_url(UploadUrlRequest {
                bucket_name,
                dir_name,
                version,
                filename,
                content_type,
            })
            .await
    }
}
