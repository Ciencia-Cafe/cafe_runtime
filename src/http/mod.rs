use deno_core::extension;
use serve::{op_http_serve, op_http_wait};

pub mod serve;

extension!(
    runtime_http,
    ops = [
        op_http_serve,
        op_http_wait,
    ],
    esm_entry_point = "ext:runtime_http/01.js",
    esm = [dir "src/http/js", "01.js"]
);
