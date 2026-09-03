use argon2::{Argon2, PasswordHash, PasswordVerifier};
use axum::{extract::FromRequestParts, http::request::Parts};
use chrono::{Duration, Utc};
use jsonwebtoken::{DecodingKey, EncodingKey, Header, Validation, decode, encode};
use serde::{Deserialize, Serialize};

use crate::{
    AppState,
    error::{ApiError, ApiResult},
};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Claims {
    pub sub: i64,
    pub username: String,
    pub role: String,
    pub exp: usize,
}

#[derive(Debug, Clone)]
pub struct Actor {
    pub id: i64,
    pub username: String,
    pub role: String,
}

impl FromRequestParts<AppState> for Actor {
    type Rejection = ApiError;
    async fn from_request_parts(
        parts: &mut Parts,
        state: &AppState,
    ) -> Result<Self, Self::Rejection> {
        let raw = parts
            .headers
            .get("authorization")
            .and_then(|v| v.to_str().ok())
            .and_then(|v| v.strip_prefix("Bearer "))
            .ok_or(ApiError::Unauthorized)?;
        let token = decode::<Claims>(
            raw,
            &DecodingKey::from_secret(state.jwt_secret.as_bytes()),
            &Validation::default(),
        )
        .map_err(|_| ApiError::Unauthorized)?;
        Ok(Self {
            id: token.claims.sub,
            username: token.claims.username,
            role: token.claims.role,
        })
    }
}

pub fn token(id: i64, username: &str, role: &str, secret: &str) -> ApiResult<String> {
    let claims = Claims {
        sub: id,
        username: username.into(),
        role: role.into(),
        exp: (Utc::now() + Duration::hours(12)).timestamp() as usize,
    };
    encode(
        &Header::default(),
        &claims,
        &EncodingKey::from_secret(secret.as_bytes()),
    )
    .map_err(|_| ApiError::Internal)
}

pub fn verify_password(password: &str, hash: &str) -> bool {
    PasswordHash::new(hash).ok().is_some_and(|parsed| {
        Argon2::default()
            .verify_password(password.as_bytes(), &parsed)
            .is_ok()
    })
}

pub fn require(actor: &Actor, roles: &[&str]) -> ApiResult<()> {
    if roles.contains(&actor.role.as_str()) {
        Ok(())
    } else {
        Err(ApiError::Forbidden)
    }
}
