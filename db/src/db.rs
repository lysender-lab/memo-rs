use std::sync::Arc;

use deadpool_diesel::sqlite::{Manager, Pool, Runtime};

use crate::{
    bucket::{BucketRepo, BucketStore},
    client::{ClientRepo, ClientStore},
    dir::{DirRepo, DirStore},
    file::{FileRepo, FileStore},
    user::{UserRepo, UserStore},
};

pub fn create_db_pool(database_url: &str) -> Pool {
    let manager = Manager::new(database_url, Runtime::Tokio1);
    Pool::builder(manager).max_size(8).build().unwrap()
}

pub struct DbMapper {
    pub buckets: Arc<dyn BucketStore>,
    pub clients: Arc<dyn ClientStore>,
    pub dirs: Arc<dyn DirStore>,
    pub files: Arc<dyn FileStore>,
    pub users: Arc<dyn UserStore>,
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

#[cfg(feature = "test")]
pub fn create_test_db_mapper() -> DbMapper {
    use crate::bucket::BucketTestRepo;
    use crate::client::ClientTestRepo;
    use crate::dir::DirTestRepo;
    use crate::file::FileTestRepo;
    use crate::user::UserTestRepo;

    DbMapper {
        buckets: Arc::new(BucketTestRepo {}),
        clients: Arc::new(ClientTestRepo {}),
        dirs: Arc::new(DirTestRepo {}),
        files: Arc::new(FileTestRepo {}),
        users: Arc::new(UserTestRepo {}),
    }
}
