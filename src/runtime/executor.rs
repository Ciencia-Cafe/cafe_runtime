use std::rc::Rc;

use deno_core::error::JsError;
use deno_core::{Extension, ModuleSpecifier, anyhow, extension, v8};
use deno_core::{FsModuleLoader, JsRuntime, RuntimeOptions};
use deno_error::JsError;

use crate::http::runtime_http;

extension!(
    runtime,
    esm_entry_point = "ext:runtime/bootstrap.js",
    esm = [dir "src/runtime/js", "bootstrap.js"]
);

pub async fn execute_js(file_path: &str) -> deno_core::anyhow::Result<()> {
    let main_module =
        deno_core::resolve_path(file_path, &std::env::current_dir()?)?;

    let extensions: Vec<Extension> = vec![
        runtime_http::init_ops_and_esm(),
        runtime::init_ops_and_esm(),
    ];

    let mut js_runtime = JsRuntime::new(RuntimeOptions {
        module_loader: Some(Rc::new(FsModuleLoader)),
        extensions,
        is_main: true,
        ..Default::default()
    });

    let bootstrap_fn = {
        let context = js_runtime.main_context();
        let scope = &mut js_runtime.handle_scope();
        let context_local = v8::Local::new(scope, context);
        let global_obj = context_local.global(scope);

        let bootstrap_ns_symbol =
            v8::String::new_external_onebyte_static(scope, b"bootstrap")
                .unwrap();
        let bootstrap_ns: v8::Local<v8::Object> = global_obj
            .get(scope, bootstrap_ns_symbol.into())
            .unwrap()
            .try_into()
            .unwrap();

        let main_runtime_fn_symbol =
            v8::String::new_external_onebyte_static(scope, b"mainRuntime")
                .unwrap();

        let bootstrap_fn = bootstrap_ns
            .get(scope, main_runtime_fn_symbol.into())
            .unwrap();

        let bootstrap_fn =
            v8::Local::<v8::Function>::try_from(bootstrap_fn).unwrap();

        v8::Global::new(scope, bootstrap_fn)
    };

    // bootstrap
    {
        let scope = &mut js_runtime.handle_scope();
        let scope = &mut v8::TryCatch::new(scope);

        let bootstrap_fn = v8::Local::new(scope, bootstrap_fn);
        let undefined = v8::undefined(scope);
        bootstrap_fn.call(scope, undefined.into(), &[]);

        if let Some(exception) = scope.exception() {
            let error = JsError::from_v8_exception(scope, exception);
            panic!("Bootstrap exception: {error}");
        }
    }

    let mod_id = js_runtime.load_main_es_module(&main_module).await?;
    let result = js_runtime.mod_evaluate(mod_id);
    js_runtime.run_event_loop(Default::default()).await?;

    result.await.map_err(anyhow::Error::from)
}
