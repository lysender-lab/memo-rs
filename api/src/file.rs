use chrono::{DateTime, NaiveDateTime};
use exif::{In, Tag};
use image::DynamicImage;
use image::ImageReader;
use image::imageops;
use snafu::ResultExt;
use std::fs::File;
use std::path::PathBuf;
use tracing::error;

use crate::Result;
use crate::error::DbSnafu;
use crate::error::{ExifInfoSnafu, StorageSnafu, UploadFileSnafu, ValidationSnafu};

use crate::state::AppState;
use db::file::FilePayload;
use db::file::MAX_FILES;
use memo::bucket::BucketDto;
use memo::dir::DirDto;
use memo::file::{
    ALLOWED_IMAGE_TYPES, FileDto, ImgDimension, ImgVersion, ImgVersionDto, MAX_DIMENSION,
    MAX_PREVIEW_DIMENSION, MAX_THUMB_DIMENSION, ORIGINAL_PATH,
};
use memo::utils::generate_id;
use memo::utils::truncate_string;

#[derive(Debug, Clone)]
pub struct PhotoExif {
    pub orientation: u32,
    pub img_taken_at: Option<i64>,
}

impl Default for PhotoExif {
    fn default() -> Self {
        Self {
            orientation: 1,
            img_taken_at: None,
        }
    }
}

pub async fn create_file(
    state: AppState,
    bucket: &BucketDto,
    dir: &DirDto,
    data: &FilePayload,
) -> Result<FileDto> {
    let mut file_dto = init_file(dir, data)?;

    let cleanup = |data: &FilePayload, file: Option<&FileDto>| {
        if let Err(e) = cleanup_temp_uploads(data, file) {
            error!("Cleanup file(s): {}", e);
        }
    };

    if bucket.images_only && !file_dto.is_image {
        cleanup(data, None);

        return ValidationSnafu {
            msg: "Bucket only accepts images".to_string(),
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
            msg: "Directory already has files".to_string(),
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

    if file_dto.is_image {
        let exif_info = parse_exif_info(&data.path).unwrap_or_default();

        match create_versions(data, &exif_info) {
            Ok(versions) => {
                if !versions.is_empty() {
                    file_dto.img_versions = Some(versions);
                }
            }
            Err(e) => {
                cleanup(data, None);
                return Err(e);
            }
        };

        file_dto.img_taken_at = exif_info.img_taken_at;
    }

    if let Err(upload_err) = state
        .storage_client
        .upload_object(bucket, dir, &data.upload_dir, &file_dto)
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
        .create(file_dto.clone())
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
                .update_timestamp(&dir.id, today)
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

fn cleanup_temp_uploads(data: &FilePayload, file: Option<&FileDto>) -> Result<()> {
    if let Some(file) = file {
        if file.is_image {
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

fn init_file(dir: &DirDto, data: &FilePayload) -> Result<FileDto> {
    let mut is_image = false;
    let content_type = get_content_type(&data.path)?;
    if content_type.starts_with("image/") {
        if !ALLOWED_IMAGE_TYPES.contains(&content_type.as_str()) {
            if let Err(e) = cleanup_temp_uploads(data, None) {
                error!("Cleanup orig file: {}", e);
            }
            return Err("Uploaded image type not allowed".into());
        }
        is_image = true;
    }

    // May be a few second delayed due to image processing
    let today = chrono::Utc::now().timestamp();

    let file = FileDto {
        id: generate_id(),
        dir_id: dir.id.clone(),
        name: data.name.clone(),
        filename: data.filename.clone(),
        content_type,
        size: data.size,
        url: None,
        is_image,
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

fn create_versions(data: &FilePayload, exif_info: &PhotoExif) -> Result<Vec<ImgVersionDto>> {
    let img = read_image(&data.path)?;

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

    // // Only create preview if original image has side longer than max
    if source_width > MAX_DIMENSION || source_height > MAX_DIMENSION {
        let preview = create_preview(data, &rotated_img)?;
        versions.push(preview);
    }

    // Create thumbnail
    let thumb = create_thumbnail(data, &rotated_img)?;
    versions.push(thumb);

    Ok(versions)
}

fn create_preview(data: &FilePayload, img: &DynamicImage) -> Result<ImgVersionDto> {
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

    if let Err(err) = resized_img.save(dest_file) {
        return Err(format!("Unable to save preview: {}", err).into());
    }

    Ok(version)
}

fn create_thumbnail(data: &FilePayload, img: &DynamicImage) -> Result<ImgVersionDto> {
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

    if let Err(err) = resized_img.save(dest_file) {
        return Err(format!("Unable to save preview: {}", err).into());
    }

    Ok(version)
}

fn get_content_type(path: &PathBuf) -> Result<String> {
    match infer::get_from_path(path) {
        Ok(Some(kind)) => Ok(kind.mime_type().to_string()),
        Ok(None) => Err("Uploaded file type unknown".into()),
        Err(_) => Err("Unable to read uploaded file".into()),
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
