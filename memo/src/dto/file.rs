use serde::{Deserialize, Serialize};
use std::{path::PathBuf, str::FromStr};

#[derive(Debug, Clone, Serialize, Deserialize)]
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

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ImgDimension {
    pub width: u32,
    pub height: u32,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum ImgVersion {
    #[serde(rename = "orig")]
    Original,

    #[serde(rename = "prev")]
    Preview,

    #[serde(rename = "thumb")]
    Thumbnail,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
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
