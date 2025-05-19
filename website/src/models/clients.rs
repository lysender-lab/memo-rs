use serde::{Deserialize, Serialize};

#[derive(Clone, Deserialize, Serialize)]
pub struct ClientFormSubmitData {
    pub name: String,
    pub status: String,
    pub token: String,
}

#[derive(Clone, Serialize)]
pub struct ClientSubmitData {
    pub name: String,
    pub status: String,
}
