use axum::{
    Json,
    http::StatusCode,
    response::{IntoResponse, Response},
};
use serde_json::json;

#[derive(Debug, thiserror::Error)]
pub enum ApiError {
    #[error("未认证或登录已过期")]
    Unauthorized,
    #[error("没有执行此操作的权限")]
    Forbidden,
    #[error("{0}")]
    BadRequest(String),
    #[error("资源不存在")]
    NotFound,
    #[error("数据库操作失败")]
    Database(#[from] sqlx::Error),
    #[error("内部服务错误")]
    Internal,
}

impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        let status = match self {
            Self::Unauthorized => StatusCode::UNAUTHORIZED,
            Self::Forbidden => StatusCode::FORBIDDEN,
            Self::BadRequest(_) => StatusCode::BAD_REQUEST,
            Self::NotFound => StatusCode::NOT_FOUND,
            Self::Database(_) | Self::Internal => StatusCode::INTERNAL_SERVER_ERROR,
        };
        (status, Json(json!({"error": self.to_string()}))).into_response()
    }
}

pub type ApiResult<T> = Result<T, ApiError>;
