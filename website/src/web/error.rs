use askama::Template;
use axum::{body::Body, extract::State, http::StatusCode, response::Response};

use crate::{
    models::{Actor, Pref, TemplateData},
    run::AppState,
};
use memo::Error;

#[derive(Clone, Template)]
#[template(path = "pages/error.html")]
struct ErrorPageData {
    t: TemplateData,
    error: ErrorInfo,
}

#[derive(Clone, Template)]
#[template(path = "widgets/error.html")]
struct ErrorWidgetData {
    error: ErrorInfo,
}

#[derive(Clone, Template)]
#[template(path = "widgets/error_message.html")]
struct ErrorMessageData {
    message: String,
}

#[derive(Clone)]
pub struct ErrorInfo {
    pub status_code: StatusCode,
    pub title: String,
    pub message: String,
    pub description: String,
}

impl ErrorInfo {
    /// Creates a generic internal server error
    pub fn new(message: String) -> Self {
        Self {
            status_code: StatusCode::INTERNAL_SERVER_ERROR,
            title: "Internal Server Error".to_string(),
            message: message.clone(),
            description: message,
        }
    }
}

impl From<Error> for ErrorInfo {
    fn from(e: Error) -> Self {
        match e {
            Error::AnyError(msg) => Self {
                status_code: StatusCode::INTERNAL_SERVER_ERROR,
                title: "Internal Server Error".to_string(),
                message: msg.clone(),
                description: msg,
            },
            Error::ValidationError(msg) => Self {
                status_code: StatusCode::BAD_REQUEST,
                title: "Validation Error".to_string(),
                message: msg.clone(),
                description: msg,
            },
            Error::BadRequest(msg) => Self {
                status_code: StatusCode::BAD_REQUEST,
                title: "Bad Request".to_string(),
                message: msg.clone(),
                description: msg,
            },
            Error::Forbidden(msg) => Self {
                status_code: StatusCode::FORBIDDEN,
                title: "Forbidden".to_string(),
                message: msg.clone(),
                description: msg,
            },
            Error::LoginFailed(msg) => Self {
                status_code: StatusCode::UNAUTHORIZED,
                title: "Unauthorized".to_string(),
                message: msg.clone(),
                description: msg,
            },
            Error::InvalidCaptcha(msg) => Self {
                status_code: StatusCode::BAD_REQUEST,
                title: "Invalid Captcha".to_string(),
                message: msg.clone(),
                description: msg,
            },
            Error::CaptchaResponseError(msg) => Self {
                status_code: StatusCode::INTERNAL_SERVER_ERROR,
                title: "Captcha Error".to_string(),
                message: msg.clone(),
                description: msg,
            },
            Error::LoginRequired(msg) => Self {
                status_code: StatusCode::UNAUTHORIZED,
                title: "Unauthorized".to_string(),
                message: msg.clone(),
                description: msg,
            },
            Error::NoDefaultBucket => Self {
                status_code: StatusCode::INTERNAL_SERVER_ERROR,
                title: "Internal Server Error".to_string(),
                message: "No default bucket".to_string(),
                description: "No default bucket configured".to_string(),
            },
            Error::AlbumNotFound => Self {
                status_code: StatusCode::NOT_FOUND,
                title: "Not Found".to_string(),
                message: "Album not found".to_string(),
                description: "The album you are looking for does not exist".to_string(),
            },
            Error::PhotoNotFound => Self {
                status_code: StatusCode::NOT_FOUND,
                title: "Not Found".to_string(),
                message: "Photo not found".to_string(),
                description: "The photo you are looking for does not exist".to_string(),
            },
            Error::NoAuthCookie => Self {
                status_code: StatusCode::UNAUTHORIZED,
                title: "Unauthorized".to_string(),
                message: "Login to continue".to_string(),
                description: "You need to login to view this page".to_string(),
            },
            Error::InvalidCsrfToken => Self {
                status_code: StatusCode::BAD_REQUEST,
                title: "Bad Request".to_string(),
                message: "Stale form data. Refresh the page and try again".to_string(),
                description:
                    "The form data you are using is out of date. Refresh the page and try again."
                        .to_string(),
            },
            Error::JsonParseError(msg) => Self {
                status_code: StatusCode::INTERNAL_SERVER_ERROR,
                title: "Bad Request".to_string(),
                message: msg.clone(),
                description: msg,
            },
            Error::ServiceError(msg) => Self {
                status_code: StatusCode::INTERNAL_SERVER_ERROR,
                title: "Internal Server Error".to_string(),
                message: msg.clone(),
                description: msg,
            },

            // Catch all other errors
            _ => Self {
                status_code: StatusCode::INTERNAL_SERVER_ERROR,
                title: "Internal Server Error".to_string(),
                message: "An unknown error occurred".to_string(),
                description: "An unknown error occurred".to_string(),
            },
        }
    }
}

#[axum::debug_handler]
pub async fn error_handler(State(state): State<AppState>) -> Response<Body> {
    let actor = None;
    let pref = Pref::new();

    handle_error(
        &state,
        actor,
        &pref,
        ErrorInfo {
            status_code: StatusCode::NOT_FOUND,
            title: String::from("Not Found"),
            message: String::from("Page not found"),
            description: String::from("The page you are looking for cannot be found."),
        },
        true,
    )
}

/// Render an error page or an error widget
pub fn handle_error(
    state: &AppState,
    actor: Option<Actor>,
    pref: &Pref,
    error: ErrorInfo,
    full_page: bool,
) -> Response<Body> {
    if full_page {
        let title = error.title.as_str();
        let status_code = error.status_code;

        let mut t = TemplateData::new(&state, actor, pref);
        t.title = String::from(title);

        let tpl = ErrorPageData { t, error };

        Response::builder()
            .status(status_code)
            .body(Body::from(tpl.render().unwrap()))
            .unwrap()
    } else {
        let status_code = error.status_code;
        let tpl = ErrorWidgetData { error };

        Response::builder()
            .status(status_code)
            .body(Body::from(tpl.render().unwrap()))
            .unwrap()
    }
}

/// Render a simple error message
pub fn handle_error_message(error: Error) -> Response<Body> {
    let error_info: ErrorInfo = error.into();
    let tpl = ErrorMessageData {
        message: error_info.message,
    };

    Response::builder()
        .status(error_info.status_code)
        .body(Body::from(tpl.render().unwrap()))
        .unwrap()
}
