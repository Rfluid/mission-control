//! Static pages and assets, compiled in: the index and mission page shells, scripts, styles, fonts, favicon.

use axum::extract::Path;
use axum::http::{StatusCode, header};
use axum::response::{Html, IntoResponse, Response};

pub(super) const INDEX_HTML: &str = include_str!("../../web/index.html");
pub(super) const MISSION_HTML: &str = include_str!("../../web/mission.html");
pub(super) const ASSETS: [(&str, &str, &str); 33] = [
    ("home.css", "text/css", include_str!("../../web/home.css")),
    ("attention.js", "text/javascript", include_str!("../../web/attention.js")),
    ("palette.js", "text/javascript", include_str!("../../web/palette.js")),
    ("keys.js", "text/javascript", include_str!("../../web/keys.js")),
    ("widgets-graph.js", "text/javascript", include_str!("../../web/widgets-graph.js")),
    ("mermaid.js", "text/javascript", include_str!("../../web/mermaid.js")),
    ("drawer.js", "text/javascript", include_str!("../../web/drawer.js")),
    ("drawer.css", "text/css", include_str!("../../web/drawer.css")),
    ("viewport.js", "text/javascript", include_str!("../../web/viewport.js")),
    ("graph.css", "text/css", include_str!("../../web/graph.css")),
    ("plan-layout.js", "text/javascript", include_str!("../../web/plan-layout.js")),
    ("widgets-track.js", "text/javascript", include_str!("../../web/widgets-track.js")),
    ("widgets-plan.js", "text/javascript", include_str!("../../web/widgets-plan.js")),
    ("shell.js", "text/javascript", include_str!("../../web/shell.js")),
    ("widgets-core.js", "text/javascript", include_str!("../../web/widgets-core.js")),
    ("widgets-needs.js", "text/javascript", include_str!("../../web/widgets-needs.js")),
    ("mark.svg", "image/svg+xml", include_str!("../../.design/logo/mark.svg")),
    ("mark-light.svg", "image/svg+xml", include_str!("../../.design/logo/mark-light.svg")),
    ("pages.js", "text/javascript", include_str!("../../web/pages.js")),
    ("live.js", "text/javascript", include_str!("../../web/live.js")),
    ("shell.css", "text/css", include_str!("../../web/shell.css")),
    ("widgets.css", "text/css", include_str!("../../web/widgets.css")),
    ("glyphs.js", "text/javascript", include_str!("../../web/glyphs.js")),
    ("index.js", "text/javascript", include_str!("../../web/index.js")),
    ("widgets-chart.js", "text/javascript", include_str!("../../web/widgets-chart.js")),
    ("widgets-board.js", "text/javascript", include_str!("../../web/widgets-board.js")),
    ("widgets-embed.js", "text/javascript", include_str!("../../web/widgets-embed.js")),
    ("markdown.js", "text/javascript", include_str!("../../web/markdown.js")),
    ("app.js", "text/javascript", include_str!("../../web/app.js")),
    ("actions.js", "text/javascript", include_str!("../../web/actions.js")),
    ("widgets.js", "text/javascript", include_str!("../../web/widgets.js")),
    ("style.css", "text/css", include_str!("../../web/style.css")),
    ("favicon.svg", "image/svg+xml", include_str!("../../.design/logo/favicon.svg")),
];
/// The bundled fonts (OFL; their licenses are served next to them). File names carry the font version, so
/// browsers may cache them for long.
const FONTS: [(&str, &str, &[u8]); 6] = [
    ("inter-v20-latin.woff2", "font/woff2", include_bytes!("../../web/fonts/inter-v20-latin.woff2")),
    ("inter-v20-latin-ext.woff2", "font/woff2", include_bytes!("../../web/fonts/inter-v20-latin-ext.woff2")),
    ("jetbrains-mono-v24-latin.woff2", "font/woff2", include_bytes!("../../web/fonts/jetbrains-mono-v24-latin.woff2")),
    (
        "jetbrains-mono-v24-latin-ext.woff2",
        "font/woff2",
        include_bytes!("../../web/fonts/jetbrains-mono-v24-latin-ext.woff2"),
    ),
    ("OFL-inter.txt", "text/plain; charset=utf-8", include_bytes!("../../web/fonts/OFL-inter.txt")),
    ("OFL-jetbrains-mono.txt", "text/plain; charset=utf-8", include_bytes!("../../web/fonts/OFL-jetbrains-mono.txt")),
];
const FONT_CACHE: &str = "public, max-age=31536000, immutable";
/// Mermaid, gzipped (see `web/vendor/README.md`); the URL carries its version, so it caches like the fonts.
pub(super) const MERMAID_URL: &str = "mermaid-11.12.3.js";
const MERMAID_GZ: &[u8] = include_bytes!("../../web/vendor/mermaid-11.12.3.min.js.gz");
const MERMAID_LICENSE: &str = include_str!("../../web/vendor/LICENSE-mermaid.txt");
/// `GET /`: the missions home, with the daemon's address and version filled in.
pub async fn index(axum::extract::State(app): axum::extract::State<std::sync::Arc<super::App>>) -> Html<String> {
    Html(
        INDEX_HTML
            .replace("{{ADDR}}", &format!("127.0.0.1:{}", app.port))
            .replace("{{VERSION}}", env!("CARGO_PKG_VERSION")),
    )
}

pub async fn asset(Path(file): Path<String>) -> Response {
    match ASSETS.iter().find(|(name, ..)| *name == file) {
        Some((_, mime, body)) => {
            ([(header::CONTENT_TYPE, *mime), (header::CACHE_CONTROL, "no-cache")], *body).into_response()
        }
        None => StatusCode::NOT_FOUND.into_response(),
    }
}

/// `GET /assets/fonts/{file}`: a bundled font or its license.
pub async fn font(Path(file): Path<String>) -> Response {
    match FONTS.iter().find(|(name, ..)| *name == file) {
        Some((_, mime, body)) => {
            ([(header::CONTENT_TYPE, *mime), (header::CACHE_CONTROL, FONT_CACHE)], *body).into_response()
        }
        None => StatusCode::NOT_FOUND.into_response(),
    }
}

/// `GET /assets/vendor/{file}`: the Mermaid renderer (sent gzipped, as stored) and its license.
pub async fn vendor(Path(file): Path<String>) -> Response {
    if file == MERMAID_URL {
        let headers = [
            (header::CONTENT_TYPE, "text/javascript"),
            (header::CONTENT_ENCODING, "gzip"),
            (header::CACHE_CONTROL, FONT_CACHE),
        ];
        return (headers, MERMAID_GZ).into_response();
    }
    if file == "LICENSE-mermaid.txt" {
        return ([(header::CONTENT_TYPE, "text/plain; charset=utf-8")], MERMAID_LICENSE).into_response();
    }
    StatusCode::NOT_FOUND.into_response()
}

#[cfg(test)]
mod tests {
    #[test]
    fn the_page_asks_for_the_mermaid_url_the_daemon_serves() {
        let js = include_str!("../../web/mermaid.js");
        assert!(
            js.contains(&format!("/assets/vendor/{}", super::MERMAID_URL)),
            "web/mermaid.js and MERMAID_URL disagree"
        );
    }
}
