use snafu::Snafu;

pub type Result<T> = std::result::Result<T, Error>;

#[derive(Debug, Snafu)]
#[snafu(visibility(pub(crate)))]
pub enum Error {
    #[snafu(display("I/O error: {}", source))]
    Io { source: std::io::Error },

    #[snafu(display("DB Builder error: {}", source))]
    DbBuilder { source: turso::Error },

    #[snafu(display("DB Connect error: {}", source))]
    DbConnect { source: turso::Error },

    #[snafu(display("DB Execute error: {}", source))]
    DbExecute { source: turso::Error },

    #[snafu(display("DB Prepare error: {}", source))]
    DbPrepare { source: turso::Error },

    #[snafu(display("DB Statement error: {}", source))]
    DbStatement { source: turso::Error },

    #[snafu(display("DB Query error: {}", source))]
    DbQuery { source: turso::Error },

    #[snafu(display("DB Row error: {}", source))]
    DbRow { source: turso::Error },

    #[snafu(display("DB Result error: {}", source))]
    DbResult { source: turso::Error },

    #[snafu(display("DB Value error: {}", source))]
    DbValue { source: turso::Error },

    #[snafu(display("DB Transaction error: {}", source))]
    DbTransaction { source: turso::Error },

    #[snafu(display("Database pool configuration error: {}", msg))]
    DbPoolConfig { msg: String },

    #[snafu(display("Timed out acquiring database connection from pool"))]
    DbPoolAcquireTimeout { source: tokio::time::error::Elapsed },

    #[snafu(display("Database pool state error: {}", msg))]
    DbPoolState { msg: String },

    #[snafu(display("{}", msg))]
    Validation { msg: String },

    #[snafu(display("Maximum number of clients reached: 10"))]
    MaxClientsReached,

    #[snafu(display("Maximum number of users reached: 100"))]
    MaxUsersReached,

    #[snafu(display("Maximum number of buckets reached: 50"))]
    MaxBucketsReached,

    #[snafu(display("Maximum number of directories reached: 1000"))]
    MaxDirsReached,

    #[snafu(display("Maximum number of files reached: 1000"))]
    MaxFilesReached,

    #[snafu(display("{}", source))]
    HashPassword { source: password::Error },

    #[snafu(display("{}", msg))]
    Whatever { msg: String },
}

// Allow string slices to be converted to Error
impl From<&str> for Error {
    fn from(val: &str) -> Self {
        Self::Whatever {
            msg: val.to_string(),
        }
    }
}

impl From<String> for Error {
    fn from(val: String) -> Self {
        Self::Whatever { msg: val }
    }
}
