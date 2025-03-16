use std::path::PathBuf;
use std::time::Duration;

use google_cloud_storage::client::google_cloud_auth::credentials::CredentialsFile;
use google_cloud_storage::client::{Client, ClientConfig};
use google_cloud_storage::http::Error as CloudError;
use google_cloud_storage::http::buckets::get::GetBucketRequest;
use google_cloud_storage::http::hmac_keys::list::ListHmacKeysRequest;
use google_cloud_storage::http::objects::delete::DeleteObjectRequest;
use google_cloud_storage::http::objects::upload::{Media, UploadObjectRequest, UploadType};
use google_cloud_storage::sign::SignedURLOptions;

use crate::buckets::BucketDto;
use crate::dirs::Dir;
use crate::files::{FileDto, ImgVersionDto, ORIGINAL_PATH};
use crate::{Error, Result};

pub async fn create_storage_client(key_file: &str) -> Result<Client> {
    match CredentialsFile::new_from_file(key_file.to_string()).await {
        Ok(creds) => match ClientConfig::default().with_credentials(creds).await {
            Ok(config) => Ok(Client::new(config)),
            Err(err) => Err(format!("Error creating Cloud Storage config: {}", err).into()),
        },
        Err(err) => Err(format!("Error reading credentials file: {}", err).into()),
    }
}

pub async fn read_bucket(client: &Client, name: &str) -> Result<String> {
    let res = client
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
                    Err(Error::ValidationError(gerr.message))
                } else {
                    Err(format!("Google error: {}", gerr.message).as_str().into())
                }
            }
            _ => Err("Failed to read bucket from cloud storage.".into()),
        },
    }
}

pub async fn upload_object(
    client: &Client,
    bucket: &BucketDto,
    dir: &Dir,
    source_dir: &PathBuf,
    file: &FileDto,
) -> Result<()> {
    match file.is_image {
        true => upload_image_object(client, bucket, dir, source_dir, file).await,
        false => upload_regular_object(client, bucket, dir, source_dir, file).await,
    }
}

async fn upload_regular_object(
    client: &Client,
    bucket: &BucketDto,
    dir: &Dir,
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

    let upload_res = client
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
                    Err(Error::ValidationError(gerr.message))
                } else {
                    Err(format!("Google error: {}", gerr.message).as_str().into())
                }
            }
            _ => Err("Failed to upload object to cloud storage.".into()),
        },
    }
}

async fn upload_image_object(
    client: &Client,
    bucket: &BucketDto,
    dir: &Dir,
    source_dir: &PathBuf,
    file: &FileDto,
) -> Result<()> {
    if let Some(versions) = &file.img_versions {
        for version in versions.iter() {
            let _ = upload_image_version(&client, bucket, dir, source_dir, &file, version).await?;
        }
    }

    Ok(())
}

async fn upload_image_version(
    client: &Client,
    bucket: &BucketDto,
    dir: &Dir,
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

    let upload_res = client
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
                    Err(Error::ValidationError(gerr.message))
                } else {
                    Err(format!("Google error: {}", gerr.message).as_str().into())
                }
            }
            _ => Err("Failed to upload object to cloud storage.".into()),
        },
    }
}

pub async fn delete_file_object(
    client: &Client,
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
                let _ = delete_object_by_path(&client, bucket_name, &path).await?;
            }
        }
    } else {
        let path = format!("{}/{}/{}", dir_name, ORIGINAL_PATH, &file.filename);
        let _ = delete_object_by_path(&client, bucket_name, &path).await?;
    }

    Ok(())
}

async fn delete_object_by_path(client: &Client, bucket_name: &str, path: &str) -> Result<()> {
    let res = client
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
                    Err(Error::ValidationError(gerr.message))
                } else {
                    Err(format!("Google error: {}", gerr.message).as_str().into())
                }
            }
            _ => Err("Failed to delete object from cloud storage.".into()),
        },
    }
}

pub async fn format_files(
    client: &Client,
    bucket_name: &str,
    dir: &str,
    files: Vec<FileDto>,
) -> Result<Vec<FileDto>> {
    let mut tasks = Vec::with_capacity(files.len());
    for file in files.iter() {
        let client_copy = client.clone();
        let file_copy = file.clone();
        let bname = bucket_name.to_string();
        let dir_name = dir.to_string();

        tasks.push(tokio::spawn(async move {
            format_file_single(&client_copy, &bname, &dir_name, file_copy).await
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
    client: &Client,
    bucket_name: &str,
    dir_name: &str,
    file: FileDto,
) -> Result<FileDto> {
    format_file_single(&client, bucket_name, dir_name, file).await
}

async fn format_file_single(
    client: &Client,
    bucket_name: &str,
    dir_name: &str,
    mut file: FileDto,
) -> Result<FileDto> {
    if file.is_image {
        if let Some(versions) = &file.img_versions {
            let mut updated_versions: Vec<ImgVersionDto> = Vec::with_capacity(versions.len());
            for version in versions.iter() {
                let url = generate_url(
                    client,
                    bucket_name,
                    &format!(
                        "{}/{}/{}",
                        dir_name,
                        version.version.to_string(),
                        file.filename
                    ),
                )
                .await?;
                let mut version_copy = version.clone();
                version_copy.url = Some(url);
                updated_versions.push(version_copy);
            }
            if updated_versions.len() > 0 {
                file.img_versions = Some(updated_versions);
            }
        }
    } else {
        let url = generate_url(
            client,
            bucket_name,
            &format!("{}/{}/{}", dir_name, ORIGINAL_PATH, file.filename),
        )
        .await?;
        file.url = Some(url);
    }

    Ok(file)
}

async fn generate_url(client: &Client, bucket_name: &str, file_path: &str) -> Result<String> {
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
                    Err(Error::ValidationError(gerr.message))
                } else {
                    Err(format!("Google error: {}", gerr.message).as_str().into())
                }
            }
            _ => Err("Failed to list buckets from cloud storage.".into()),
        },
    }
}
