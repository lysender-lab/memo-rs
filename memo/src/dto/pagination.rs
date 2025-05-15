use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct PaginatedMeta {
    pub page: i32,
    pub per_page: i32,
    pub total_records: i64,
    pub total_pages: i64,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Paginated<T> {
    pub meta: PaginatedMeta,
    pub data: Vec<T>,
}

impl PaginatedMeta {
    pub fn new(page: i32, per_page: i32, total_records: i64) -> Self {
        let total_pages = (total_records as f64 / per_page as f64).ceil() as i64;
        let actual_page = if page <= total_pages as i32 { page } else { 1 };
        Self {
            page: actual_page,
            per_page,
            total_records,
            total_pages,
        }
    }
}

impl<T> Paginated<T> {
    pub fn new(records: Vec<T>, page: i32, per_page: i32, total_records: i64) -> Self {
        Self {
            meta: PaginatedMeta::new(page, per_page, total_records),
            data: records,
        }
    }
}
