mod albums;
mod error;
mod index;
mod login;
mod logout;
mod middlewares;
mod photos;
mod policies;
mod pref;
mod routes;

pub const AUTH_TOKEN_COOKIE: &str = "auth_token";
pub const THEME_COOKIE: &str = "theme";

pub use albums::*;
pub use error::*;
pub use index::*;
pub use login::*;
pub use logout::*;
pub use middlewares::*;
pub use photos::*;
pub use policies::*;
pub use pref::*;
pub use routes::*;
