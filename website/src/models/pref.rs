use serde::{Deserialize, Serialize};

#[derive(Clone, Deserialize, Serialize)]
pub struct Pref {
    pub theme: String,
}

impl Pref {
    pub fn new() -> Self {
        Self {
            theme: String::from("light"),
        }
    }
}
