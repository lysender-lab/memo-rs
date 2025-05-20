use serde::{Deserialize, Serialize};

#[derive(Clone, Deserialize, Serialize)]
pub struct SelectOption {
    pub value: String,
    pub label: String,
}
