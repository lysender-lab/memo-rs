use deadpool_diesel::sqlite::{Manager, Pool, Runtime};

pub fn create_db_pool(database_url: &str) -> Pool {
    let manager = Manager::new(database_url, Runtime::Tokio1);
    Pool::builder(manager).max_size(8).build().unwrap()
}
