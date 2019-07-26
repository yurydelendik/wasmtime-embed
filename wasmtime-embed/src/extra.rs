pub use cranelift_codegen::{ir, isa};
pub use cranelift_entity::PrimaryMap;
pub use cranelift_wasm::DefinedFuncIndex;
pub use wasmtime_environ::{Export, Module};
pub use wasmtime_runtime::{
    Imports, InstanceHandle, InstantiationError, VMContext, VMFunctionBody,
};
