pub mod auth;
pub mod dirs;
pub mod document_whitelist;
pub mod error;
pub mod files;
pub mod index;
pub mod login;
pub mod logout;
pub mod middleware;
pub mod my_bucket;
pub mod policies;
pub mod pref;
pub mod profile;
pub mod routes;

pub const AUTH_TOKEN_COOKIE: &str = "memo_auth";
pub const THEME_COOKIE: &str = "memo_theme";

pub use error::*;
pub use index::*;
pub use logout::*;
pub use policies::*;
pub use pref::*;
pub use routes::*;
