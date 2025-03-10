use serde::{Deserialize, Serialize};

#[derive(Clone, Deserialize)]
pub struct Album {
    pub id: String,

    #[allow(dead_code)]
    pub bucket_id: String,

    #[allow(dead_code)]
    pub name: String,

    pub label: String,

    #[allow(dead_code)]
    pub file_count: i64,

    #[allow(dead_code)]
    pub created_at: i64,

    #[allow(dead_code)]
    pub updated_at: i64,
}

#[derive(Clone, Deserialize, Serialize)]
pub struct NewAlbumForm {
    pub name: String,
    pub label: String,
    pub token: String,
}

#[derive(Clone, Serialize)]
pub struct NewAlbum {
    pub name: String,
    pub label: String,
}

#[derive(Clone, Deserialize, Serialize)]
pub struct UpdateAlbumForm {
    pub label: String,
    pub token: String,
}

#[derive(Clone, Serialize)]
pub struct UpdateAlbum {
    pub label: String,
}

#[derive(Clone, Deserialize, Serialize)]
pub struct DeleteAlbumForm {
    pub token: String,
}

#[derive(Clone, Deserialize, Serialize)]
pub struct DeletePhotoForm {
    pub token: String,
}

#[derive(Clone, Deserialize)]
pub struct FileObject {
    pub id: String,
    pub dir_id: String,
    pub name: String,
    pub filename: String,
    pub content_type: String,
    pub size: i64,

    // Only available on non-image files
    #[allow(dead_code)]
    pub url: Option<String>,

    pub is_image: bool,

    // Only available for image files, main url is in orig version
    pub img_versions: Option<Vec<ImgVersionDto>>,

    pub created_at: i64,
    pub updated_at: i64,
}

#[derive(Clone, Deserialize, Serialize)]
pub struct Photo {
    pub id: String,
    pub dir_id: String,
    pub name: String,
    pub filename: String,
    pub content_type: String,
    pub size: i64,
    pub orig: PhotoVersionDto,
    pub preview: PhotoVersionDto,
    pub thumb: PhotoVersionDto,
    pub created_at: i64,
    pub updated_at: i64,
}

#[derive(Clone, Deserialize, Serialize)]
pub struct UploadResult {
    pub error_message: Option<String>,
    pub photo: Option<Photo>,
    pub next_token: String,
}

impl TryFrom<FileObject> for Photo {
    type Error = String;

    fn try_from(file: FileObject) -> Result<Self, Self::Error> {
        if !file.is_image {
            return Err("File is not an image".into());
        }

        let Some(versions) = file.img_versions else {
            return Err("Missing image versions".into());
        };

        let versions: Vec<PhotoVersionDto> = versions
            .into_iter()
            .filter_map(|v| match v.url {
                None => None,
                Some(url) => Some(PhotoVersionDto {
                    version: v.version.as_str().try_into().unwrap(),
                    dimension: v.dimension,
                    url,
                }),
            })
            .collect();

        let orig = versions.iter().find(|v| v.version == ImgVersion::Original);
        let mut preview = versions.iter().find(|v| v.version == ImgVersion::Preview);
        let thumb = versions.iter().find(|v| v.version == ImgVersion::Thumbnail);

        if preview.is_none() && orig.is_some() {
            preview = orig.clone();
        }

        if orig.is_none() || preview.is_none() || thumb.is_none() {
            return Err("Missing image versions".into());
        }

        Ok(Photo {
            id: file.id,
            dir_id: file.dir_id,
            name: file.name,
            filename: file.filename,
            content_type: file.content_type,
            size: file.size,
            orig: orig.unwrap().clone(),
            preview: preview.unwrap().clone(),
            thumb: thumb.unwrap().clone(),
            created_at: file.created_at,
            updated_at: file.updated_at,
        })
    }
}

#[derive(Clone, Deserialize, Serialize)]
pub struct ImgDimension {
    pub width: u32,
    pub height: u32,
}

#[derive(Clone, PartialEq, Deserialize, Serialize)]
pub enum ImgVersion {
    #[serde(rename = "orig")]
    Original,

    #[serde(rename = "prev")]
    Preview,

    #[serde(rename = "thumb")]
    Thumbnail,
}

#[derive(Clone, Deserialize, Serialize)]
pub struct PhotoVersionDto {
    pub version: ImgVersion,
    pub dimension: ImgDimension,
    pub url: String,
}

#[derive(Clone, Deserialize, Serialize)]
pub struct ImgVersionDto {
    pub version: String,
    pub dimension: ImgDimension,
    pub url: Option<String>,
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
