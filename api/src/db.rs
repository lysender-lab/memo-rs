use std::sync::Arc;

use deadpool_diesel::sqlite::{Manager, Pool, Runtime};

use crate::{
    bucket::{BucketRepo, BucketRepoable},
    client::{ClientRepo, ClientRepoable},
};

pub fn create_db_pool(database_url: &str) -> Pool {
    let manager = Manager::new(database_url, Runtime::Tokio1);
    Pool::builder(manager).max_size(8).build().unwrap()
}

pub struct DbMapper {
    pub buckets: Arc<dyn BucketRepoable>,
    pub clients: Arc<dyn ClientRepoable>,
    pub dirs: Arc<dyn BucketRepoable>,
    pub files: Arc<dyn BucketRepoable>,
    pub users: Arc<dyn BucketRepoable>,
}

pub fn create_db_mapper(database_url: &str) -> DbMapper {
    let pool = create_db_pool(database_url);
    DbMapper {
        buckets: Arc::new(BucketRepo::new(pool.clone())),
        clients: Arc::new(ClientRepo::new(pool.clone())),
        dirs: Arc::new(BucketRepo::new(pool.clone())),
        files: Arc::new(BucketRepo::new(pool.clone())),
        users: Arc::new(BucketRepo::new(pool.clone())),
    }
}
