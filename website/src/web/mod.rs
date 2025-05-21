pub mod albums;
pub mod buckets;
pub mod clients;
pub mod dirs;
pub mod error;
pub mod files;
pub mod index;
pub mod login;
pub mod logout;
pub mod middleware;
pub mod my_bucket;
pub mod photos;
pub mod policies;
pub mod pref;
pub mod routes;
pub mod users;

pub const AUTH_TOKEN_COOKIE: &str = "auth_token";
pub const THEME_COOKIE: &str = "theme";

pub use albums::*;
pub use error::*;
pub use index::*;
pub use login::*;
pub use logout::*;
pub use photos::*;
pub use policies::*;
pub use pref::*;
pub use routes::*;
