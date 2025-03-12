use diesel::prelude::*;
use serde::{Deserialize, Serialize};
use validator::Validate;

#[derive(Debug, Clone, Queryable, Selectable, Insertable, Serialize)]
#[diesel(table_name = crate::schema::dirs)]
#[diesel(check_for_backend(diesel::sqlite::Sqlite))]
pub struct Dir {
    pub id: String,
    pub bucket_id: String,
    pub name: String,
    pub label: String,
    pub file_count: i32,
    pub created_at: i64,
    pub updated_at: i64,
}

#[derive(Debug, Clone, Deserialize, Validate)]
pub struct NewDir {
    #[validate(length(min = 1, max = 50))]
    #[validate(custom(function = "crate::validators::sluggable"))]
    pub name: String,

    #[validate(length(min = 1, max = 60))]
    pub label: String,
}

#[derive(Debug, Clone, Deserialize, Validate, AsChangeset)]
#[diesel(table_name = crate::schema::dirs)]
pub struct UpdateDir {
    #[validate(length(min = 1, max = 100))]
    pub label: Option<String>,
}

#[derive(Debug, Clone, Deserialize, Validate)]
pub struct ListDirsParams {
    #[validate(range(min = 1, max = 1000))]
    pub page: Option<i32>,

    #[validate(range(min = 1, max = 50))]
    pub per_page: Option<i32>,

    #[validate(length(min = 0, max = 50))]
    pub keyword: Option<String>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_new_dir() {
        let data = NewDir {
            name: "hello-world".to_string(),
            label: "Hello World".to_string(),
        };
        assert!(data.validate().is_ok());

        let data = NewDir {
            name: "hello_world".to_string(),
            label: "Hello World".to_string(),
        };
        assert!(data.validate().is_err());

        let data = NewDir {
            name: "".to_string(),
            label: "Hello World".to_string(),
        };
        assert!(data.validate().is_err());
    }
}
