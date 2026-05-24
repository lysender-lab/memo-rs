use std::path::Path;

use crate::Result;
use crate::provider::{DownloadRequest, DownloadedFile, ProviderClient, UploadUrlRequest};
use crate::providers::AwsStorageProvider;
use memo::dir::DirMeta;
use memo::file::FileDto;

pub struct StorageClient {
    provider: ProviderClient,
}

impl StorageClient {
    pub async fn new_aws(role_arn: &str) -> Result<Self> {
        let provider = ProviderClient::Aws(AwsStorageProvider::new(role_arn).await?);
        Ok(Self { provider })
    }

    pub async fn upload(&self, dir: &DirMeta, source_dir: &Path, file: &FileDto) -> Result<()> {
        self.provider.upload(dir, source_dir, file).await
    }

    pub async fn download(
        &self,
        dir: &DirMeta,
        version: &str,
        orig_filename: &str,
        new_filename: &str,
        upload_dir: &Path,
    ) -> Result<DownloadedFile> {
        self.provider
            .download(DownloadRequest {
                bucket_name: &dir.bucket_name,
                org_id: &dir.org_id,
                dir_type: &dir.dir_type.to_string(),
                dir_name: &dir.dir_name,
                version,
                orig_filename,
                new_filename,
                upload_dir,
            })
            .await
    }

    pub async fn delete(&self, dir: &DirMeta, file: &FileDto) -> Result<()> {
        self.provider.delete(dir, file).await
    }

    pub async fn attach_urls(&self, dir: &DirMeta, files: Vec<FileDto>) -> Result<Vec<FileDto>> {
        self.provider.attach_urls(dir, files).await
    }

    pub async fn attach_url(&self, dir: &DirMeta, file: FileDto) -> Result<FileDto> {
        self.provider.attach_url(dir, file).await
    }

    pub async fn generate_upload_url(
        &self,
        dir: &DirMeta,
        version: &str,
        filename: &str,
        content_type: &str,
    ) -> Result<String> {
        self.provider
            .generate_upload_url(UploadUrlRequest {
                bucket_name: &dir.bucket_name,
                org_id: &dir.org_id,
                dir_type: &dir.dir_type.to_string(),
                dir_name: &dir.dir_name,
                version,
                filename,
                content_type,
            })
            .await
    }
}
