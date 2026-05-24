use std::path::Path;
use std::time::Duration;

use aws_config::BehaviorVersion;
use aws_config::sts::AssumeRoleProvider;
use aws_sdk_s3::Client as S3Client;
use aws_sdk_s3::error::ProvideErrorMetadata;
use aws_sdk_s3::presigning::PresigningConfig;
use aws_sdk_s3::primitives::ByteStream;
use snafu::ResultExt;
use tokio::{fs::File, fs::create_dir_all, io::AsyncWriteExt};

use crate::error::{CreateFileSnafu, UploadDirSnafu, UploadFileSnafu};
use crate::provider::{DownloadRequest, DownloadedFile, UploadUrlRequest};
use crate::{Error, Result};
use memo::dir::DirMeta;
use memo::file::{FileDto, ImgVersion, ImgVersionDto, ORIGINAL_PATH};

pub struct AwsStorageProvider {
    client: S3Client,
}

impl AwsStorageProvider {
    pub async fn new(role_arn: &str) -> Result<Self> {
        let client = create_storage_client(role_arn).await;
        Ok(Self { client })
    }

    async fn upload_image_object(
        &self,
        dir: &DirMeta,
        source_dir: &Path,
        file: &FileDto,
    ) -> Result<()> {
        if let Some(versions) = &file.img_versions {
            for version in versions.iter() {
                if version.version != ImgVersion::Original {
                    self.upload_image_version(dir, source_dir, file, version)
                        .await?;
                }
            }
        }

        Ok(())
    }

    async fn upload_image_version(
        &self,
        dir: &DirMeta,
        source_dir: &Path,
        file: &FileDto,
        version: &ImgVersionDto,
    ) -> Result<()> {
        let version_dir: String = version.version.to_string();
        let file_path = format!(
            "{}/{}/{}/{}/{}",
            &dir.org_id, &dir.dir_type, &dir.dir_name, &version_dir, &file.filename
        );

        let source_path = source_dir.join(&version_dir).join(&file.filename);
        let Ok(data) = std::fs::read(&source_path) else {
            return Err("Failed to read image version for upload.".into());
        };

        let upload_res = self
            .client
            .put_object()
            .bucket(&dir.bucket_name)
            .key(file_path)
            .content_type(file.content_type.clone())
            .body(ByteStream::from(data))
            .send()
            .await;

        match upload_res {
            Ok(_) => Ok(()),
            Err(err) => map_s3_error(err, "Failed to upload object to cloud storage."),
        }
    }

    async fn delete_object_by_path(&self, bucket_name: &str, path: &str) -> Result<()> {
        let res = self
            .client
            .delete_object()
            .bucket(bucket_name)
            .key(path)
            .send()
            .await;

        match res {
            Ok(_) => Ok(()),
            Err(err) => map_s3_error(err, "Failed to delete object from cloud storage."),
        }
    }

    pub async fn upload(&self, dir: &DirMeta, source_dir: &Path, file: &FileDto) -> Result<()> {
        if file.is_image {
            return self.upload_image_object(dir, source_dir, file).await;
        }

        Ok(())
    }

    pub async fn download(&self, req: DownloadRequest<'_>) -> Result<DownloadedFile> {
        let file_path = format!(
            "{}/{}/{}/{}/{}",
            req.org_id, req.dir_type, req.dir_name, req.version, req.new_filename
        );
        let res = self
            .client
            .get_object()
            .bucket(req.bucket_name)
            .key(file_path)
            .send()
            .await;

        match res {
            Ok(data) => {
                let version_dir = req.upload_dir.join(req.version);
                create_dir_all(version_dir.clone())
                    .await
                    .context(UploadDirSnafu)?;

                let file_path = version_dir.as_path().join(req.new_filename);
                let mut file = File::create(&file_path)
                    .await
                    .context(CreateFileSnafu { path: file_path })?;

                let bytes = data
                    .body
                    .collect()
                    .await
                    .map_err(|err| Error::Whatever {
                        msg: format!("Failed to read object data from cloud storage: {err}"),
                    })?
                    .into_bytes();
                let size = bytes.len();

                file.write_all(&bytes).await.context(UploadFileSnafu)?;

                Ok(DownloadedFile {
                    upload_dir: req.upload_dir.to_path_buf(),
                    name: req.orig_filename.to_owned(),
                    filename: req.new_filename.to_owned(),
                    path: version_dir.clone().join(req.new_filename),
                    size: size as i64,
                })
            }
            Err(err) => map_s3_error(err, "Failed to download object from cloud storage."),
        }
    }

    pub async fn delete(&self, dir: &DirMeta, file: &FileDto) -> Result<()> {
        if file.is_image {
            if let Some(versions) = &file.img_versions {
                for version in versions.iter() {
                    let path = format!(
                        "{}/{}/{}/{}/{}",
                        &dir.org_id, &dir.dir_type, &dir.dir_name, version.version, &file.filename
                    );
                    self.delete_object_by_path(&dir.bucket_name, &path).await?;
                }
            }
        } else {
            let path = format!(
                "{}/{}/{}/{}/{}",
                &dir.org_id, &dir.dir_type, &dir.dir_name, ORIGINAL_PATH, &file.filename
            );
            self.delete_object_by_path(&dir.bucket_name, &path).await?;
        }

        Ok(())
    }

    pub async fn attach_urls(&self, dir: &DirMeta, files: Vec<FileDto>) -> Result<Vec<FileDto>> {
        let client = self.client.clone();

        let mut tasks = Vec::with_capacity(files.len());
        for file in files.iter() {
            let client_copy = client.clone();
            let file_copy = file.clone();
            let dir_copy = dir.clone();

            tasks.push(tokio::spawn(async move {
                format_file_single(&client_copy, &dir_copy, file_copy).await
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

    pub async fn attach_url(&self, dir: &DirMeta, file: FileDto) -> Result<FileDto> {
        format_file_single(&self.client, dir, file).await
    }

    pub async fn generate_upload_url(&self, req: UploadUrlRequest<'_>) -> Result<String> {
        let file_path = format!(
            "{}/{}/{}/{}/{}",
            req.org_id, req.dir_type, req.dir_name, req.version, req.filename
        );
        generate_upload_signed_url(&self.client, req.bucket_name, &file_path, req.content_type)
            .await
    }
}

async fn create_storage_client(role_arn: &str) -> S3Client {
    let source_config = aws_config::load_defaults(BehaviorVersion::latest()).await;
    let assume_role_provider = AssumeRoleProvider::builder(role_arn)
        .configure(&source_config)
        .session_name("memo-rs-storage")
        .build()
        .await;

    let config = aws_config::defaults(BehaviorVersion::latest())
        .credentials_provider(assume_role_provider)
        .load()
        .await;

    S3Client::new(&config)
}

async fn generate_signed_url(
    client: &S3Client,
    bucket_name: &str,
    file_path: &str,
) -> Result<String> {
    let expires = Duration::from_secs(3600 * 12);
    let presign_config = PresigningConfig::expires_in(expires).map_err(|err| Error::Whatever {
        msg: format!("Failed to create signed URL config: {err}"),
    })?;

    let res = client
        .get_object()
        .bucket(bucket_name)
        .key(file_path)
        .presigned(presign_config)
        .await;

    match res {
        Ok(url) => Ok(url.uri().to_string()),
        Err(err) => Err(Error::Whatever {
            msg: format!("Failed to sign object URL: {err}"),
        }),
    }
}

async fn generate_upload_signed_url(
    client: &S3Client,
    bucket_name: &str,
    file_path: &str,
    content_type: &str,
) -> Result<String> {
    let expires = Duration::from_secs(3600);
    let presign_config = PresigningConfig::expires_in(expires).map_err(|err| Error::Whatever {
        msg: format!("Failed to create signed URL config: {err}"),
    })?;

    let res = client
        .put_object()
        .bucket(bucket_name)
        .key(file_path)
        .content_type(content_type)
        .presigned(presign_config)
        .await;

    match res {
        Ok(url) => Ok(url.uri().to_string()),
        Err(err) => Err(Error::Whatever {
            msg: format!("Failed to sign upload URL: {err}"),
        }),
    }
}

async fn format_file_single(
    client: &S3Client,
    dir: &DirMeta,
    mut file: FileDto,
) -> Result<FileDto> {
    let bucket_name = &dir.bucket_name;

    if file.is_image {
        if let Some(versions) = &file.img_versions
            && !versions.is_empty()
        {
            let mut updated_versions: Vec<ImgVersionDto> = Vec::with_capacity(versions.len());

            for i in 0..versions.len() {
                let mut version = versions[i].clone();
                let client_copy = client.clone();
                let file_path = format!(
                    "{}/{}/{}/{}/{}",
                    dir.org_id, dir.dir_type, dir.dir_name, version.version, file.filename
                );
                let url = generate_signed_url(&client_copy, bucket_name, &file_path).await?;
                version.url = Some(url);

                updated_versions.push(version);
            }

            if !updated_versions.is_empty() {
                // Attach the original version to the url
                let orig_url = updated_versions
                    .iter()
                    .find(|v| v.version == ImgVersion::Original)
                    .and_then(|v| v.url.clone());

                if let Some(orig_url) = orig_url {
                    file.url = Some(orig_url);
                }

                file.img_versions = Some(updated_versions);
            }
        }
    } else {
        let url = generate_signed_url(
            client,
            bucket_name,
            &format!(
                "{}/{}/{}/{}/{}",
                dir.org_id, dir.dir_type, dir.dir_name, ORIGINAL_PATH, file.filename
            ),
        )
        .await?;

        file.url = Some(url);
    }

    Ok(file)
}

fn map_s3_error<T, E>(err: E, fallback: &str) -> Result<T>
where
    E: ProvideErrorMetadata + std::fmt::Display,
{
    if let Some(code) = err.code() {
        if code == "AccessDenied" {
            return Err(Error::Forbidden {
                msg: err.to_string(),
            });
        }

        if code == "NoSuchKey" || code == "NoSuchBucket" {
            return Err(Error::NotFound {
                msg: err.to_string(),
            });
        }

        if code.starts_with("Invalid") {
            return Err(Error::Validation {
                msg: err.to_string(),
            });
        }
    }

    Err(Error::Whatever {
        msg: format!("{}: {}", fallback, err),
    })
}
