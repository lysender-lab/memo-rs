use diesel::prelude::*;
use serde::{Deserialize, Serialize};
use validator::Validate;

#[derive(Debug, Clone, Queryable, Selectable, Insertable, Serialize)]
#[diesel(table_name = crate::schema::buckets)]
#[diesel(check_for_backend(diesel::sqlite::Sqlite))]
pub struct Bucket {
    pub id: String,
    pub client_id: String,
    pub name: String,
    pub images_only: i32,
    pub created_at: i64,
}

#[derive(Debug, Clone, Serialize)]
pub struct BucketDto {
    pub id: String,
    pub client_id: String,
    pub name: String,
    pub images_only: bool,
    pub created_at: i64,
}

#[derive(Debug, Clone, Deserialize, Validate)]
pub struct NewBucket {
    #[validate(length(min = 1, max = 50))]
    #[validate(custom(function = "crate::validators::sluggable"))]
    pub name: String,

    pub images_only: bool,
}

#[derive(Debug, Clone, Deserialize, Validate)]
pub struct ListBucketsParams {
    #[validate(range(min = 1, max = 1000))]
    pub page: Option<i32>,

    #[validate(range(min = 1, max = 50))]
    pub per_page: Option<i32>,

    #[validate(length(min = 0, max = 50))]
    pub keyword: Option<String>,
}

impl From<BucketDto> for Bucket {
    fn from(dto: BucketDto) -> Self {
        Bucket {
            id: dto.id,
            client_id: dto.client_id,
            name: dto.name,
            images_only: if dto.images_only { 1 } else { 0 },
            created_at: dto.created_at,
        }
    }
}

impl From<Bucket> for BucketDto {
    fn from(bucket: Bucket) -> Self {
        BucketDto {
            id: bucket.id,
            client_id: bucket.client_id,
            name: bucket.name,
            images_only: bucket.images_only == 1,
            created_at: bucket.created_at,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_new_bucket() {
        let data = NewBucket {
            name: "hello-world".to_string(),
            images_only: false,
        };
        assert!(data.validate().is_ok());

        let data = NewBucket {
            name: "hello_world".to_string(),
            images_only: false,
        };
        assert!(data.validate().is_err());

        let data = NewBucket {
            name: "".to_string(),
            images_only: false,
        };
        assert!(data.validate().is_err());
    }
}
