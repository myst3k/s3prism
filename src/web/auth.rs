use axum::{
    Json,
    extract::{Request, State},
    http::StatusCode,
    middleware::Next,
    response::{IntoResponse, Response},
};

use super::state::MgmtState;

pub async fn auth_middleware(
    State(state): State<MgmtState>,
    req: Request,
    next: Next,
) -> Response {
    let has_users = state
        .store
        .list_admin_users()
        .map(|u| !u.is_empty())
        .unwrap_or(false);
    let has_password = state.mgmt_password.is_some();

    // No auth configured — allow everything
    if !has_users && !has_password {
        return next.run(req).await;
    }

    let auth_header = req
        .headers()
        .get("authorization")
        .and_then(|v| v.to_str().ok());

    let token = match auth_header {
        Some(h) if h.starts_with("Bearer ") => &h[7..],
        _ => {
            return (
                StatusCode::UNAUTHORIZED,
                Json(serde_json::json!({"error": "unauthorized"})),
            )
                .into_response();
        }
    };

    // Check user-based token (format: "user:<password_hash>")
    if let Some(hash) = token.strip_prefix("user:") {
        if has_users {
            let valid = state
                .store
                .list_admin_users()
                .map(|users| users.iter().any(|u| u.password_hash == hash))
                .unwrap_or(false);
            if valid {
                return next.run(req).await;
            }
        }
    }

    // Check legacy password
    if let Some(pw) = &state.mgmt_password {
        if token == pw {
            return next.run(req).await;
        }
    }

    (
        StatusCode::UNAUTHORIZED,
        Json(serde_json::json!({"error": "invalid token"})),
    )
        .into_response()
}
