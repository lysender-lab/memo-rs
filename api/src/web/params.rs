use serde::Deserialize;

#[derive(Clone, Debug, Deserialize)]
pub struct DirTypeParams {
    pub dir_type: String,
}

#[derive(Clone, Debug, Deserialize)]
pub struct DirParams {
    #[allow(dead_code)]
    pub dir_type: String,
    pub dir_id: String,
}

#[derive(Clone, Debug, Deserialize)]
pub struct FileParams {
    #[allow(dead_code)]
    pub dir_type: String,
    pub dir_id: String,
    pub file_id: String,
}
