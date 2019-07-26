use crate::context::ContextToken;
use crate::instance::InstanceToken;
use std::collections::HashSet;
use std::fs::File;
use wasmtime_wasi::instantiate_wasi;

pub fn create_wasi(
    mut context: ContextToken,
    preopen_dirs: &[(String, File)],
    argv: &[String],
    environ: &[(String, String)],
) -> InstanceToken {
    let global_exports = context.context().get_global_exports();
    let handle =
        instantiate_wasi("", global_exports, &preopen_dirs, &argv, &environ).expect("wasi");

    let mut contexts = HashSet::new();
    contexts.insert(context);

    InstanceToken::new(handle, contexts)
}
