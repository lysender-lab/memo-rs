use std::path::PathBuf;
use std::time::Duration;

use bytes::Bytes;
use google_cloud_auth::credentials::Builder as CredentialsBuilder;
use google_cloud_auth::signer::Signer;
use google_cloud_storage::Error as CloudError;
use google_cloud_storage::builder::storage::SignedUrlBuilder;
use google_cloud_storage::client::{Storage, StorageControl};
use google_cloud_storage::http::Method;

use crate::Result;
use crate::error::{GoogleSnafu, ValidationSnafu};
use memo::bucket::BucketDto;
use memo::dir::DirDto;
use memo::file::ORIGINAL_PATH;
use memo::file::{FileDto, ImgVersionDto};

pub struct StorageClient {
    storage: Storage,
    control: StorageControl,
    signer: Signer,
}

impl StorageClient {
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

    async fn upload_regular_object(
        &self,
        bucket: &BucketDto,
        dir: &DirDto,
        source_dir: &PathBuf,
        file: &FileDto,
    ) -> Result<()> {
        let file_path = format!("{}/{}/{}", &dir.name, ORIGINAL_PATH, &file.filename);
        let bucket_name = bucket_resource_name(&bucket.name);

        let source_path = source_dir.join(ORIGINAL_PATH).join(&file.filename);
        let Ok(data) = std::fs::read(&source_path) else {
            return Err("Failed to read file for upload.".into());
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

    async fn upload_image_object(
        &self,
        bucket: &BucketDto,
        dir: &DirDto,
        source_dir: &PathBuf,
        file: &FileDto,
    ) -> Result<()> {
        if let Some(versions) = &file.img_versions {
            for version in versions.iter() {
                self.upload_image_version(bucket, dir, source_dir, file, version)
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
        let version_dir: String = version.version.to_string();
        let file_path = format!("{}/{}/{}", &dir.name, &version_dir, &file.filename);
        let bucket_name = bucket_resource_name(&bucket.name);

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

    pub async fn read_bucket(&self, name: &str) -> Result<String> {
        let bucket_name = bucket_resource_name(name);

        let res = self.control.get_bucket().set_name(bucket_name).send().await;

        match res {
            Ok(bucket) => Ok(bucket.name),
            Err(e) => {
                if let Some(code) = e.http_status_code() {
                    match code {
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
                        _ if (400..500).contains(&code) => {
                            ValidationSnafu { msg: e.to_string() }.fail()
                        }
                        _ => GoogleSnafu { msg: e.to_string() }.fail(),
                    }
                } else {
                    GoogleSnafu { msg: e.to_string() }.fail()
                }
            }
        }
    }

    pub async fn list_file_objects(&self, bucket_name: &str, prefix: &str) -> Result<Vec<String>> {
        let parent = bucket_resource_name(bucket_name);
        let mut page_token = String::new();
        let mut files = Vec::new();

        loop {
            let mut req = self
                .control
                .list_objects()
                .set_parent(parent.clone())
                .set_prefix(prefix.to_string());

            if !page_token.is_empty() {
                req = req.set_page_token(page_token.clone());
            }

            let res = req.send().await;
            let response = match res {
                Ok(response) => response,
                Err(e) => {
                    return map_cloud_error(e, "Failed to list files from cloud storage.");
                }
            };

            files.extend(response.objects.iter().map(|obj| obj.name.clone()));

            if response.next_page_token.is_empty() {
                break;
            }
            page_token = response.next_page_token;
        }

        Ok(files)
    }

    pub async fn get_file_object(&self, bucket_name: &str, object_path: &str) -> Result<String> {
        let bucket_name = bucket_resource_name(bucket_name);
        let res = self
            .control
            .get_object()
            .set_bucket(bucket_name)
            .set_object(object_path.to_string())
            .send()
            .await;

        match res {
            Ok(object) => Ok(object.name),
            Err(e) => map_cloud_error(e, "Failed to get file from cloud storage."),
        }
    }

    pub async fn upload_object(
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

    pub async fn delete_file_object(
        &self,
        bucket_name: &str,
        dir_name: &str,
        file: &FileDto,
    ) -> Result<()> {
        if file.is_image {
            // Delete all versions
            if let Some(versions) = &file.img_versions {
                for version in versions.iter() {
                    let path = format!("{}/{}/{}", dir_name, version.version, &file.filename);
                    let _ = self.delete_object_by_path(bucket_name, &path).await?;
                }
            }
        } else {
            let path = format!("{}/{}/{}", dir_name, ORIGINAL_PATH, &file.filename);
            let _ = self.delete_object_by_path(bucket_name, &path).await?;
        }

        Ok(())
    }

    pub async fn format_files(
        &self,
        bucket_name: &str,
        dir_name: &str,
        files: Vec<FileDto>,
    ) -> Result<Vec<FileDto>> {
        let signer = self.get_signer();
        let bucket_resource = bucket_resource_name(bucket_name);

        let mut tasks = Vec::with_capacity(files.len());
        for file in files.iter() {
            let signer_copy = signer.clone();
            let file_copy = file.clone();
            let bname = bucket_resource.clone();
            let dir_name_copy = dir_name.to_string();

            tasks.push(tokio::spawn(async move {
                format_file_single(&signer_copy, &bname, &dir_name_copy, file_copy).await
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

    pub async fn format_file(
        &self,
        bucket_name: &str,
        dir_name: &str,
        file: FileDto,
    ) -> Result<FileDto> {
        let bucket_resource = bucket_resource_name(bucket_name);
        format_file_single(self.get_signer(), &bucket_resource, dir_name, file).await
    }
}

pub async fn create_storage_client() -> Result<StorageControl> {
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

pub async fn test_list_hmac_keys(client: &StorageControl, project_id: &str) -> Result<()> {
    let parent = format!("projects/{}", project_id);
    let res = client
        .list_buckets()
        .set_parent(parent)
        .set_page_size(1)
        .send()
        .await;

    match res {
        Ok(_) => Ok(()),
        Err(e) => map_cloud_error(e, "Failed to list buckets from cloud storage."),
    }
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

async fn format_file_single(
    signer: &Signer,
    bucket_name: &str,
    dir_name: &str,
    mut file: FileDto,
) -> Result<FileDto> {
    if file.is_image {
        if let Some(versions) = &file.img_versions
            && !versions.is_empty()
        {
            let mut updated_versions: Vec<ImgVersionDto> = Vec::with_capacity(versions.len());

            for i in 0..versions.len() {
                let mut version = versions[i].clone();
                let signer_copy = signer.clone();
                let bname = bucket_name.to_string();
                let file_path = format!("{}/{}/{}", dir_name, version.version, file.filename);
                let url = generate_signed_url(&signer_copy, &bname, &file_path).await?;
                version.url = Some(url);

                updated_versions.push(version);
            }

            if !updated_versions.is_empty() {
                file.img_versions = Some(updated_versions);
            }
        }
    } else {
        let url = generate_signed_url(
            &signer,
            bucket_name,
            &format!("{}/{}/{}", dir_name, ORIGINAL_PATH, file.filename),
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
            return ValidationSnafu {
                msg: err.to_string(),
            }
            .fail();
        }

        return GoogleSnafu {
            msg: err.to_string(),
        }
        .fail();
    }

    GoogleSnafu {
        msg: format!("{} {}", fallback, err),
    }
    .fail()
}
