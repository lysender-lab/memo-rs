use std::fs::File;
use std::path::PathBuf;

use chrono::{DateTime, NaiveDateTime};
use exif::{In, Tag};
use image::DynamicImage;
use image::ImageReader;
use image::imageops;
use snafu::ResultExt;
use tracing::error;

use crate::error::DbSnafu;
use crate::error::TokioJoinSnafu;
use crate::error::{ExifInfoSnafu, StorageSnafu, UploadFileSnafu, ValidationSnafu};
use crate::state::AppState;
use crate::token::FileUploadClaims;
use crate::token::create_upload_token;
use crate::{Error, Result};
use db::file::MAX_FILES;
use memo::dir::DirDto;
use memo::dir::DirMeta;
use memo::dir::DirType;
use memo::file::MAX_FILE_SIZE;
use memo::file::RemoteUploadDto;
use memo::file::SignedFileUploadDto;
use memo::file::{
    ALLOWED_IMAGE_TYPES, FileDto, ImgDimension, ImgVersion, ImgVersionDto, MAX_DIMENSION,
    MAX_PREVIEW_DIMENSION, MAX_THUMB_DIMENSION, ORIGINAL_PATH,
};
use memo::file::{FileStatus, FileType};
use memo::utils::IdPrefix;
use memo::utils::generate_prefixed_id;
use memo::utils::slugify_prefixed;
use memo::utils::truncate_string;
use storage::DownloadedFile;

#[derive(Debug, Clone)]
pub struct PhotoExif {
    pub orientation: u32,
    pub img_taken_at: Option<i64>,
}

/// Process a previously reserved image record.
pub async fn process_image_svc(state: &AppState, file_id: &str) -> Result<()> {
    let Some(mut file) = state.db.files.get(file_id).await.context(DbSnafu)? else {
        return Ok(());
    };
    if file.file_type != FileType::Image || file.status != FileStatus::Processing {
        return Ok(());
    }
    let Some(dir) = state.db.dirs.get(&file.dir_id).await.context(DbSnafu)? else {
        return Ok(());
    };
    let dir_meta = DirMeta {
        bucket_name: state.config.cloud.bucket.clone(),
        org_id: dir.org_id,
        dir_type: dir.dir_type,
        dir_name: dir.name,
    };
    // Each attempt uses its own directory, including when a lease is lost.
    let temp_dir = state
        .config
        .upload_dir
        .join(format!("image-job-{}", memo::utils::generate_id()));
    let result = async {
        let downloaded = state
            .storage_client
            .download(
                &dir_meta,
                ORIGINAL_PATH,
                &file.name,
                &file.filename,
                &file.content_type,
                &temp_dir,
            )
            .await
            .context(StorageSnafu)?;
        let data = downloaded.clone();
        let (versions, taken_at, content_type) = tokio::task::spawn_blocking(move || {
            let content_type = get_content_type(&data.path).unwrap_or(data.content_type.clone());
            let exif = parse_exif_info(&data.path).unwrap_or_default();
            let versions = create_versions(&data, &exif)?;
            Ok::<_, Error>((versions, exif.img_taken_at, content_type))
        })
        .await
        .context(TokioJoinSnafu)??;
        file.img_versions = Some(versions);
        file.img_taken_at = taken_at;
        file.content_type = content_type;
        file.size = downloaded.size;

        if downloaded.content_type == "image/avif" && file.content_type == "image/jpeg" {
            state
                .storage_client
                .reupload_original(
                    &dir_meta,
                    &file.filename,
                    &downloaded.path,
                    &file.content_type,
                )
                .await
                .context(StorageSnafu)?;
        }
        state
            .storage_client
            .upload(&dir_meta, &temp_dir, &file)
            .await
            .context(StorageSnafu)?;
        state
            .db
            .files
            .finish_image_processing(&file)
            .await
            .context(DbSnafu)?;
        state.file_cache.remove(file_id);
        Ok(())
    }
    .await;
    if let Err(err) = tokio::fs::remove_dir_all(&temp_dir).await
        && err.kind() != std::io::ErrorKind::NotFound
    {
        error!("Unable to clean image job temporary directory: {}", err);
    }
    result
}

impl Default for PhotoExif {
    fn default() -> Self {
        Self {
            orientation: 1,
            img_taken_at: None,
        }
    }
}

pub async fn generate_upload_url_svc(
    state: AppState,
    dir: &DirDto,
    data: &RemoteUploadDto,
) -> Result<SignedFileUploadDto> {
    if data.size > MAX_FILE_SIZE {
        return Err(Error::Validation {
            msg: "File size exceeds maximum allowed".to_string(),
        });
    }

    let dir_meta = DirMeta {
        bucket_name: state.config.cloud.bucket,
        org_id: dir.org_id.clone(),
        dir_type: dir.dir_type.clone(),
        dir_name: dir.name.clone(),
    };

    // Limit the number of files per dir
    let count = state
        .db
        .files
        .count_by_dir(&dir.id)
        .await
        .context(DbSnafu)?;

    if count >= MAX_FILES as i64 {
        return Err(Error::Validation {
            msg: "Directory already has maximum files".to_string(),
        });
    }

    // Name must be unique for the dir (not filename)
    if state
        .db
        .files
        .find_by_name(&dir.id, &data.filename)
        .await
        .context(DbSnafu)?
        .is_some()
    {
        // Show error but ensure name is not too long
        let short_name = truncate_string(&data.filename, 20);
        return ValidationSnafu {
            msg: format!("{} already exists", short_name),
        }
        .fail();
    }

    // Low chance of collision but higher than the full uuid v7 string
    // Prefer a shorter filename for better readability
    let uniq_filename = slugify_prefixed(&data.filename);

    let token = create_upload_token(
        data.filename.clone(),
        uniq_filename.clone(),
        data.content_type.clone(),
        data.size,
        &state.config.jwt_secret,
    )?;

    let upload_url = state
        .storage_client
        .generate_upload_url(&dir_meta, ORIGINAL_PATH, &uniq_filename, &data.content_type)
        .await
        .context(StorageSnafu)?;

    Ok(SignedFileUploadDto {
        orig_filename: data.filename.clone(),
        new_filename: uniq_filename,
        content_type: data.content_type.clone(),
        size: data.size,
        url: upload_url,
        token,
    })
}

/// Creates a file record by downloading the file first, capture metadata
/// create image versions, then upload to cloud storage, and finally save to database
#[allow(dead_code)]
pub async fn create_file_svc(
    state: AppState,
    dir: &DirDto,
    data: &DownloadedFile,
) -> Result<FileDto> {
    let dir_meta = DirMeta {
        bucket_name: state.config.cloud.bucket,
        org_id: dir.org_id.clone(),
        dir_type: dir.dir_type.clone(),
        dir_name: dir.name.clone(),
    };

    let mut file_dto = init_file(dir, data)?;

    let cleanup = |data: &DownloadedFile, file: Option<&FileDto>| {
        if let Err(e) = cleanup_temp_uploads(data, file) {
            error!("Cleanup file(s): {}", e);
        }
    };

    if dir_meta.dir_type == DirType::Photos && file_dto.file_type != FileType::Image {
        cleanup(data, None);

        return ValidationSnafu {
            msg: "Only photos are allowed".to_string(),
        }
        .fail();
    }

    // Limit the number of files per dir
    let count = state
        .db
        .files
        .count_by_dir(&dir.id)
        .await
        .context(DbSnafu)?;

    if count >= MAX_FILES as i64 {
        cleanup(data, None);

        return ValidationSnafu {
            msg: "Directory already has maximum files".to_string(),
        }
        .fail();
    }

    // Name must be unique for the dir (not filename)
    if state
        .db
        .files
        .find_by_name(&dir.id, &data.name)
        .await
        .context(DbSnafu)?
        .is_some()
    {
        cleanup(data, None);

        // Show error but ensure name is not too long
        let short_name = truncate_string(&data.name, 20);
        return ValidationSnafu {
            msg: format!("{} already exists", short_name),
        }
        .fail();
    }

    if file_dto.file_type == FileType::Image {
        let data_copy = data.clone();

        // Process image in blocking task to avoid blocking the async runtime
        let versions_proc = tokio::task::spawn_blocking(move || {
            let exif_info = parse_exif_info(&data_copy.path).unwrap_or_default();
            let img_taken_at = exif_info.img_taken_at;

            let mut img_versions: Option<Vec<ImgVersionDto>> = None;

            match create_versions(&data_copy, &exif_info) {
                Ok(versions) => {
                    if !versions.is_empty() {
                        img_versions = Some(versions);
                    }
                }
                Err(e) => return Err(e),
            };

            Ok((img_versions, img_taken_at))
        });

        let versions_res = versions_proc.await.context(TokioJoinSnafu)?;
        match versions_res {
            Ok((img_versions, img_taken_at)) => {
                file_dto.img_versions = img_versions;
                file_dto.img_taken_at = img_taken_at;
            }
            Err(e) => {
                cleanup(data, None);
                return Err(e);
            }
        }
    }

    if data.content_type == "image/avif"
        && file_dto.content_type == "image/jpeg"
        && let Err(err) = state
            .storage_client
            .reupload_original(
                &dir_meta,
                &data.filename,
                &data.path,
                &file_dto.content_type,
            )
            .await
            .context(StorageSnafu)
    {
        cleanup(data, Some(&file_dto));
        return Err(err);
    }

    if let Err(upload_err) = state
        .storage_client
        .upload(&dir_meta, &data.upload_dir, &file_dto)
        .await
        .context(StorageSnafu)
    {
        cleanup(data, Some(&file_dto));
        return Err(upload_err);
    }

    // Save to database
    let create_res = state
        .db
        .files
        .retry_create(file_dto.clone(), 5)
        .await
        .context(DbSnafu);

    match create_res {
        Ok(file) => {
            cleanup(data, Some(&file_dto));

            // Also update dir
            let today = chrono::Utc::now().timestamp();
            let dir_result = state
                .db
                .dirs
                .retry_update_timestamp(&dir.id, today, 5)
                .await
                .context(DbSnafu);

            if let Err(e) = dir_result {
                // Can't afford to fail here, we will just log the error...
                error!("{}", e);
            }

            Ok(file)
        }
        Err(e) => {
            cleanup(data, Some(&file_dto));
            Err(e)
        }
    }
}

pub async fn delete_file_svc(state: AppState, dir_meta: &DirMeta, file: &FileDto) -> Result<()> {
    if file.file_type == FileType::Note {
        state.db.files.delete(&file.id).await.context(DbSnafu)?;
        state.file_cache.remove(&file.id);
        // Also delete all note revisions
        state
            .db
            .notes
            .delete_revisions(&file.id)
            .await
            .context(DbSnafu)?;
    } else {
        if !state
            .db
            .files
            .mark_inactive(&file.id)
            .await
            .context(DbSnafu)?
        {
            return ValidationSnafu {
                msg: "Only active files can be deleted".to_string(),
            }
            .fail();
        }
        state.file_cache.remove(&file.id);
        // Read after the transition so the payload includes completed image metadata.
        let current = state
            .db
            .files
            .get(&file.id)
            .await
            .context(DbSnafu)?
            .ok_or_else(|| Error::NotFound {
                msg: "File not found".to_string(),
            })?;
        crate::job::enqueue(
            &state.db,
            crate::job::delete_file_job(current, dir_meta.clone()),
        )
        .await
        .context(DbSnafu)?;
    }

    Ok(())
}

/// Creates a file record relying completely from upload metadata
/// Assumes that the file has been uploaded already
pub async fn create_remote_file_svc(
    state: AppState,
    dir: &DirDto,
    data: &FileUploadClaims,
) -> Result<FileDto> {
    let today = chrono::Utc::now().timestamp();

    let is_image = data.is_image();
    if dir.dir_type == DirType::Photos && !is_image {
        return ValidationSnafu {
            msg: "Only photos are allowed".to_string(),
        }
        .fail();
    }
    let file = FileDto {
        id: generate_prefixed_id(IdPrefix::File),
        org_id: dir.org_id.clone(),
        dir_id: dir.id.clone(),
        file_type: if is_image {
            FileType::Image
        } else {
            FileType::File
        },
        status: if is_image {
            FileStatus::Processing
        } else {
            FileStatus::Active
        },
        name: data.orig_filename.clone(),
        filename: data.new_filename.clone(),
        content_type: data.content_type.clone(),
        size: data.size,
        url: None,
        img_versions: None,
        img_taken_at: None,
        created_at: today,
        updated_at: today,
    };

    // Limit the number of files per dir
    let count = state
        .db
        .files
        .count_by_dir(&dir.id)
        .await
        .context(DbSnafu)?;

    if count >= MAX_FILES as i64 {
        return Err(Error::Validation {
            msg: "Directory already has maximum files".to_string(),
        });
    }

    // Name must be unique for the dir (not filename)
    if state
        .db
        .files
        .find_by_name(&dir.id, &data.orig_filename)
        .await
        .context(DbSnafu)?
        .is_some()
    {
        // Show error but ensure name is not too long
        let short_name = truncate_string(&data.orig_filename, 20);
        return Err(Error::Validation {
            msg: format!("{} already exists", short_name),
        });
    }

    // Save to database
    let create_res = state.db.files.retry_create(file, 5).await.context(DbSnafu);

    match create_res {
        Ok(file) => {
            // Also update dir
            let today = chrono::Utc::now().timestamp();
            let dir_result = state
                .db
                .dirs
                .retry_update_timestamp(&dir.id, today, 5)
                .await
                .context(DbSnafu);

            if let Err(e) = dir_result {
                // Can't afford to fail here, we will just log the error...
                error!("{}", e);
            }

            Ok(file)
        }
        Err(e) => Err(e),
    }
}

fn cleanup_temp_uploads(data: &DownloadedFile, file: Option<&FileDto>) -> Result<()> {
    if let Some(file) = file {
        if file.file_type == FileType::Image {
            // Cleanup versions
            if let Some(versions) = &file.img_versions {
                let mut errors: Vec<String> = Vec::new();
                for version in versions.iter() {
                    let source_file = version.to_path(&data.upload_dir, &file.filename);
                    // Collect errors, can't afford to stop here
                    if let Err(err) = std::fs::remove_file(&source_file) {
                        errors.push(format!("Unable to remove file after upload: {}", err));
                    }
                }

                if !errors.is_empty() {
                    return Err(errors.join(", ").as_str().into());
                }
            }
        } else {
            // Cleanup original file
            let upload_dir = data.upload_dir.clone();
            let source_file = upload_dir.join(ORIGINAL_PATH).join(&file.filename);
            if let Err(err) = std::fs::remove_file(&source_file) {
                return Err(format!("Unable to remove file after upload: {}", err).into());
            }
        }
    } else {
        // Full data not available, just cleanup the original
        let upload_dir = data.upload_dir.clone();
        let source_file = upload_dir.join(ORIGINAL_PATH).join(&data.filename);
        if let Err(err) = std::fs::remove_file(&source_file) {
            return Err(format!("Unable to remove file after upload: {}", err).into());
        }
    }

    Ok(())
}

fn init_file(dir: &DirDto, data: &DownloadedFile) -> Result<FileDto> {
    // Current design limits file_type to either File or Image
    // In the future, there will be a Video and Note
    let mut file_type = FileType::File;

    // Try to get content type from file, fallback to the one from upload metadata
    let content_type = get_content_type(&data.path).unwrap_or(data.content_type.clone());

    if ALLOWED_IMAGE_TYPES.contains(&content_type.as_str()) {
        file_type = FileType::Image;
    }

    // May be a few second delayed due to image processing
    let today = chrono::Utc::now().timestamp();

    let file = FileDto {
        id: generate_prefixed_id(IdPrefix::File),
        org_id: dir.org_id.clone(),
        dir_id: dir.id.clone(),
        file_type,
        status: FileStatus::Active,
        name: data.name.clone(),
        filename: data.filename.clone(),
        content_type,
        size: data.size,
        url: None,
        img_versions: None,
        img_taken_at: None,
        created_at: today,
        updated_at: today,
    };

    Ok(file)
}

fn read_image(path: &PathBuf) -> Result<DynamicImage> {
    match ImageReader::open(path) {
        Ok(read_img) => match read_img.with_guessed_format() {
            Ok(format_img) => match format_img.decode() {
                Ok(img) => Ok(img),
                Err(e) => {
                    let msg = format!("Unable to decode image: {}", e);
                    error!("{}", msg);
                    Err(msg.as_str().into())
                }
            },
            Err(e) => {
                let msg = format!("Unable to guess image format: {}", e);
                error!("{}", msg);
                Err(msg.as_str().into())
            }
        },
        Err(e) => {
            let msg = format!("Unable to read image: {}", e);
            error!("{}", msg);
            Err(msg.as_str().into())
        }
    }
}

fn create_versions(data: &DownloadedFile, exif_info: &PhotoExif) -> Result<Vec<ImgVersionDto>> {
    let img = read_image(&data.path)?;
    let content_type = get_content_type(&data.path);
    let jpeg_versions = matches!(content_type.as_deref(), Some("image/webp" | "image/avif"));
    let encode_jpeg = jpeg_versions || content_type.as_deref() == Some("image/jpeg");

    // Rotate based on exif orientation before creating versions
    let rotated_img = match exif_info.orientation {
        8 => img.rotate270(),
        7 => img.rotate270().fliph(),
        6 => img.rotate90(),
        5 => img.rotate90().fliph(),
        4 => img.flipv(),
        3 => img.rotate180(),
        2 => img.fliph(),
        _ => img,
    };

    let source_width = rotated_img.width();
    let source_height = rotated_img.height();

    let orig_version = ImgVersionDto {
        version: ImgVersion::Original,
        dimension: ImgDimension {
            width: source_width,
            height: source_height,
        },
        url: None,
    };

    let mut versions: Vec<ImgVersionDto> = vec![orig_version];

    // JPEG has no alpha channel; composite before resizing to avoid dark edges.
    let version_img = if jpeg_versions {
        let mut rgba = rotated_img.into_rgba8();
        for pixel in rgba.pixels_mut() {
            let alpha = u32::from(pixel[3]);
            for channel in &mut pixel.0[..3] {
                *channel = ((u32::from(*channel) * alpha + 255 * (255 - alpha) + 127) / 255) as u8;
            }
            pixel[3] = 255;
        }
        DynamicImage::ImageRgba8(rgba)
    } else {
        rotated_img
    };

    // These formats always need a JPEG preview, even when the original is small.
    if jpeg_versions || source_width > MAX_DIMENSION || source_height > MAX_DIMENSION {
        let preview = create_preview(data, &version_img, encode_jpeg)?;
        versions.push(preview);
    }

    // Create thumbnail
    let thumb = create_thumbnail(data, &version_img, encode_jpeg)?;
    versions.push(thumb);

    Ok(versions)
}

fn create_preview(
    data: &DownloadedFile,
    img: &DynamicImage,
    jpeg_versions: bool,
) -> Result<ImgVersionDto> {
    // Prepare dir
    let prev_dir = data
        .upload_dir
        .clone()
        .join(ImgVersion::Preview.to_string());

    if let Err(err) = std::fs::create_dir_all(&prev_dir) {
        return Err(format!("Unable to create preview dir: {}", err).into());
    }

    // Either resize to max dimension or original dimension
    // whichever is smaller
    let mut max_width = MAX_PREVIEW_DIMENSION;
    if img.width() < MAX_PREVIEW_DIMENSION {
        max_width = img.width();
    }
    let mut max_height = MAX_PREVIEW_DIMENSION;
    if img.height() < MAX_PREVIEW_DIMENSION {
        max_height = img.height();
    }

    let resized_img = img.resize(max_width, max_height, imageops::FilterType::Lanczos3);

    // Save the resized image
    let version = ImgVersionDto {
        version: ImgVersion::Preview,
        dimension: ImgDimension {
            width: resized_img.width(),
            height: resized_img.height(),
        },
        url: None,
    };

    let dest_file = version.to_path(&data.upload_dir, &data.filename);

    let save_result = if jpeg_versions {
        resized_img.save_with_format(dest_file, image::ImageFormat::Jpeg)
    } else {
        resized_img.save(dest_file)
    };
    if let Err(err) = save_result {
        return Err(format!("Unable to save preview: {}", err).into());
    }

    Ok(version)
}

fn create_thumbnail(
    data: &DownloadedFile,
    img: &DynamicImage,
    jpeg_versions: bool,
) -> Result<ImgVersionDto> {
    // Prepare dir
    let prev_dir = data
        .upload_dir
        .clone()
        .join(ImgVersion::Thumbnail.to_string());

    if let Err(err) = std::fs::create_dir_all(&prev_dir) {
        return Err(format!("Unable to create preview dir: {}", err).into());
    }

    // Either resize to max dimension or original dimension
    // whichever is smaller
    let mut max_width = MAX_THUMB_DIMENSION;
    if img.width() < MAX_THUMB_DIMENSION {
        max_width = img.width();
    }
    let mut max_height = MAX_THUMB_DIMENSION;
    if img.height() < MAX_THUMB_DIMENSION {
        max_height = img.height();
    }

    let resized_img = img.resize(max_width, max_height, imageops::FilterType::Lanczos3);

    // Save the resized image
    let version = ImgVersionDto {
        version: ImgVersion::Thumbnail,
        dimension: ImgDimension {
            width: resized_img.width(),
            height: resized_img.height(),
        },
        url: None,
    };

    let dest_file = version.to_path(&data.upload_dir, &data.filename);

    let save_result = if jpeg_versions {
        resized_img.save_with_format(dest_file, image::ImageFormat::Jpeg)
    } else {
        resized_img.save(dest_file)
    };
    if let Err(err) = save_result {
        return Err(format!("Unable to save preview: {}", err).into());
    }

    Ok(version)
}

fn get_content_type(path: &PathBuf) -> Option<String> {
    match infer::get_from_path(path) {
        Ok(Some(kind)) => Some(kind.mime_type().to_string()),
        Ok(None) => None,
        Err(_) => None,
    }
}

fn parse_exif_info(path: &PathBuf) -> Result<PhotoExif> {
    let file = File::open(path).context(UploadFileSnafu)?;

    let mut buf_reader = std::io::BufReader::new(&file);
    let exit_reader = exif::Reader::new();
    let exif = exit_reader
        .read_from_container(&mut buf_reader)
        .context(ExifInfoSnafu)?;

    // Default to 1 if cannot identify orientation
    let orientation = match exif.get_field(Tag::Orientation, In::PRIMARY) {
        Some(orientation) => orientation.value.get_uint(0).unwrap_or(1),
        None => 1,
    };

    let mut taken_at: Option<i64> = None;

    if let Some(date_time) = exif.get_field(Tag::DateTimeOriginal, In::PRIMARY) {
        let naive_str = date_time.display_value().to_string();

        if let Some(offset_field) = exif.get_field(Tag::OffsetTimeOriginal, In::PRIMARY) {
            // For some reason, it is wrapped in quotes
            let offset_str = offset_field.display_value().to_string().replace("\"", "");

            // Combine datetime and offset to build the actual time
            let date_str = format!("{} {}", naive_str, offset_str);
            if let Ok(dt) = DateTime::parse_from_str(&date_str, "%Y-%m-%d %H:%M:%S %z") {
                taken_at = Some(dt.timestamp());
            }
        } else {
            // No timezone info so we will just incorrectly assume its UTC
            // I want it Philippine time but hey, someone else on the other side
            // of the world may use this right?
            if let Ok(dt) = NaiveDateTime::parse_from_str(&naive_str, "%Y-%m-%d %H:%M:%S") {
                taken_at = Some(dt.and_utc().timestamp());
            }
        }
    }

    Ok(PhotoExif {
        orientation,
        img_taken_at: taken_at,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn jpeg_with_avif_filename_generates_jpeg_versions() {
        for (width, height) in [(20, 10), (2100, 100)] {
            let upload_dir = std::env::temp_dir().join(generate_prefixed_id(IdPrefix::File));
            std::fs::create_dir_all(&upload_dir).unwrap();
            let filename = "source.avif".to_string();
            let path = upload_dir.join(&filename);
            DynamicImage::new_rgb8(width, height)
                .save_with_format(&path, image::ImageFormat::Jpeg)
                .unwrap();
            let original = std::fs::read(&path).unwrap();
            assert_eq!(get_content_type(&path).as_deref(), Some("image/jpeg"));
            let data = DownloadedFile {
                upload_dir: upload_dir.clone(),
                name: filename.clone(),
                filename,
                content_type: "image/avif".into(),
                path: path.clone(),
                size: original.len() as i64,
            };

            let versions = create_versions(&data, &PhotoExif::default()).unwrap();
            assert_eq!(versions.len(), if width > MAX_DIMENSION { 3 } else { 2 });
            for version in &versions[1..] {
                let bytes = std::fs::read(version.to_path(&upload_dir, &data.filename)).unwrap();
                assert_eq!(
                    image::guess_format(&bytes).unwrap(),
                    image::ImageFormat::Jpeg
                );
                let decoded = image::load_from_memory(&bytes).unwrap();
                assert_eq!(decoded.width(), version.dimension.width);
                assert_eq!(decoded.height(), version.dimension.height);
            }
            assert_eq!(std::fs::read(path).unwrap(), original);
            std::fs::remove_dir_all(upload_dir).unwrap();
        }
    }

    #[test]
    fn webp_and_avif_versions_are_jpeg_and_preserve_original() {
        for (extension, content_type, format) in [
            ("webp", "image/webp", image::ImageFormat::WebP),
            ("avif", "image/avif", image::ImageFormat::Avif),
        ] {
            for (width, height) in [(20, 10), (2100, 100)] {
                let upload_dir = std::env::temp_dir().join(generate_prefixed_id(IdPrefix::File));
                std::fs::create_dir_all(&upload_dir).unwrap();
                let filename = format!("source.{extension}");
                let path = upload_dir.join(&filename);
                let image = DynamicImage::ImageRgba8(image::RgbaImage::from_pixel(
                    width,
                    height,
                    image::Rgba([0, 0, 0, 0]),
                ));
                image.save_with_format(&path, format).unwrap();
                let original = std::fs::read(&path).unwrap();
                let data = DownloadedFile {
                    upload_dir: upload_dir.clone(),
                    name: filename.clone(),
                    filename,
                    content_type: content_type.into(),
                    path: path.clone(),
                    size: original.len() as i64,
                };

                let versions = create_versions(&data, &PhotoExif::default()).unwrap();
                assert_eq!(versions.len(), 3);
                for (version, limit) in [
                    (&versions[1], MAX_PREVIEW_DIMENSION),
                    (&versions[2], MAX_THUMB_DIMENSION),
                ] {
                    let bytes =
                        std::fs::read(version.to_path(&upload_dir, &data.filename)).unwrap();
                    assert_eq!(
                        image::guess_format(&bytes).unwrap(),
                        image::ImageFormat::Jpeg
                    );
                    let decoded = image::load_from_memory(&bytes).unwrap().to_rgb8();
                    assert_eq!(decoded.width(), version.dimension.width);
                    assert_eq!(decoded.height(), version.dimension.height);
                    assert!(decoded.width() <= width.min(limit));
                    assert!(decoded.height() <= height.min(limit));
                    assert!(
                        decoded
                            .get_pixel(0, 0)
                            .0
                            .iter()
                            .all(|channel| *channel >= 250)
                    );
                }
                assert_eq!(std::fs::read(path).unwrap(), original);
                std::fs::remove_dir_all(upload_dir).unwrap();
            }
        }
    }
}
