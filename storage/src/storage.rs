use async_trait::async_trait;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

use google_cloud_storage::client::google_cloud_auth::credentials::CredentialsFile;
use google_cloud_storage::client::{Client, ClientConfig};
use google_cloud_storage::http::Error as CloudError;
use google_cloud_storage::http::buckets::get::GetBucketRequest;
use google_cloud_storage::http::hmac_keys::list::ListHmacKeysRequest;
use google_cloud_storage::http::objects::delete::DeleteObjectRequest;
use google_cloud_storage::http::objects::upload::{Media, UploadObjectRequest, UploadType};
use google_cloud_storage::sign::SignedURLOptions;

use aws_config::from_env;
use aws_credential_types::CredentialsBuilder;
use aws_types::region::Region;

use crate::Result;
use crate::error::{GoogleSnafu, ValidationSnafu};
use memo::bucket::BucketDto;
use memo::dir::DirDto;
use memo::file::ORIGINAL_PATH;
use memo::file::{FileDto, ImgVersionDto};

#[async_trait]
pub trait CloudStorable: Send + Sync {
    async fn read_bucket(&self, name: &str) -> Result<String>;

    async fn upload_object(
        &self,
        bucket: &BucketDto,
        dir: &DirDto,
        source_dir: &PathBuf,
        file: &FileDto,
    ) -> Result<()>;

    async fn delete_file_object(
        &self,
        bucket_name: &str,
        dir_name: &str,
        file: &FileDto,
    ) -> Result<()>;

    async fn format_files(
        &self,
        bucket_name: &str,
        dir_name: &str,
        files: Vec<FileDto>,
    ) -> Result<Vec<FileDto>>;

    async fn format_file(
        &self,
        bucket_name: &str,
        dir_name: &str,
        file: FileDto,
    ) -> Result<FileDto>;
}

pub struct StorageClientS3 {
    client: Arc<aws_sdk_s3::Client>,
}

impl StorageClientS3 {
    pub async fn new(region: &str, access_key: &str, secret_key: &str) -> Self {
        // Manually create credentials
        let credentials = CredentialsBuilder::default()
            .access_key_id(access_key)
            .secret_access_key(secret_key)
            .build();

        let region = Region::new(region.to_string());

        // Build config with manually provided credentials and region
        let config = from_env()
            .region(region)
            .credentials_provider(credentials)
            .load()
            .await;

        let client = aws_sdk_s3::Client::new(&config);

        Self {
            client: Arc::new(client),
        }
    }

    pub fn get_client(&self) -> Arc<aws_sdk_s3::Client> {
        self.client.clone()
    }
}

pub struct StorageClient {
    client: Arc<Client>,
}

impl StorageClient {
    pub async fn new(key_file: &str) -> Result<Self> {
        let client = create_storage_client(key_file).await?;
        Ok(Self {
            client: Arc::new(client),
        })
    }

    pub fn get_client(&self) -> Arc<Client> {
        self.client.clone()
    }

    async fn upload_regular_object(
        &self,
        bucket: &BucketDto,
        dir: &DirDto,
        source_dir: &PathBuf,
        file: &FileDto,
    ) -> Result<()> {
        // Prepare media
        let file_path = format!("{}/{}/{}", &dir.name, ORIGINAL_PATH, &file.filename);
        let mut media = Media::new(file_path.clone());
        media.content_type = file.content_type.clone().into();
        let upload_type = UploadType::Simple(media);

        // Read file, preferred a stream but skill issues...
        let source_path = source_dir.join(ORIGINAL_PATH).join(&file.filename);
        let Ok(data) = std::fs::read(&source_path) else {
            return Err("Failed to read file for upload.".into());
        };

        let upload_res = self
            .client
            .upload_object(
                &UploadObjectRequest {
                    bucket: bucket.name.clone(),
                    ..Default::default()
                },
                data,
                &upload_type,
            )
            .await;

        match upload_res {
            Ok(_) => Ok(()),
            Err(e) => match e {
                CloudError::Response(gerr) => {
                    if gerr.code >= 400 && gerr.code < 500 {
                        ValidationSnafu { msg: gerr.message }.fail()
                    } else {
                        GoogleSnafu { msg: gerr.message }.fail()
                    }
                }
                _ => Err("Failed to upload object to cloud storage.".into()),
            },
        }
    }

    async fn upload_image_object(
        &self,
        bucket: &BucketDto,
        dir: &DirDto,
        source_dir: &PathBuf,
        file: &FileDto,
    ) -> Result<()> {
        if let Some(versions) = &file.img_versions {
            for version in versions.iter() {
                let _ = self
                    .upload_image_version(bucket, dir, source_dir, &file, version)
                    .await?;
            }
        }

        Ok(())
    }

    async fn upload_image_version(
        &self,
        bucket: &BucketDto,
        dir: &DirDto,
        source_dir: &PathBuf,
        file: &FileDto,
        version: &ImgVersionDto,
    ) -> Result<()> {
        // Prepare media
        let version_dir: String = version.version.to_string();
        let file_path = format!("{}/{}/{}", &dir.name, &version_dir, &file.filename);
        let mut media = Media::new(file_path.clone());
        media.content_type = file.content_type.clone().into();
        let upload_type = UploadType::Simple(media);

        // Read file, preferred a stream but skill issues...
        let source_path = source_dir.join(&version_dir).join(&file.filename);
        let Ok(data) = std::fs::read(&source_path) else {
            return Err("Failed to read image version for upload.".into());
        };

        let upload_res = self
            .client
            .upload_object(
                &UploadObjectRequest {
                    bucket: bucket.name.clone(),
                    ..Default::default()
                },
                data,
                &upload_type,
            )
            .await;

        match upload_res {
            Ok(_) => Ok(()),
            Err(e) => match e {
                CloudError::Response(gerr) => {
                    if gerr.code >= 400 && gerr.code < 500 {
                        ValidationSnafu { msg: gerr.message }.fail()
                    } else {
                        GoogleSnafu { msg: gerr.message }.fail()
                    }
                }
                _ => Err("Failed to upload object to cloud storage.".into()),
            },
        }
    }

    async fn delete_object_by_path(&self, bucket_name: &str, path: &str) -> Result<()> {
        let res = self
            .client
            .delete_object(&DeleteObjectRequest {
                bucket: bucket_name.to_string(),
                object: path.to_string(),
                ..Default::default()
            })
            .await;

        match res {
            Ok(_) => Ok(()),
            Err(e) => match e {
                CloudError::Response(gerr) => {
                    if gerr.code >= 400 && gerr.code < 500 {
                        ValidationSnafu { msg: gerr.message }.fail()
                    } else {
                        GoogleSnafu { msg: gerr.message }.fail()
                    }
                }
                _ => Err("Failed to delete object from cloud storage.".into()),
            },
        }
    }
}

#[async_trait]
impl CloudStorable for StorageClient {
    async fn read_bucket(&self, name: &str) -> Result<String> {
        let res = self
            .client
            .get_bucket(&GetBucketRequest {
                bucket: name.to_string(),
                ..Default::default()
            })
            .await;

        match res {
            Ok(bucket) => Ok(bucket.name),
            Err(e) => match e {
                CloudError::Response(gerr) => {
                    if gerr.code >= 400 && gerr.code < 500 {
                        match gerr.code {
                            401 => ValidationSnafu {
                                msg: "Cloud Storage: Unauthorized",
                            }
                            .fail(),
                            403 => ValidationSnafu {
                                msg: "Cloud Storage: Forbidden",
                            }
                            .fail(),
                            404 => ValidationSnafu {
                                msg: "Cloud Storage: Bucket not found",
                            }
                            .fail(),
                            _ => ValidationSnafu { msg: gerr.message }.fail(),
                        }
                    } else {
                        GoogleSnafu { msg: gerr.message }.fail()
                    }
                }
                _ => Err("Failed to read bucket from cloud storage.".into()),
            },
        }
    }

    async fn upload_object(
        &self,
        bucket: &BucketDto,
        dir: &DirDto,
        source_dir: &PathBuf,
        file: &FileDto,
    ) -> Result<()> {
        match file.is_image {
            true => {
                self.upload_image_object(bucket, dir, source_dir, file)
                    .await
            }
            false => {
                self.upload_regular_object(bucket, dir, source_dir, file)
                    .await
            }
        }
    }

    async fn delete_file_object(
        &self,
        bucket_name: &str,
        dir_name: &str,
        file: &FileDto,
    ) -> Result<()> {
        if file.is_image {
            // Delete all versions
            if let Some(versions) = &file.img_versions {
                for version in versions.iter() {
                    let path = format!(
                        "{}/{}/{}",
                        dir_name,
                        version.version.to_string(),
                        &file.filename
                    );
                    let _ = self.delete_object_by_path(bucket_name, &path).await?;
                }
            }
        } else {
            let path = format!("{}/{}/{}", dir_name, ORIGINAL_PATH, &file.filename);
            let _ = self.delete_object_by_path(bucket_name, &path).await?;
        }

        Ok(())
    }

    async fn format_files(
        &self,
        bucket_name: &str,
        dir_name: &str,
        files: Vec<FileDto>,
    ) -> Result<Vec<FileDto>> {
        let client = self.get_client();

        let mut tasks = Vec::with_capacity(files.len());
        for file in files.iter() {
            let client_copy = client.clone();
            let file_copy = file.clone();
            let bname = bucket_name.to_string();
            let dir_name_copy = dir_name.to_string();

            tasks.push(tokio::spawn(async move {
                format_file_single(client_copy, &bname, &dir_name_copy, file_copy).await
            }));
        }

        let mut updated_files: Vec<FileDto> = Vec::with_capacity(files.len());
        for task in tasks {
            let Ok(res) = task.await else {
                return Err("Unable to extract data from spanwed task.".into());
            };
            let file = res?;
            updated_files.push(file);
        }

        Ok(updated_files)
    }

    async fn format_file(
        &self,
        bucket_name: &str,
        dir_name: &str,
        file: FileDto,
    ) -> Result<FileDto> {
        format_file_single(self.get_client(), bucket_name, dir_name, file).await
    }
}

pub async fn create_storage_client(key_file: &str) -> Result<Client> {
    match CredentialsFile::new_from_file(key_file.to_string()).await {
        Ok(creds) => match ClientConfig::default().with_credentials(creds).await {
            Ok(config) => Ok(Client::new(config)),
            Err(err) => Err(format!("Error creating Cloud Storage config: {}", err).into()),
        },
        Err(err) => Err(format!("Error reading credentials file: {}", err).into()),
    }
}

pub async fn test_list_hmac_keys(client: &Client, project_id: &str) -> Result<()> {
    let res = client
        .list_hmac_keys(&ListHmacKeysRequest {
            project_id: project_id.to_string(),
            ..Default::default()
        })
        .await;

    match res {
        Ok(_) => Ok(()),
        Err(e) => match e {
            CloudError::Response(gerr) => {
                if gerr.code >= 400 && gerr.code < 500 {
                    ValidationSnafu { msg: gerr.message }.fail()
                } else {
                    GoogleSnafu { msg: gerr.message }.fail()
                }
            }
            _ => Err("Failed to list buckets from cloud storage.".into()),
        },
    }
}

async fn generate_signed_url(
    client: &Client,
    bucket_name: &str,
    file_path: &str,
) -> Result<String> {
    let expires = Duration::from_secs(3600 * 12);
    let mut options = SignedURLOptions::default();
    options.expires = expires;

    let res = client
        .signed_url(bucket_name, file_path, None, None, options)
        .await;

    match res {
        Ok(url) => Ok(url),
        Err(_) => Err("Failed to sign object URL.".into()),
    }
}

async fn format_file_single(
    client: Arc<Client>,
    bucket_name: &str,
    dir_name: &str,
    mut file: FileDto,
) -> Result<FileDto> {
    if file.is_image {
        if let Some(versions) = &file.img_versions {
            if versions.len() > 0 {
                let mut tasks = Vec::with_capacity(versions.len());

                for version in versions.iter() {
                    let client_copy = client.clone();
                    let bname = bucket_name.to_string();
                    let file_path = format!(
                        "{}/{}/{}",
                        dir_name,
                        version.version.to_string(),
                        file.filename
                    );

                    tasks.push(tokio::spawn(async move {
                        generate_signed_url(&client_copy, &bname, &file_path).await
                    }));
                }

                let mut updated_versions: Vec<ImgVersionDto> = Vec::with_capacity(versions.len());
                for (k, task) in tasks.into_iter().enumerate() {
                    let Ok(res) = task.await else {
                        return Err("Unable to extract data from spanwed task.".into());
                    };
                    let url = res?;
                    let mut version = versions[k].clone();
                    version.url = Some(url);

                    updated_versions.push(version);
                }

                if updated_versions.len() > 0 {
                    file.img_versions = Some(updated_versions);
                }
            }
        }
    } else {
        let url = generate_signed_url(
            &client,
            bucket_name,
            &format!("{}/{}/{}", dir_name, ORIGINAL_PATH, file.filename),
        )
        .await?;
        file.url = Some(url);
    }

    Ok(file)
}

#[cfg(feature = "test")]
pub struct StorageTestClient {}

#[cfg(feature = "test")]
impl StorageTestClient {
    pub fn new() -> Self {
        Self {}
    }
}

#[cfg(feature = "test")]
#[async_trait]
impl CloudStorable for StorageTestClient {
    async fn read_bucket(&self, name: &str) -> Result<String> {
        Ok(name.to_string())
    }

    async fn upload_object(
        &self,
        _bucket: &BucketDto,
        _dir: &DirDto,
        _source_dir: &PathBuf,
        _file: &FileDto,
    ) -> Result<()> {
        Ok(())
    }

    async fn delete_file_object(
        &self,
        _bucket_name: &str,
        _dir_name: &str,
        _file: &FileDto,
    ) -> Result<()> {
        Ok(())
    }

    async fn format_files(
        &self,
        _bucket_name: &str,
        _dir_name: &str,
        files: Vec<FileDto>,
    ) -> Result<Vec<FileDto>> {
        Ok(files)
    }

    async fn format_file(
        &self,
        _bucket_name: &str,
        _dir_name: &str,
        file: FileDto,
    ) -> Result<FileDto> {
        Ok(file)
    }
}
