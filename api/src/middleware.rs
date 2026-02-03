use axum::{
    async_trait,
    body::Body,
    extract::{FromRequestParts, State},
    http::{header, request::Parts, Request, StatusCode},
    middleware::Next,
    response::{IntoResponse, Response},
};
use jsonwebtoken::{decode, DecodingKey, Validation};
use std::future::Future;
use std::pin::Pin;
use uuid::Uuid;

use crate::models::{AppState, Claims};

#[derive(Debug, Clone)]
pub struct AuthUser {
    pub id: Uuid,
    pub username: String,
    pub role_id: Option<Uuid>,
    pub role_name: Option<String>,
}

#[async_trait]
impl<S> FromRequestParts<S> for AuthUser
where
    S: Send + Sync,
{
    type Rejection = StatusCode;

    async fn from_request_parts(parts: &mut Parts, _state: &S) -> Result<Self, Self::Rejection> {
        let claims = parts
            .extensions
            .get::<Claims>()
            .ok_or(StatusCode::UNAUTHORIZED)?;

        Ok(AuthUser {
            id: claims.sub,
            username: claims.username.clone(),
            role_id: claims.role_id,
            role_name: claims.role_name.clone(),
        })
    }
}

pub async fn auth(
    State(state): State<AppState>,
    mut req: Request<Body>,
    next: Next,
) -> Result<Response, StatusCode> {
    let auth_header = req
        .headers()
        .get(header::AUTHORIZATION)
        .and_then(|h| h.to_str().ok());

    let token = match auth_header {
        Some(h) if h.starts_with("Bearer ") => &h[7..],
        _ => return Err(StatusCode::UNAUTHORIZED),
    };

    let claims = decode::<Claims>(
        token,
        &DecodingKey::from_secret(state.config.jwt_secret.as_bytes()),
        &Validation::default(),
    )
    .map_err(|_| StatusCode::UNAUTHORIZED)?
    .claims;

    req.extensions_mut().insert(claims);
    Ok(next.run(req).await)
}

pub fn require_permission(
    permission: &'static str,
) -> impl Fn(Request<Body>, Next) -> Pin<Box<dyn Future<Output = Response> + Send>> + Clone + Send {
    move |req: Request<Body>, next: Next| {
        Box::pin(async move {
            match req.extensions().get::<Claims>() {
                Some(claims) if claims.has_permission(permission) => next.run(req).await,
                Some(_) => StatusCode::FORBIDDEN.into_response(),
                None => StatusCode::UNAUTHORIZED.into_response(),
            }
        })
    }
}

pub async fn require_admin(req: Request<Body>, next: Next) -> Response {
    match req.extensions().get::<Claims>() {
        Some(c) if c.has_permission("*") => next.run(req).await,
        Some(_) => StatusCode::FORBIDDEN.into_response(),
        None => StatusCode::UNAUTHORIZED.into_response(),
    }
}

pub async fn require_manager(req: Request<Body>, next: Next) -> Response {
    match req.extensions().get::<Claims>() {
        Some(c) if c.has_permission("admin.access") || c.has_permission("*") => next.run(req).await,
        Some(_) => StatusCode::FORBIDDEN.into_response(),
        None => StatusCode::UNAUTHORIZED.into_response(),
    }
}

