//! `--proxy`: forward requests to an upstream dev server.

use hyper::body::Incoming;
use hyper::header::{HeaderName, HeaderValue, HOST};
use hyper::{Request, Response, StatusCode};

use crate::body::{self, ResBody};
use crate::listing::error_html;
use crate::state::State;

/// Headers that must not be forwarded between two HTTP hops.
const HOP_BY_HOP: &[&str] = &[
    "connection",
    "keep-alive",
    "proxy-authenticate",
    "proxy-authorization",
    "te",
    "trailer",
    "transfer-encoding",
    "upgrade",
    "proxy-connection",
];

pub async fn forward(state: &State, target: &str, req: Request<Incoming>) -> Response<ResBody> {
    let Some(client) = &state.proxy else {
        return error_html(
            StatusCode::INTERNAL_SERVER_ERROR,
            "500 Internal Server Error",
            "The proxy client is not initialised.",
        );
    };

    let base = target.trim_end_matches('/');
    let path_and_query = req
        .uri()
        .path_and_query()
        .map(|p| p.as_str().to_string())
        .unwrap_or_else(|| "/".to_string());
    let url = format!("{base}{path_and_query}");

    let uri: hyper::Uri = match url.parse() {
        Ok(u) => u,
        Err(e) => {
            return error_html(
                StatusCode::BAD_GATEWAY,
                "502 Bad Gateway",
                &format!("invalid proxy target '{url}': {e}"),
            )
        }
    };

    let (mut parts, body) = req.into_parts();
    parts.uri = uri.clone();
    parts.version = hyper::Version::HTTP_11;
    if let Some(authority) = uri.authority() {
        if let Ok(hv) = HeaderValue::from_str(authority.as_str()) {
            parts.headers.insert(HOST, hv);
        }
    }
    for name in HOP_BY_HOP {
        parts.headers.remove(*name);
    }
    parts.headers.remove("accept-encoding");

    let upstream_req = Request::from_parts(parts, body);

    match client.request(upstream_req).await {
        Ok(resp) => {
            let (mut parts, body) = resp.into_parts();
            for name in HOP_BY_HOP {
                parts.headers.remove(*name);
            }
            let _ = HeaderName::from_static("x-proxied-by");
            parts
                .headers
                .insert("x-proxied-by", HeaderValue::from_static("yhtps"));
            Response::from_parts(parts, body::from_incoming(body))
        }
        Err(e) => error_html(
            StatusCode::BAD_GATEWAY,
            "502 Bad Gateway",
            &format!("cannot reach the proxy target '{base}': {e}"),
        ),
    }
}
