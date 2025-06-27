use deno_core::extension;
use http_stream::{
    op_http_get_method, op_http_read_request_body_text,
    op_http_set_promise_complete,
};
use serve::{op_http_serve, op_http_wait};

mod http_stream;
pub mod serve;

extension!(
    runtime_http,
    ops = [
        op_http_serve,
        op_http_wait,
        op_http_get_method,
        op_http_set_promise_complete,
        op_http_read_request_body_text,
    ],
    esm_entry_point = "ext:runtime_http/http.js",
    esm = [
        dir "src/http/js",
        "http.js",
        "http_stream.js",
        "utils.js"
    ]
);
