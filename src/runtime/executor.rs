use std::rc::Rc;

use deno_core::{Extension, anyhow, extension};
use deno_core::{FsModuleLoader, JsRuntime, RuntimeOptions};

use crate::http::runtime_http;

extension!(
    runtime,
    esm_entry_point = "ext:runtime/bootstrap.js",
    esm = [dir "src/runtime", "bootstrap.js"]
);

pub async fn execute_js(file_path: &str) -> deno_core::anyhow::Result<()> {
    let main_module =
        deno_core::resolve_path(file_path, &std::env::current_dir()?)?;

    let extensions: Vec<Extension> = vec![runtime_http::init_ops_and_esm()];

    let mut js_runtime = JsRuntime::new(RuntimeOptions {
        module_loader: Some(Rc::new(FsModuleLoader)),
        extensions,
        ..Default::default()
    });

    let mod_id = js_runtime.load_main_es_module(&main_module).await?;
    let result = js_runtime.mod_evaluate(mod_id);
    js_runtime.run_event_loop(Default::default()).await?;
    result.await.map_err(anyhow::Error::from)
}
