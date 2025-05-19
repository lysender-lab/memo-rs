use serde::{Deserialize, Serialize};

#[derive(Clone, Deserialize, Serialize)]
pub struct TokenFormData {
    pub token: String,
}
