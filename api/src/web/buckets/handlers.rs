use axum::{Extension, extract::State};

use crate::{
    Result,
    auth::Actor,
    buckets::{BucketDto, list_buckets},
    web::{response::JsonResponse, server::AppState},
};

pub async fn list_buckets_handler(
    State(state): State<AppState>,
    Extension(actor): Extension<Actor>,
) -> Result<JsonResponse> {
    let buckets = list_buckets(&state.db_pool, &actor.client_id).await?;
    Ok(JsonResponse::new(serde_json::to_string(&buckets).unwrap()))
}

pub async fn get_bucket_handler(Extension(bucket): Extension<BucketDto>) -> Result<JsonResponse> {
    // Extract bucket from the middleware extension
    Ok(JsonResponse::new(serde_json::to_string(&bucket).unwrap()))
}
