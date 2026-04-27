use serde::{Deserialize, Serialize};

#[derive(PartialEq, Debug, Clone, Serialize, Deserialize)]
pub enum DirType {
    Photos,
    Videos,
    Documents,
}

impl TryFrom<&str> for DirType {
    type Error = String;

    fn try_from(value: &str) -> core::result::Result<Self, Self::Error> {
        match value {
            "photos" => Ok(Self::Photos),
            "videos" => Ok(Self::Videos),
            "documents" => Ok(Self::Documents),
            _ => Err(format!("Invalid dir type: {value}")),
        }
    }
}

impl core::fmt::Display for DirType {
    fn fmt(&self, f: &mut core::fmt::Formatter) -> core::fmt::Result {
        match self {
            Self::Photos => write!(f, "{}", "photos"),
            Self::Videos => write!(f, "{}", "videos"),
            Self::Documents => write!(f, "{}", "documents"),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DirMeta {
    pub bucket_name: String,
    pub org_id: String,
    pub dir_type: DirType,
    pub dir_name: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DirDto {
    pub id: String,
    pub bucket_id: String,
    pub name: String,
    pub label: String,
    pub file_count: i32,
    pub created_at: i64,
    pub updated_at: i64,
}
