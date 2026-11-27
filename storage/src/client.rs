use std::path::Path;
use std::time::SystemTime;

use crate::Result;
use crate::provider::{DownloadRequest, DownloadedFile, ProviderClient, UploadUrlRequest};
use crate::providers::AwsStorageProvider;
use memo::dir::DirMeta;
use memo::file::{FileDto, ORIGINAL_PATH};

pub fn original_object_key(dir: &DirMeta, filename: &str) -> String {
    format!(
        "{}/{}/{}/{ORIGINAL_PATH}/{filename}",
        dir.org_id, dir.dir_type, dir.dir_name
    )
}

pub struct StorageClient {
    provider: ProviderClient,
}

impl StorageClient {
    pub async fn new_aws() -> Result<Self> {
        let provider = ProviderClient::Aws(AwsStorageProvider::new().await?);
        Ok(Self { provider })
    }

    pub async fn upload(&self, dir: &DirMeta, source_dir: &Path, file: &FileDto) -> Result<()> {
        self.provider.upload(dir, source_dir, file).await
    }

    /// Replaces the original object with a local copy and its verified content type.
    ///
    /// This overwrites the object and does not preserve its other S3 metadata.
    /// The caller must ensure the local file is the intended original and that its
    /// content type has been verified.
    pub async fn reupload_original(
        &self,
        dir: &DirMeta,
        filename: &str,
        local_path: &Path,
        verified_content_type: &str,
    ) -> Result<()> {
        self.provider
            .reupload_original(dir, filename, local_path, verified_content_type)
            .await
    }

    pub async fn download(
        &self,
        dir: &DirMeta,
        version: &str,
        orig_filename: &str,
        new_filename: &str,
        content_type: &str,
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
                content_type,
                upload_dir,
            })
            .await
    }

    pub async fn delete(&self, dir: &DirMeta, file: &FileDto) -> Result<()> {
        self.provider.delete(dir, file).await
    }

    /// Attempt every possible image object, including partially uploaded versions.
    pub async fn delete_processing_file(&self, dir: &DirMeta, file: &FileDto) -> Result<()> {
        self.provider.delete_processing_file(dir, file).await
    }

    pub async fn attach_urls(&self, dir: &DirMeta, files: Vec<FileDto>) -> Result<Vec<FileDto>> {
        self.provider.attach_urls(dir, files).await
    }

    pub async fn attach_url(&self, dir: &DirMeta, file: FileDto) -> Result<FileDto> {
        self.provider.attach_url(dir, file).await
    }

    pub async fn generate_download_url(
        &self,
        bucket_name: &str,
        storage_key: &str,
    ) -> Result<String> {
        self.provider
            .generate_download_url(bucket_name, storage_key)
            .await
    }

    pub async fn generate_upload_url(
        &self,
        bucket_name: &str,
        storage_key: &str,
        content_type: &str,
        deadline: SystemTime,
    ) -> Result<String> {
        self.provider
            .generate_upload_url(UploadUrlRequest {
                bucket_name,
                storage_key,
                content_type,
                deadline,
            })
            .await
    }
}
