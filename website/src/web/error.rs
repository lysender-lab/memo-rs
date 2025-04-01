use askama::Template;
use axum::{body::Body, extract::State, http::StatusCode, response::Response};

use crate::{
    Error,
    models::{Actor, Pref, TemplateData},
    run::AppState,
};

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
    pub backtrace: Option<String>,
}

impl ErrorInfo {
    /// Creates a generic internal server error
    pub fn new(message: String) -> Self {
        Self {
            status_code: StatusCode::INTERNAL_SERVER_ERROR,
            title: "Internal Server Error".to_string(),
            message,
            backtrace: None,
        }
    }
}

impl From<&Error> for ErrorInfo {
    fn from(e: &Error) -> Self {
        let status_code = e.into();
        let msg = e.to_string();
        Self {
            status_code,
            title: status_code.canonical_reason().unwrap().to_string(),
            message: msg,
            backtrace: None,
        }
    }
}

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
            message: String::from("The page you are looking for cannot be found."),
            backtrace: None,
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
pub fn handle_error_message(error: &Error) -> Response<Body> {
    let error_info: ErrorInfo = error.into();
    let tpl = ErrorMessageData {
        message: error_info.message,
    };

    Response::builder()
        .status(error_info.status_code)
        .body(Body::from(tpl.render().unwrap()))
        .unwrap()
}
