//! HTTP Basic authentication.

use hyper::header::{HeaderMap, AUTHORIZATION, WWW_AUTHENTICATE};
use hyper::{Response, StatusCode};

use crate::body::{self, ResBody};

/// Constant-time-ish comparison, so we don't leak length via timing too badly.
fn eq(a: &[u8], b: &[u8]) -> bool {
    if a.len() != b.len() {
        return false;
    }
    let mut diff = 0u8;
    for (x, y) in a.iter().zip(b.iter()) {
        diff |= x ^ y;
    }
    diff == 0
}

fn decode_base64(s: &str) -> Option<Vec<u8>> {
    const TABLE: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut lookup = [255u8; 256];
    for (i, c) in TABLE.iter().enumerate() {
        lookup[*c as usize] = i as u8;
    }
    let mut out = Vec::with_capacity(s.len() / 4 * 3);
    let mut buf: u32 = 0;
    let mut bits = 0u32;
    for ch in s.bytes() {
        if ch == b'=' || ch == b'\n' || ch == b'\r' {
            continue;
        }
        let v = lookup[ch as usize];
        if v == 255 {
            return None;
        }
        buf = (buf << 6) | v as u32;
        bits += 6;
        if bits >= 8 {
            bits -= 8;
            out.push((buf >> bits) as u8);
        }
    }
    Some(out)
}

/// Verify the `Authorization` header of a request.
pub fn authorized(headers: &HeaderMap, user: &str, pass: &str) -> bool {
    let Some(raw) = headers.get(AUTHORIZATION).and_then(|v| v.to_str().ok()) else {
        return false;
    };
    let Some(b64) = raw.strip_prefix("Basic ") else {
        return false;
    };
    let Some(decoded) = decode_base64(b64.trim()) else {
        return false;
    };
    let Some(colon) = decoded.iter().position(|b| *b == b':') else {
        return false;
    };
    let (u, p) = decoded.split_at(colon);
    let p = &p[1..];
    eq(u, user.as_bytes()) && eq(p, pass.as_bytes())
}

/// Standard 401 challenge.
pub fn challenge(realm: &str) -> Response<ResBody> {
    let body_str = "<!doctype html><html><head><meta charset=\"utf-8\">\
<title>401 Unauthorized</title></head><body style=\"font-family:system-ui;padding:3rem\">\
<h1>401 Unauthorized</h1><p>Authentication required.</p></body></html>";
    let mut resp = Response::new(body::full(body_str));
    *resp.status_mut() = StatusCode::UNAUTHORIZED;
    resp.headers_mut().insert(
        WWW_AUTHENTICATE,
        format!("Basic realm=\"{realm}\", charset=\"UTF-8\"")
            .parse()
            .unwrap(),
    );
    resp
}
