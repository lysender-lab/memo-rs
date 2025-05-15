use std::sync::Arc;

use deadpool_diesel::sqlite::{Manager, Pool, Runtime};

use crate::{
    auth::user::{UserRepo, UserRepoable},
    bucket::{BucketRepo, BucketRepoable},
    client::{ClientRepo, ClientRepoable},
    dir::{DirRepo, DirRepoable},
    file::{FileRepo, FileRepoable},
};

pub fn create_db_pool(database_url: &str) -> Pool {
    let manager = Manager::new(database_url, Runtime::Tokio1);
    Pool::builder(manager).max_size(8).build().unwrap()
}

pub struct DbMapper {
    pub buckets: Arc<dyn BucketRepoable>,
    pub clients: Arc<dyn ClientRepoable>,
    pub dirs: Arc<dyn DirRepoable>,
    pub files: Arc<dyn FileRepoable>,
    pub users: Arc<dyn UserRepoable>,
}

pub fn create_db_mapper(database_url: &str) -> DbMapper {
    let pool = create_db_pool(database_url);
    DbMapper {
        buckets: Arc::new(BucketRepo::new(pool.clone())),
        clients: Arc::new(ClientRepo::new(pool.clone())),
        dirs: Arc::new(DirRepo::new(pool.clone())),
        files: Arc::new(FileRepo::new(pool.clone())),
        users: Arc::new(UserRepo::new(pool.clone())),
    }
}

#[cfg(test)]
pub fn create_test_db_mapper() -> DbMapper {
    use crate::auth::user::UserTestRepo;
    use crate::bucket::BucketTestRepo;
    use crate::client::ClientTestRepo;
    use crate::dir::DirTestRepo;
    use crate::file::FileTestRepo;

    DbMapper {
        buckets: Arc::new(BucketTestRepo {}),
        clients: Arc::new(ClientTestRepo {}),
        dirs: Arc::new(DirTestRepo {}),
        files: Arc::new(FileTestRepo {}),
        users: Arc::new(UserTestRepo {}),
    }
}
