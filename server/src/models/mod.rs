pub mod comment;
pub mod post;
pub mod tag;
pub mod user;

use axum::response::{IntoResponse, Response};
use serde::Serialize;

/// 统一响应包：{ "code": 0, "data": ..., "message": "" }
#[derive(Serialize)]
pub struct ApiResponse<T: Serialize> {
    pub code: i64,
    pub data: T,
    pub message: String,
}

impl<T: Serialize> ApiResponse<T> {
    pub fn ok(data: T) -> Self {
        Self {
            code: 0,
            data,
            message: String::new(),
        }
    }
}

impl<T: Serialize> IntoResponse for ApiResponse<T> {
    fn into_response(self) -> Response {
        axum::Json(self).into_response()
    }
}

/// 空 data 的成功响应
#[derive(Serialize)]
pub struct Empty {}

pub fn ok<T: Serialize>(data: T) -> ApiResponse<T> {
    ApiResponse::ok(data)
}
