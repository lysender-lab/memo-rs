use async_trait::async_trait;

use chrono::{DateTime, NaiveDateTime};
use deadpool_diesel::sqlite::Pool;
use diesel::dsl::count_star;
use diesel::prelude::*;
use diesel::{QueryDsl, SelectableHelper};
use exif::{In, Tag};
use image::DynamicImage;
use image::ImageReader;
use image::imageops;
use serde::{Deserialize, Serialize};
use snafu::{ResultExt, ensure};
use std::fs::File;
use std::path::PathBuf;
use tracing::error;
use validator::Validate;

use crate::Result;
use crate::dir::Dir;
use crate::error::{
    DbInteractSnafu, DbPoolSnafu, DbQuerySnafu, ExifInfoSnafu, UploadFileSnafu, ValidationSnafu,
};

use crate::schema::files::{self, dsl};
use crate::state::AppState;
use memo::bucket::BucketDto;
use memo::file::{FileDto, ImgDimension, ImgVersion, ImgVersionDto};
use memo::pagination::Paginated;
use memo::utils::generate_id;
use memo::utils::truncate_string;
use memo::validators::flatten_errors;

pub const ORIGINAL_PATH: &str = "orig";
pub const ALLOWED_IMAGE_TYPES: [&str; 4] = ["image/jpeg", "image/pjpeg", "image/png", "image/gif"];

/// Maximum image dimension before creating a preview version
pub const MAX_DIMENSION: u32 = 1000;
pub const MAX_PREVIEW_DIMENSION: u32 = 2000;
pub const MAX_THUMB_DIMENSION: u32 = 200;

#[derive(Debug, Clone, Queryable, Selectable, Insertable, Serialize)]
#[diesel(table_name = crate::schema::files)]
#[diesel(check_for_backend(diesel::sqlite::Sqlite))]
pub struct FileObject {
    pub id: String,
    pub dir_id: String,
    pub name: String,
    pub filename: String,
    pub content_type: String,
    pub size: i64,
    pub is_image: i32,
    pub img_versions: Option<String>,
    pub img_taken_at: Option<i64>,
    pub created_at: i64,
    pub updated_at: i64,
}

#[derive(Debug, Clone)]
pub struct FilePayload {
    pub upload_dir: PathBuf,
    pub name: String,
    pub filename: String,
    pub path: PathBuf,
    pub size: i64,
}

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

#[derive(Debug, Clone, Deserialize, Validate)]
pub struct ListFilesParams {
    #[validate(range(min = 1, max = 1000))]
    pub page: Option<i32>,

    #[validate(range(min = 1, max = 50))]
    pub per_page: Option<i32>,

    #[validate(length(min = 0, max = 50))]
    pub keyword: Option<String>,
}

/// Convert FileDto to File
impl From<FileDto> for FileObject {
    fn from(file: FileDto) -> Self {
        let img_versions = match file.img_versions {
            Some(versions) => {
                let versions_str: String = versions
                    .iter()
                    .map(|v| v.to_string())
                    .collect::<Vec<String>>()
                    .join(",");

                Some(versions_str)
            }
            None => None,
        };

        Self {
            id: file.id,
            dir_id: file.dir_id,
            name: file.name,
            filename: file.filename,
            content_type: file.content_type,
            size: file.size,
            is_image: if file.is_image { 1 } else { 0 },
            img_versions,
            img_taken_at: file.img_taken_at,
            created_at: file.created_at,
            updated_at: file.updated_at,
        }
    }
}

/// Convert File to FileDto
impl From<FileObject> for FileDto {
    fn from(file: FileObject) -> Self {
        let img_versions = match file.img_versions {
            Some(versions_str) => {
                let versions: Vec<ImgVersionDto> = versions_str
                    .split(',')
                    .filter_map(|s| s.parse::<ImgVersionDto>().ok())
                    .collect();

                if versions.len() > 0 {
                    Some(versions)
                } else {
                    None
                }
            }
            None => None,
        };

        Self {
            id: file.id,
            dir_id: file.dir_id,
            name: file.name,
            filename: file.filename,
            content_type: file.content_type,
            size: file.size,
            is_image: file.is_image == 1,
            img_versions,
            img_taken_at: file.img_taken_at,
            url: None,
            created_at: file.created_at,
            updated_at: file.updated_at,
        }
    }
}

const MAX_PER_PAGE: i32 = 50;
const MAX_FILES: i32 = 1000;

pub async fn create_file(
    state: AppState,
    bucket: &BucketDto,
    dir: &Dir,
    data: &FilePayload,
) -> Result<FileObject> {
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
    let count = state.db.files.count_by_dir(&dir.id).await?;
    if count >= MAX_FILES as i64 {
        cleanup(data, None);

        return ValidationSnafu {
            msg: "Directory already has files".to_string(),
        }
        .fail();
    }

    // Name must be unique for the dir (not filename)
    if let Some(_) = state.db.files.find_by_name(&dir.id, &data.name).await? {
        cleanup(data, None);

        // Show error but ensure name is not too long
        let short_name = truncate_string(&data.name, 20);
        return ValidationSnafu {
            msg: format!("{} already exists", short_name),
        }
        .fail();
    }

    if file_dto.is_image {
        let exif_info = match parse_exif_info(&data.path) {
            Ok(info) => info,
            Err(_) => {
                // It's okay to continue without exif info
                PhotoExif::default()
            }
        };

        match create_versions(data, &exif_info) {
            Ok(versions) => {
                if versions.len() > 0 {
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
    {
        cleanup(data, Some(&file_dto));
        return Err(upload_err);
    }

    // Save to database
    let create_res = state.db.files.create(file_dto.clone()).await;
    match create_res {
        Ok(file) => {
            cleanup(data, Some(&file_dto));

            // Also update dir
            let today = chrono::Utc::now().timestamp();
            let dir_result = state.db.dirs.update_timestamp(&dir.id, today).await;
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

#[async_trait]
pub trait FileRepoable: Send + Sync {
    async fn list(&self, dir: &Dir, params: &ListFilesParams) -> Result<Paginated<FileObject>>;

    async fn create(&self, file_dto: FileDto) -> Result<FileObject>;

    async fn get(&self, id: &str) -> Result<Option<FileObject>>;

    async fn find_by_name(&self, dir_id: &str, name: &str) -> Result<Option<FileObject>>;

    async fn count_by_dir(&self, dir_id: &str) -> Result<i64>;

    async fn delete(&self, id: &str) -> Result<()>;
}

pub struct FileRepo {
    db_pool: Pool,
}

impl FileRepo {
    pub fn new(db_pool: Pool) -> Self {
        Self { db_pool }
    }

    pub async fn listing_count(&self, dir_id: &str, params: &ListFilesParams) -> Result<i64> {
        let db = self.db_pool.get().await.context(DbPoolSnafu)?;

        let did = dir_id.to_string();
        let params_copy = params.clone();

        let count_res = db
            .interact(move |conn| {
                let mut query = dsl::files.into_boxed();
                query = query.filter(dsl::dir_id.eq(did.as_str()));
                if let Some(keyword) = params_copy.keyword {
                    if keyword.len() > 0 {
                        let pattern = format!("%{}%", keyword);
                        query = query.filter(dsl::name.like(pattern));
                    }
                }
                query.select(count_star()).get_result::<i64>(conn)
            })
            .await
            .context(DbInteractSnafu)?;

        let count = count_res.context(DbQuerySnafu {
            table: "files".to_string(),
        })?;

        Ok(count)
    }
}

#[async_trait]
impl FileRepoable for FileRepo {
    async fn list(&self, dir: &Dir, params: &ListFilesParams) -> Result<Paginated<FileObject>> {
        let errors = params.validate();
        ensure!(
            errors.is_ok(),
            ValidationSnafu {
                msg: flatten_errors(&errors.unwrap_err()),
            }
        );

        let db = self.db_pool.get().await.context(DbPoolSnafu)?;

        let did = dir.id.clone();

        let total_records = self.listing_count(&dir.id, params).await?;
        let mut page: i32 = 1;
        let mut per_page: i32 = MAX_PER_PAGE;
        let mut offset: i64 = 0;

        if let Some(per_page_param) = params.per_page {
            if per_page_param > 0 && per_page_param <= MAX_PER_PAGE {
                per_page = per_page_param;
            }
        }

        let total_pages: i64 = (total_records as f64 / per_page as f64).ceil() as i64;

        if let Some(p) = params.page {
            let p64 = p as i64;
            if p64 > 0 && p64 <= total_pages {
                page = p;
                offset = (p64 - 1) * per_page as i64;
            }
        }

        // Do not query if we already know there are no records
        if total_pages == 0 {
            return Ok(Paginated::new(Vec::new(), page, per_page, total_records));
        }

        let params_copy = params.clone();
        let select_res = db
            .interact(move |conn| {
                let mut query = dsl::files.into_boxed();
                query = query.filter(dsl::dir_id.eq(did.as_str()));

                if let Some(keyword) = params_copy.keyword {
                    if keyword.len() > 0 {
                        let pattern = format!("%{}%", keyword);
                        query = query.filter(dsl::name.like(pattern));
                    }
                }
                query
                    .limit(per_page as i64)
                    .offset(offset)
                    .select(FileObject::as_select())
                    .order(dsl::created_at.desc())
                    .load::<FileObject>(conn)
            })
            .await
            .context(DbInteractSnafu)?;

        let items = select_res.context(DbQuerySnafu {
            table: "files".to_string(),
        })?;

        Ok(Paginated::new(items, page, per_page, total_records))
    }

    async fn create(&self, file_dto: FileDto) -> Result<FileObject> {
        let file_db_pool = self.db_pool.clone();
        let db = file_db_pool.get().await.context(DbPoolSnafu)?;

        let file: FileObject = file_dto.clone().into();
        let file_copy = file.clone();

        let insert_res = db
            .interact(move |conn| {
                diesel::insert_into(files::table)
                    .values(&file_copy)
                    .execute(conn)
            })
            .await
            .context(DbInteractSnafu)?;

        let _ = insert_res.context(DbQuerySnafu {
            table: "files".to_string(),
        })?;

        Ok(file)
    }

    async fn get(&self, id: &str) -> Result<Option<FileObject>> {
        let db = self.db_pool.get().await.context(DbPoolSnafu)?;

        let fid = id.to_string();
        let select_res = db
            .interact(move |conn| {
                dsl::files
                    .find(fid)
                    .select(FileObject::as_select())
                    .first::<FileObject>(conn)
                    .optional()
            })
            .await
            .context(DbInteractSnafu)?;

        let item = select_res.context(DbQuerySnafu {
            table: "files".to_string(),
        })?;

        Ok(item)
    }

    async fn find_by_name(&self, dir_id: &str, name: &str) -> Result<Option<FileObject>> {
        let db = self.db_pool.get().await.context(DbPoolSnafu)?;

        let did = dir_id.to_string();
        let name_copy = name.to_string();
        let select_res = db
            .interact(move |conn| {
                dsl::files
                    .filter(dsl::dir_id.eq(did.as_str()))
                    .filter(dsl::name.eq(name_copy.as_str()))
                    .select(FileObject::as_select())
                    .first::<FileObject>(conn)
                    .optional()
            })
            .await
            .context(DbInteractSnafu)?;

        let item = select_res.context(DbQuerySnafu {
            table: "files".to_string(),
        })?;

        Ok(item)
    }

    async fn count_by_dir(&self, dir_id: &str) -> Result<i64> {
        let db = self.db_pool.get().await.context(DbPoolSnafu)?;

        let did = dir_id.to_string();
        let count_res = db
            .interact(move |conn| {
                dsl::files
                    .filter(dsl::dir_id.eq(did.as_str()))
                    .select(count_star())
                    .get_result::<i64>(conn)
            })
            .await
            .context(DbInteractSnafu)?;

        let count = count_res.context(DbQuerySnafu {
            table: "files".to_string(),
        })?;

        Ok(count)
    }

    async fn delete(&self, id: &str) -> Result<()> {
        let db = self.db_pool.get().await.context(DbPoolSnafu)?;

        let fid = id.to_string();
        let delete_res = db
            .interact(move |conn| diesel::delete(dsl::files.filter(dsl::id.eq(fid))).execute(conn))
            .await
            .context(DbInteractSnafu)?;

        let _ = delete_res.context(DbQuerySnafu {
            table: "files".to_string(),
        })?;

        Ok(())
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

                if errors.len() > 0 {
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

fn init_file(dir: &Dir, data: &FilePayload) -> Result<FileDto> {
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
                    let msg = format!("Unable to decode image: {}", e.to_string());
                    error!("{}", msg);
                    Err(msg.as_str().into())
                }
            },
            Err(e) => {
                let msg = format!("Unable to guess image format: {}", e.to_string());
                error!("{}", msg);
                Err(msg.as_str().into())
            }
        },
        Err(e) => {
            let msg = format!("Unable to read image: {}", e.to_string());
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
        Some(orientation) => match orientation.value.get_uint(0) {
            Some(v @ 1..=8) => v,
            _ => 1,
        },
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
pub struct FileTestRepo {}

#[cfg(test)]
#[async_trait]
impl FileRepoable for FileTestRepo {
    async fn list(&self, _dir: &Dir, _params: &ListFilesParams) -> Result<Paginated<FileObject>> {
        Ok(Paginated::new(vec![], 1, 10, 0))
    }

    async fn create(&self, _file_dto: FileDto) -> Result<FileObject> {
        Err("Not supported".into())
    }

    async fn get(&self, _id: &str) -> Result<Option<FileObject>> {
        Ok(None)
    }

    async fn find_by_name(&self, _dir_id: &str, _name: &str) -> Result<Option<FileObject>> {
        Ok(None)
    }

    async fn count_by_dir(&self, _dir_id: &str) -> Result<i64> {
        Ok(0)
    }

    async fn delete(&self, _id: &str) -> Result<()> {
        Ok(())
    }
}
