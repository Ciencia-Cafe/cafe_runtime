use deno_core::extension;
use http_stream::op_http_set_promise_complete;
use serve::{op_http_serve, op_http_wait};

mod http_stream;
pub mod serve;

extension!(
    runtime_http,
    ops = [
        op_http_serve,
        op_http_wait,
        op_http_set_promise_complete,
    ],
    esm_entry_point = "ext:runtime_http/http.js",
    esm = [
        dir "src/http/js",
        "http.js",
        "request.js"
    ]
);
