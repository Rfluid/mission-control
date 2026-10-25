//! DNS-rebinding guard: a page on `http://evil.example:7466`, its name rebound to 127.0.0.1, would be same-origin
//! with the daemon, so CORS couldn't stop it reading missions, a page's token, or posting page events. Browsers
//! always send the name they used in `Host`, so only `127.0.0.1:<port>` and `localhost:<port>` are served.
//! Requests without `Host` (non-browser clients) pass.

use super::App;
use axum::extract::{Request, State};
use axum::http::{StatusCode, header};
use axum::middleware::Next;
use axum::response::{IntoResponse, Response};
use std::sync::Arc;

pub async fn guard(State(app): State<Arc<App>>, req: Request, next: Next) -> Response {
    let host = req.headers().get(header::HOST).and_then(|h| h.to_str().ok());
    if let Some(h) = host {
        let ok = [format!("127.0.0.1:{}", app.port), format!("localhost:{}", app.port), format!("[::1]:{}", app.port)];
        if !ok.iter().any(|o| o.eq_ignore_ascii_case(h)) {
            tracing::warn!(host = h, "refused a request for another host name (DNS rebinding?)");
            return (
                StatusCode::MISDIRECTED_REQUEST,
                "this mission-control daemon only answers to 127.0.0.1 and localhost",
            )
                .into_response();
        }
    }
    next.run(req).await
}
