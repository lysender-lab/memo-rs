use std::{path::PathBuf, str::FromStr};

use diesel::prelude::*;
use serde::{Deserialize, Serialize};
use validator::Validate;

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

#[derive(Debug, Clone, Serialize)]
pub struct FileDto {
    pub id: String,
    pub dir_id: String,
    pub name: String,
    pub filename: String,
    pub content_type: String,
    pub size: i64,

    // Only available on non-image files
    pub url: Option<String>,

    pub is_image: bool,

    // Only available for image files, main url is in orig version
    pub img_versions: Option<Vec<ImgVersionDto>>,
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

#[derive(Debug, Clone, Serialize)]
pub struct ImgDimension {
    pub width: u32,
    pub height: u32,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub enum ImgVersion {
    #[serde(rename = "orig")]
    Original,

    #[serde(rename = "prev")]
    Preview,

    #[serde(rename = "thumb")]
    Thumbnail,
}

#[derive(Debug, Clone, Serialize)]
pub struct ImgVersionDto {
    pub version: ImgVersion,
    pub dimension: ImgDimension,
    pub url: Option<String>,
}

impl ImgVersionDto {
    pub fn to_path(&self, prefix: &PathBuf, filename: &str) -> PathBuf {
        prefix.clone().join(self.version.to_string()).join(filename)
    }
}

/// Convert ImgVersionDto to String
impl core::fmt::Display for ImgVersionDto {
    fn fmt(&self, f: &mut core::fmt::Formatter) -> core::fmt::Result {
        write!(
            f,
            "{}:{}x{}",
            self.version, self.dimension.width, self.dimension.height
        )
    }
}

/// Convert a string into ImgVersionDto
impl FromStr for ImgVersionDto {
    type Err = String;

    /// Parse string like "orig:200x400" into ImgVersionDto without the url part
    fn from_str(s: &str) -> core::result::Result<Self, Self::Err> {
        let parts: Vec<&str> = s.split(':').collect();
        if parts.len() != 2 {
            return Err("Invalid image version dto".to_string());
        }

        let version = ImgVersion::try_from(parts[0])?;
        let dimension = parts[1]
            .split('x')
            .filter_map(|s| s.parse::<u32>().ok())
            .collect::<Vec<u32>>();

        if dimension.len() != 2 {
            return Err("Invalid image dimension".to_string());
        }

        Ok(Self {
            version,
            dimension: ImgDimension {
                width: dimension[0],
                height: dimension[1],
            },
            url: None,
        })
    }
}

/// Convert ImgVersion to String
impl core::fmt::Display for ImgVersion {
    fn fmt(&self, f: &mut core::fmt::Formatter) -> core::fmt::Result {
        match self {
            Self::Original => write!(f, "{}", "orig"),
            Self::Preview => write!(f, "{}", "prev"),
            Self::Thumbnail => write!(f, "{}", "thumb"),
        }
    }
}

/// Convert from &str to ImgVersion
impl TryFrom<&str> for ImgVersion {
    type Error = String;

    fn try_from(value: &str) -> core::result::Result<Self, Self::Error> {
        match value {
            "orig" => Ok(Self::Original),
            "prev" => Ok(Self::Preview),
            "thumb" => Ok(Self::Thumbnail),
            _ => Err(format!("Invalid image version: {}", value)),
        }
    }
}
