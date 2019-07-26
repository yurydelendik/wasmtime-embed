use crate::context::{create_context, ContextToken};
use cranelift_codegen::ir;
use failure::Error;
use std::collections::HashSet;
use std::rc::Rc;
use wasmtime_jit::{ActionOutcome, RuntimeValue};
use wasmtime_runtime::{Imports, Export, InstanceHandle, VMContext, VMFunctionBody};
use std::any::Any;
use cranelift_entity::{PrimaryMap, BoxedSlice};
use cranelift_wasm::DefinedFuncIndex;
use wasmtime_environ::Module;

#[derive(Clone)]
pub struct InstanceToken {
    instance_handle: InstanceHandle,

    // We need to keep CodeMemory alive.
    contexts: HashSet<ContextToken>,
}

impl InstanceToken {
    pub fn handle(&self) -> &InstanceHandle {
        &self.instance_handle
    }

    pub(crate) fn contexts(&self) -> &HashSet<ContextToken> {
        &self.contexts
    }

    pub fn new(instance_handle: InstanceHandle, contexts: HashSet<ContextToken>) -> InstanceToken {
        InstanceToken {
            instance_handle,
            contexts,
        }
    }

    pub fn from_handle(handle: InstanceHandle) -> InstanceToken {
        InstanceToken {
            instance_handle: handle,
            contexts: HashSet::new(),
        }
    }

    pub fn from_raw_parts(
        module: Module, 
        finished_functions: BoxedSlice<DefinedFuncIndex, *const VMFunctionBody>,
        state: Box<dyn Any>
    ) -> InstanceToken {
        let imports = Imports::none();
        let data_initializers = Vec::new();
        let signatures = PrimaryMap::new();

        let mut context = ContextToken::create();
        let global_exports = context.context().get_global_exports();

        let mut contexts = HashSet::new();
        contexts.insert(context);

        InstanceToken::new(
            InstanceHandle::new(
                Rc::new(module),
                global_exports,
                finished_functions,
                imports,
                &data_initializers,
                signatures.into_boxed_slice(),
                None,
                state
            ).expect("handle"),
            contexts
        )
    }
}

#[derive(Fail, Debug)]
#[fail(display = "Callable export not found: {}", _0)]
pub struct CallableExportNotFound(String);

#[derive(Fail, Debug)]
#[fail(display = "Incompatible type for exported call {}: {}", _0, _1)]
pub struct CallableExportNotValidForSig(String, String);

#[derive(Fail, Debug)]
#[fail(display = "Trap from within function {}: {}", _0, _1)]
pub struct TrappedInvoke(String, String);

#[derive(Clone)]
pub struct InstanceExport {
    instance: InstanceToken,
    export_name: String,
}

impl InstanceExport {
    pub fn invoke(&self, args: &[RuntimeValue]) -> Result<Vec<RuntimeValue>, Error> {
        let mut context = create_context();
        let mut instance = self.instance.instance_handle.clone();
        Ok(
            match context.invoke(&mut instance, &self.export_name, args)? {
                ActionOutcome::Returned { values } => values,
                ActionOutcome::Trapped { message } => {
                    return Err(
                        TrappedInvoke(self.export_name.to_owned(), String::from(message)).into(),
                    );
                }
            },
        )
    }

    pub unsafe fn get_memory_slice_mut<'a, T>(
        &self,
        ptr: u32,
        len: usize,
        align: usize,
    ) -> Result<&'a mut [T], Error> {
        let mut instance = self.instance.instance_handle.clone();
        match instance.lookup(&self.export_name) {
            Some(Export::Memory {
                definition,
                vmctx: _,
                memory: _,
            }) => {
                if len > 0 {
                    // Check for overflow within the access.
                    let last = match (ptr as usize).checked_add(len - 1) {
                        Some(sum) => sum,
                        None => {
                            panic!("!!! overflow");
                        }
                    };
                    // Check for out of bounds.
                    if last >= (*definition).current_length {
                        panic!("!!! out of bounds");
                    }
                }
                // Check alignment.
                if (ptr as usize) % align != 0 {
                    panic!("!!! bad alignment: {} % {}", ptr, align);
                }
                // Ok, translate the address.
                let data = (((*definition).base as usize) + (ptr as usize)) as *mut T;
                Ok(std::slice::from_raw_parts_mut(data, len))
            }
            x => panic!("!!! no export memory, or the export isn't a mem: {:?}", x),
        }
    }
}

#[derive(Clone)]
pub struct InstanceCallableExport {
    instance: InstanceToken,
    vmctx: *mut VMContext,
    body: *const VMFunctionBody,
}

impl InstanceCallableExport {
    pub fn vmctx_and_body(&self) -> (*mut VMContext, *const VMFunctionBody) {
        (self.vmctx, self.body)
    }
}

impl InstanceToken {
    pub fn get_export(&self, name: &str) -> Option<InstanceExport> {
        let mut instance = self.clone();
        if instance.instance_handle.lookup(name).is_none() {
            return None;
        }
        Some(InstanceExport {
            instance,
            export_name: String::from(name),
        })
    }

    pub fn get_callable_export(
        &self,
        name: &str,
        sig: ir::Signature,
    ) -> Result<InstanceCallableExport, Error> {
        let mut instance = self.clone();
        match instance.instance_handle.lookup(name) {
            Some(wasmtime_runtime::Export::Function {
                address,
                signature,
                vmctx,
            }) => {
                if signature != sig {
                    return Err(CallableExportNotValidForSig(
                        name.to_owned(),
                        signature.to_string(),
                    )
                    .into());
                }
                return Ok(InstanceCallableExport {
                    instance,
                    vmctx,
                    body: address,
                });
            }
            _ => {
                return Err(CallableExportNotFound(name.to_owned()).into());
            }
        }
    }
}
