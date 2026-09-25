use std::io::Cursor;
use tiny_http::{Header, Request, Response, StatusCode};

use super::handlers::*;
use super::state::WorkerState;

pub fn route_request(mut request: Request, state: &WorkerState, api_key: &str) {
    if request.method().as_str() == "OPTIONS" {
        let mut resp = Response::empty(200);
        add_cors_headers(&mut resp, "");
        let _ = request.respond(resp);
        return;
    }

    // Bearer token check
    if !api_key.is_empty() {
        let auth_hdr = request
            .headers()
            .iter()
            .find(|h| h.field.equiv("authorization"))
            .map(|h| h.value.as_str());

        let expected = format!("Bearer {}", api_key);
        if auth_hdr != Some(&expected) {
            let json = serde_json::json!({ "error": "Unauthorized: invalid or missing Bearer token" });
            send_json(request, 401, &json.to_string(), "");
            return;
        }
    }

    let url = request.url().to_string();
    let path = url.split('?').next().unwrap_or(&url);
    let method = request.method().as_str();

    let cat_hash = state
        .catalog
        .read()
        .map(|g| g.catalog_hash.clone())
        .unwrap_or_default();

    match (method, path) {
        ("GET", "/health") => {
            let res = handle_health(state);
            send_json(request, 200, &serde_json::to_string(&res).unwrap_or_default(), &cat_hash);
        }
        ("GET", "/api/skills/stats") => {
            let res = handle_stats(state);
            send_json(request, 200, &serde_json::to_string(&res).unwrap_or_default(), &cat_hash);
        }
        ("GET", "/api/skills/list") => {
            let res = handle_list(state);
            send_json(request, 200, &serde_json::to_string(&res).unwrap_or_default(), &cat_hash);
        }
        ("POST", "/api/skills/search") => {
            let body = read_body(&mut request);
            match serde_json::from_str(&body).map_err(anyhow::Error::from).and_then(|r| handle_search(state, r)) {
                Ok(res) => send_json(request, 200, &serde_json::to_string(&res).unwrap_or_default(), &cat_hash),
                Err(e) => send_error(request, 400, &e.to_string(), &cat_hash),
            }
        }
        ("POST", "/api/skills/slice") => {
            let body = read_body(&mut request);
            match serde_json::from_str(&body).map_err(anyhow::Error::from).and_then(|r| handle_slice(state, r)) {
                Ok(res) => send_json(request, 200, &serde_json::to_string(&res).unwrap_or_default(), &cat_hash),
                Err(e) => send_error(request, 400, &e.to_string(), &cat_hash),
            }
        }
        ("POST", "/api/skills/delete") => {
            let body = read_body(&mut request);
            match serde_json::from_str(&body).map_err(anyhow::Error::from).and_then(|r| handle_delete(state, r)) {
                Ok(res) => send_json(request, 200, &serde_json::to_string(&res).unwrap_or_default(), &cat_hash),
                Err(e) => send_error(request, 500, &e.to_string(), &cat_hash),
            }
        }
        ("POST", "/api/skills/reindex") => {
            let body = read_body(&mut request);
            let req = serde_json::from_str(&body).unwrap_or(super::types::ReindexRequest { force: Some(false) });
            match handle_reindex(state, req) {
                Ok(res) => send_json(request, 200, &serde_json::to_string(&res).unwrap_or_default(), &cat_hash),
                Err(e) => send_error(request, 500, &e.to_string(), &cat_hash),
            }
        }
        ("POST", "/api/embed") => {
            let body = read_body(&mut request);
            match serde_json::from_str(&body).map_err(anyhow::Error::from).and_then(handle_embed) {
                Ok(res) => send_json(request, 200, &serde_json::to_string(&res).unwrap_or_default(), &cat_hash),
                Err(e) => send_error(request, 400, &e.to_string(), &cat_hash),
            }
        }
        _ => {
            send_error(request, 404, "Endpoint not found", &cat_hash);
        }
    }
}

fn read_body(req: &mut Request) -> String {
    let mut buf = String::new();
    let _ = req.as_reader().read_to_string(&mut buf);
    buf
}

fn send_json(req: Request, status: u16, json: &str, cat_hash: &str) {
    let mut resp = Response::new(
        StatusCode(status),
        vec![Header::from_bytes(&b"Content-Type"[..], &b"application/json; charset=utf-8"[..]).unwrap()],
        Cursor::new(json.as_bytes().to_vec()),
        Some(json.len()),
        None,
    );
    add_cors_headers(&mut resp, cat_hash);
    let _ = req.respond(resp);
}

fn send_error(req: Request, status: u16, msg: &str, cat_hash: &str) {
    let json = serde_json::json!({ "error": msg }).to_string();
    send_json(req, status, &json, cat_hash);
}

fn add_cors_headers<R: std::io::Read>(resp: &mut Response<R>, cat_hash: &str) {
    resp.add_header(Header::from_bytes(&b"Access-Control-Allow-Origin"[..], &b"*"[..]).unwrap());
    resp.add_header(Header::from_bytes(&b"Access-Control-Allow-Methods"[..], &b"GET, POST, OPTIONS"[..]).unwrap());
    resp.add_header(Header::from_bytes(&b"Access-Control-Allow-Headers"[..], &b"Content-Type, Authorization"[..]).unwrap());
    if !cat_hash.is_empty() {
        if let Ok(hdr) = Header::from_bytes(&b"ETag"[..], format!("\"{}\"", cat_hash).as_bytes()) {
            resp.add_header(hdr);
        }
    }
}
