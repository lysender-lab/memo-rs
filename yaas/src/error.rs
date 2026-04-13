use snafu::Snafu;

pub type Result<T> = std::result::Result<T, Error>;

#[derive(Debug, Snafu)]
#[snafu(visibility(pub(crate)))]
pub enum Error {
    #[snafu(display("{}", msg))]
    Validation { msg: String },

    #[snafu(display("Google Cloud error: {}", msg))]
    Google { msg: String },

    #[snafu(display("{}", msg))]
    BadRequest { msg: String },

    #[snafu(display("{}", msg))]
    Forbidden { msg: String },

    #[snafu(display("{}", msg))]
    NotFound { msg: String },

    #[snafu(display("Invalid auth token"))]
    InvalidAuthToken,

    #[snafu(display("Insufficient auth scope"))]
    InsufficientAuthScope,

    #[snafu(display("No auth token"))]
    NoAuthToken,

    #[snafu(display("Invalid client"))]
    InvalidClient,

    #[snafu(display("Requires authentication"))]
    RequiresAuth,

    #[snafu(display("Invalid roles: {}", msg))]
    InvalidRoles { msg: String },

    #[snafu(display("Invalid permissions: {}", msg))]
    InvalidPermissions { msg: String },

    #[snafu(display("Invalid scopes: {}", msg))]
    InvalidScopes { msg: String },

    #[snafu(display("App not found"))]
    AppNotFound,

    #[snafu(display("Org not found"))]
    OrgNotFound,

    #[snafu(display("Org member not found"))]
    OrgMemberNotFound,

    #[snafu(display("Org app not found"))]
    OrgAppNotFound,

    #[snafu(display("Failed to parse JWT claims: {}", source))]
    JwtClaimsParse { source: serde_json::Error },

    #[snafu(display("Failed to serialize JSON: {}", source))]
    JsonSerialize { source: serde_json::Error },

    #[snafu(display("OAuth redirect_uri mismatch"))]
    RedirectUriMistmatch,

    #[snafu(display("OAuth app not registered in the org"))]
    AppNotRegistered,

    #[snafu(display("OAuth state mismatch"))]
    OauthStateMismatch,

    #[snafu(display("OAuth code invalid"))]
    OauthCodeInvalid,

    #[snafu(display("OAuth scopes invalid"))]
    OauthInvalidScopes,

    #[snafu(display("Invalid username or password"))]
    LoginFailed,

    #[snafu(display("Login to continue"))]
    LoginRequired,

    #[snafu(display("{}", msg))]
    Service { msg: String },

    #[snafu(display("Invalid OAuth Token."))]
    InvalidOauthToken,

    #[snafu(display("{}", msg))]
    Oauth { msg: String },

    #[snafu(display("Too many requests. Please try again later."))]
    RateLimitExceeded,

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
