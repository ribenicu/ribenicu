//! 统一错误体：{ "code": <biz>, "data": null, "message": "..." }

use axum::{
    http::StatusCode,
    response::{IntoResponse, Response},
    Json,
};
use serde::Serialize;

/// 业务码约定：HTTP 状态 * 100 + 序号，见 docs/development-plan.md §6
#[derive(Debug, Clone, Copy)]
pub enum Code {
    BadRequest = 40000,
    Validation = 40001,
    Unauthorized = 40100,
    NotFound = 40400,
    Conflict = 40900,
    RateLimited = 42900,
    Internal = 50000,
}

#[derive(Debug)]
pub enum AppError {
    BadRequest(String),
    Validation(String),
    Unauthorized(String),
    NotFound(String),
    Conflict(String),
    RateLimited(String),
    Internal(String),
}

#[derive(Serialize)]
struct ErrorBody {
    code: i64,
    data: (),
    message: String,
}

impl AppError {
    fn parts(&self) -> (Code, StatusCode, &str) {
        match self {
            AppError::BadRequest(m) => (Code::BadRequest, StatusCode::BAD_REQUEST, m),
            AppError::Validation(m) => (Code::Validation, StatusCode::UNPROCESSABLE_ENTITY, m),
            AppError::Unauthorized(m) => (Code::Unauthorized, StatusCode::UNAUTHORIZED, m),
            AppError::NotFound(m) => (Code::NotFound, StatusCode::NOT_FOUND, m),
            AppError::Conflict(m) => (Code::Conflict, StatusCode::CONFLICT, m),
            AppError::RateLimited(m) => (Code::RateLimited, StatusCode::TOO_MANY_REQUESTS, m),
            AppError::Internal(m) => (Code::Internal, StatusCode::INTERNAL_SERVER_ERROR, m),
        }
    }
}

impl std::fmt::Display for AppError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let (_, _, message) = self.parts();
        write!(f, "{message}")
    }
}

impl std::error::Error for AppError {}

impl IntoResponse for AppError {
    fn into_response(self) -> Response {
        let (code, status, message) = self.parts();
        if status.is_server_error() {
            tracing::error!(code = code as i64, %message, "request failed");
        } else {
            tracing::debug!(code = code as i64, %message, "request rejected");
        }
        let body = ErrorBody {
            code: code as i64,
            data: (),
            message: message.to_string(),
        };
        (status, Json(body)).into_response()
    }
}

impl From<sqlx::Error> for AppError {
    fn from(e: sqlx::Error) -> Self {
        match e {
            sqlx::Error::RowNotFound => AppError::NotFound("资源不存在".into()),
            _ => {
                tracing::error!(error = %e, "database error");
                AppError::Internal("数据库错误".into())
            }
        }
    }
}

pub type AppResult<T> = Result<T, AppError>;
