use std::cell::{RefCell, RefMut};
use std::hash::{Hash, Hasher};
use std::rc::Rc;

use wasmtime_jit::Context;

#[derive(Clone)]
pub struct ContextToken(Rc<RefCell<Context>>);

impl ContextToken {
    pub fn new(context: Context) -> ContextToken {
        ContextToken(Rc::new(RefCell::new(context)))
    }

    pub fn create() -> ContextToken {
        ContextToken(Rc::new(RefCell::new(create_context())))
    }

    pub fn context(&mut self) -> RefMut<Context> {
        self.0.borrow_mut()
    }
}

impl Hash for ContextToken {
    fn hash<H>(&self, state: &mut H)
    where
        H: Hasher,
    {
        unsafe {
            let ptr = Rc::into_raw(self.0.clone());
            let _ = Rc::from_raw(ptr);
            ptr
        }
        .hash(state)
    }
}

impl Eq for ContextToken {}

impl PartialEq for ContextToken {
    fn eq(&self, other: &ContextToken) -> bool {
        Rc::ptr_eq(&self.0, &other.0)
    }
}

pub(crate) fn create_context() -> Context {
    let generate_debug_info = false;
    let isa = {
        let isa_builder =
            cranelift_native::builder().expect("host machine is not a supported target");
        let flag_builder = cranelift_codegen::settings::builder();
        isa_builder.finish(cranelift_codegen::settings::Flags::new(flag_builder))
    };

    let mut context = Context::with_isa(isa);
    context.set_debug_info(generate_debug_info);
    context
}
