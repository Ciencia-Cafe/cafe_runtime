use std::env;

use deno_server::runtime::executor::execute;

fn main() {
    let args = &env::args().collect::<Vec<String>>()[1..];

    if args.is_empty() {
        eprintln!("Usage: deno_server <file>");
        std::process::exit(1);
    }
    let file_path = &args[0];
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap();

    if let Err(error) = runtime.block_on(execute(file_path)) {
        eprintln!("error: {}", error);
    }
}
