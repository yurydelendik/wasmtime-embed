//! Embeddable classes.

#![deny(
    //missing_docs,
    trivial_numeric_casts,
    unused_extern_crates,
    unstable_features
)]
#![warn(unused_import_braces)]
#![cfg_attr(feature = "clippy", plugin(clippy(conf_file = "../clippy.toml")))]
#![cfg_attr(
    feature = "cargo-clippy",
    allow(clippy::new_without_default, clippy::new_without_default_derive)
)]
#![cfg_attr(
    feature = "cargo-clippy",
    warn(
        clippy::float_arithmetic,
        clippy::mut_mut,
        clippy::nonminimal_bool,
        clippy::option_map_unwrap_or,
        clippy::option_map_unwrap_or_else,
        clippy::unicode_not_nfc,
        clippy::use_self
    )
)]

#[macro_use]
extern crate failure_derive;

mod context;
mod imports;
mod instance;
mod instantiate;
mod wasi;

pub mod extra;

pub use crate::context::ContextToken;
pub use crate::imports::{Import, ImportSet};
pub use crate::instance::{InstanceCallableExport, InstanceExport, InstanceToken};
pub use crate::instantiate::{instantiate, instantiate_in_context};
pub use crate::wasi::create_wasi;
pub use wasmtime_jit::RuntimeValue;

pub trait WasmExport {
    type Concrete;
    fn export(i: InstanceToken) -> Self::Concrete;
}

#[macro_export]
macro_rules! wasm_export_impl {
    ($t:ident as $int:path) => {
        (<$int as ::wasmtime_embed::WasmExport>::Concrete::export($t.clone()))
    };
    ( ( $t:expr ) as $int:path) => {
        (<$int as ::wasmtime_embed::WasmExport>::Concrete::export(($t).clone()))
    };
}

#[macro_export]
macro_rules! wasm_import_wrapper {
    ($t:ident for < $int_t:ty as $int:path > ) => {
        <$int_t as $int>::wrap_wasm_imports::<$int_t>($t)
    };
    (($t:expr) for < $int_t:ty as $int:path > ) => {
        <$int_t as $int>::wrap_wasm_imports::<$int_t>($t)
    };
}
