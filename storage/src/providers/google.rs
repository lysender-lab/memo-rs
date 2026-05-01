use std::path::Path;
use std::time::Duration;

use bytes::Bytes;
use google_cloud_auth::credentials::Builder as CredentialsBuilder;
use google_cloud_auth::signer::Signer;
use google_cloud_storage::Error as CloudError;
use google_cloud_storage::builder::storage::SignedUrlBuilder;
use google_cloud_storage::client::{Storage, StorageControl};
use google_cloud_storage::http::Method;
use snafu::ResultExt;
use tokio::{fs::File, fs::create_dir_all, io::AsyncWriteExt};

use crate::error::{CreateFileSnafu, GoogleSnafu, UploadDirSnafu};
use crate::provider::{DownloadRequest, DownloadedFile, UploadUrlRequest};
use crate::{Error, Result};
use memo::dir::DirMeta;
use memo::file::{FileDto, ImgVersion, ImgVersionDto, ORIGINAL_PATH};

pub struct GoogleStorageProvider {
    storage: Storage,
    control: StorageControl,
    signer: Signer,
}

impl GoogleStorageProvider {
    pub async fn new() -> Result<Self> {
        let storage = create_data_storage_client().await?;
        let control = create_storage_client().await?;
        let signer = create_storage_signer()?;

        Ok(Self {
            storage,
            control,
            signer,
        })
    }

    fn get_signer(&self) -> &Signer {
        &self.signer
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
        let bucket_name = bucket_resource_name(&dir.bucket_name);

        let source_path = source_dir.join(&version_dir).join(&file.filename);
        let Ok(data) = std::fs::read(&source_path) else {
            return Err("Failed to read image version for upload.".into());
        };

        let upload_res = self
            .storage
            .write_object(bucket_name, file_path, Bytes::from(data))
            .set_content_type(file.content_type.clone())
            .send_buffered()
            .await;

        match upload_res {
            Ok(_) => Ok(()),
            Err(e) => map_cloud_error(e, "Failed to upload object to cloud storage."),
        }
    }

    async fn delete_object_by_path(&self, bucket_name: &str, path: &str) -> Result<()> {
        let bucket_name = bucket_resource_name(bucket_name);
        let res = self
            .control
            .delete_object()
            .set_bucket(bucket_name)
            .set_object(path.to_string())
            .send()
            .await;

        match res {
            Ok(_) => Ok(()),
            Err(e) => map_cloud_error(e, "Failed to delete object from cloud storage."),
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
            .storage
            .read_object(bucket_resource_name(req.bucket_name), file_path)
            .send()
            .await;

        match res {
            Ok(mut data) => {
                let version_dir = req.upload_dir.join(req.version);
                create_dir_all(version_dir.clone())
                    .await
                    .context(UploadDirSnafu)?;

                let file_path = version_dir.as_path().join(req.new_filename);
                let mut size: usize = 0;
                let mut file = File::create(&file_path)
                    .await
                    .context(CreateFileSnafu { path: file_path })?;

                while let Ok(chunk_opt) = data.next().await.transpose() {
                    match chunk_opt {
                        Some(chunk) => {
                            size += chunk.len();
                            file.write_all(&chunk).await.unwrap();
                        }
                        None => break,
                    };
                }

                Ok(DownloadedFile {
                    upload_dir: req.upload_dir.to_path_buf(),
                    name: req.orig_filename.to_owned(),
                    filename: req.new_filename.to_owned(),
                    path: version_dir.clone().join(req.new_filename),
                    size: size as i64,
                })
            }
            Err(_) => Err("Failed to download object from cloud storage.".into()),
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
        let signer = self.get_signer();

        let mut tasks = Vec::with_capacity(files.len());
        for file in files.iter() {
            let signer_copy = signer.clone();
            let file_copy = file.clone();
            let dir_copy = dir.clone();

            tasks.push(tokio::spawn(async move {
                format_file_single(&signer_copy, &dir_copy, file_copy).await
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
        format_file_single(self.get_signer(), dir, file).await
    }

    pub async fn generate_upload_url(&self, req: UploadUrlRequest<'_>) -> Result<String> {
        let file_path = format!(
            "{}/{}/{}/{}/{}",
            req.org_id, req.dir_type, req.dir_name, req.version, req.filename
        );
        generate_upload_signed_url(
            self.get_signer(),
            &bucket_resource_name(req.bucket_name),
            &file_path,
            req.content_type,
        )
        .await
    }
}

async fn create_storage_client() -> Result<StorageControl> {
    let credentials = CredentialsBuilder::default()
        .build()
        .map_err(|err| format!("Error creating credentials: {}", err))?;

    StorageControl::builder()
        .with_credentials(credentials)
        .build()
        .await
        .map_err(|err| format!("Error creating Cloud Storage client: {}", err).into())
}

async fn create_data_storage_client() -> Result<Storage> {
    let credentials = CredentialsBuilder::default()
        .build()
        .map_err(|err| format!("Error creating credentials: {}", err))?;

    Storage::builder()
        .with_credentials(credentials)
        .build()
        .await
        .map_err(|err| format!("Error creating Cloud Storage data client: {}", err).into())
}

fn create_storage_signer() -> Result<Signer> {
    CredentialsBuilder::default()
        .build_signer()
        .map_err(|err| format!("Error creating cloud signer: {}", err).into())
}

async fn generate_signed_url(
    signer: &Signer,
    bucket_name: &str,
    file_path: &str,
) -> Result<String> {
    let expires = Duration::from_secs(3600 * 12);
    let res = SignedUrlBuilder::for_object(bucket_name.to_string(), file_path.to_string())
        .with_method(Method::GET)
        .with_expiration(expires)
        .sign_with(signer)
        .await;

    match res {
        Ok(url) => Ok(url),
        Err(err) => GoogleSnafu {
            msg: format!("Failed to sign object URL: {}", err),
        }
        .fail(),
    }
}

async fn generate_upload_signed_url(
    signer: &Signer,
    bucket_name: &str,
    file_path: &str,
    content_type: &str,
) -> Result<String> {
    let expires = Duration::from_secs(3600);
    let builder = SignedUrlBuilder::for_object(bucket_name.to_string(), file_path.to_string())
        .with_method(Method::PUT)
        .with_header("content-type", content_type)
        .with_expiration(expires);

    let res = builder.sign_with(signer).await;

    match res {
        Ok(url) => Ok(url),
        Err(err) => GoogleSnafu {
            msg: format!("Failed to sign upload URL: {}", err),
        }
        .fail(),
    }
}

async fn format_file_single(signer: &Signer, dir: &DirMeta, mut file: FileDto) -> Result<FileDto> {
    let bucket_name = bucket_resource_name(&dir.bucket_name);

    if file.is_image {
        if let Some(versions) = &file.img_versions
            && !versions.is_empty()
        {
            let mut updated_versions: Vec<ImgVersionDto> = Vec::with_capacity(versions.len());

            for i in 0..versions.len() {
                let mut version = versions[i].clone();
                let signer_copy = signer.clone();
                let file_path = format!(
                    "{}/{}/{}/{}/{}",
                    dir.org_id, dir.dir_type, dir.dir_name, version.version, file.filename
                );
                let url = generate_signed_url(&signer_copy, &bucket_name, &file_path).await?;
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
            signer,
            &bucket_name,
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

fn bucket_resource_name(bucket_name: &str) -> String {
    if bucket_name.starts_with("projects/") && bucket_name.contains("/buckets/") {
        return bucket_name.to_string();
    }

    format!("projects/_/buckets/{}", bucket_name)
}

fn map_cloud_error<T>(err: CloudError, fallback: &str) -> Result<T> {
    if let Some(code) = err.http_status_code() {
        if (400..500).contains(&code) {
            return Err(Error::Validation {
                msg: err.to_string(),
            });
        }

        return Err(Error::Google {
            msg: err.to_string(),
        });
    }

    Err(Error::Google {
        msg: format!("{}: {}", fallback, err),
    })
}
