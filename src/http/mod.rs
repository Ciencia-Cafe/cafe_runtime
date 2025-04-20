use deno_core::extension;
use serve::{op_http_accept, op_http_serve};

pub mod serve;

extension!(
    runtime_http,
    ops = [
        op_http_serve,
        op_http_accept,
    ],
    esm_entry_point = "ext:runtime_http/01.js",
    esm = [dir "src/http/js", "01.js"]
);
