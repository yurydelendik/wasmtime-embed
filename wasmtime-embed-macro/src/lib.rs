#![recursion_limit = "128"]

extern crate proc_macro;
extern crate proc_macro2;
#[macro_use]
extern crate syn;
extern crate quote;

use proc_macro::TokenStream;
use proc_macro2::{Span, TokenStream as TokenStream2};
use quote::quote;
use syn::{
    parse, parse_macro_input, ArgCaptured, FnArg, Ident, ItemTrait, MethodSig, Pat, PatIdent,
    ReturnType, TraitItem, TraitItemMethod, Type, TypeParamBound,
};

fn convert_type(ty: &Type) -> TokenStream2 {
    match ty {
        Type::Path(p) if p.path.is_ident("u32") => quote! { ir::types::I32 },
        Type::Path(p) if p.path.is_ident("u64") => quote! { ir::types::I64 },
        Type::Path(p) if p.path.is_ident("f32") => quote! { ir::types::F32 },
        Type::Path(p) if p.path.is_ident("f64") => quote! { ir::types::F64 },
        _ => panic!("unsupported type"),
    }
}

fn convert_method_sig(sig: &MethodSig) -> (TokenStream2, TokenStream2, TokenStream2, TokenStream2) {
    if let FnArg::SelfRef(_) = sig.decl.inputs[0] {
        ()
    } else {
        panic!("&self is required for method");
    }

    let mut ty_args = TokenStream2::new();
    let mut params = TokenStream2::new();
    let mut call_passthru_params = TokenStream2::new();

    for param in &sig.decl.inputs {
        match param {
            FnArg::SelfRef(_) => {
                ty_args.extend(quote! { *mut VMContext });
                params.extend(quote! {
                    ir::AbiParam::special(ir::types::I64, ir::ArgumentPurpose::VMContext)
                });
            }
            FnArg::Captured(ArgCaptured {
                pat:
                    Pat::Ident(PatIdent {
                        by_ref: None,
                        mutability: None,
                        ident,
                        subpat: None,
                    }),
                ty,
                ..
            }) => {
                ty_args.extend(quote! { , #ty });
                let param_type = convert_type(ty);
                params.extend(quote! { , ir::AbiParam::new(#param_type) });
                call_passthru_params.extend(quote! {, #ident });
            }
            _ => panic!("unsupported param type"),
        }
    }

    let mut ty_ret = TokenStream2::new();
    let mut returns = TokenStream2::new();
    match sig.decl.output {
        ReturnType::Default => (),
        ReturnType::Type(_, ref ty) => {
            ty_ret = quote! { -> #ty };
            let return_type = convert_type(ty);
            returns.extend(quote! {
                ir::AbiParam::new(#return_type)
            });
        }
    }

    let ty = quote! {
        unsafe extern "sysv64" fn(#ty_args) #ty_ret
    };

    (ty, params, returns, call_passthru_params)
}

fn convert_method(
    method: &TraitItemMethod,
    fields: &mut TokenStream2,
    metas: &mut TokenStream2,
    inits: &mut TokenStream2,
    proxies: &mut TokenStream2,
) {
    let (ty, params, returns, call_passthru_params) = convert_method_sig(&method.sig);

    let method_name = method.sig.ident.clone();
    let wasm_name = method_name.to_string();
    let field_name = Ident::new(&format!("{}_", method_name), Span::call_site());
    fields.extend(quote! { #field_name: (*mut VMContext, #ty), });

    metas.extend(quote! {
        let #field_name = instance.get_callable_export(
            #wasm_name,
            ir::Signature {
                params: vec![#params],
                returns: vec![#returns],
                call_conv: isa::CallConv::SystemV,
            }
        ).expect("valid callable export").vmctx_and_body();
    });
    inits.extend(quote! {
        #field_name: (
            #field_name.0,
            unsafe { std::mem::transmute(#field_name.1) }
        ),
    });

    let method_sig = &method.sig;
    proxies.extend(quote! {
        #method_sig {
            let f = self.#field_name.1;
            unsafe { f(self.#field_name.0 #call_passthru_params) }
        }
    });
}

fn extend_trait_with_wasm_derive(ast: &mut ItemTrait, extra_mod_indent: &Ident) {
    ast.colon_token = Some(Token![:](Span::call_site()));
    let ts = TokenStream::from(quote! {
        ::wasmtime_embed::WasmExport<Concrete=#extra_mod_indent::Impl>
    });
    ast.supertraits.push(parse::<TypeParamBound>(ts).expect(""));
}

#[proc_macro_attribute]
pub fn wasm_export(attr: TokenStream, item: TokenStream) -> TokenStream {
    if attr.to_string() != "" {
        panic!("expecting no attr");
    }

    let mut ast = parse_macro_input!(item as ItemTrait);
    let extra_mod_name = format!("_{}_wasm_export", ast.ident.clone());

    let trait_name = ast.ident.clone();
    let extra_mod_indent = Ident::new(&extra_mod_name, Span::call_site());

    extend_trait_with_wasm_derive(&mut ast, &extra_mod_indent);

    let mut fields = TokenStream2::new();
    let mut metas = TokenStream2::new();
    let mut inits = TokenStream2::new();
    let mut proxies = TokenStream2::new();

    for item in &ast.items {
        match item {
            TraitItem::Method(ref method) => {
                convert_method(method, &mut fields, &mut metas, &mut inits, &mut proxies);
            }
            _ => {
                panic!("Unexpected trait type: {:?}", item);
            }
        }
    }

    let implementations = quote! {
            impl WasmExport for Impl {
                type Concrete = Impl;
                fn export(instance: InstanceToken) -> Impl {
                    #metas
                    Impl {
                        instance,
                        #inits
                    }
                }
            }

            impl super::#trait_name for Impl {
                #proxies
            }
    };
    let extra = quote! {
        mod #extra_mod_indent {
            use ::wasmtime_embed::{InstanceToken, WasmExport, InstanceCallableExport};
            use ::wasmtime_embed::extra::{VMContext, VMFunctionBody, ir, isa};

            pub struct Impl {
                instance: InstanceToken,
                #fields
            }

            #implementations
        }
    };

    TokenStream::from(quote! {
        #ast
        #extra
    })
}

fn convert_method_sig2(
    sig: &MethodSig,
) -> (TokenStream2, TokenStream2, TokenStream2, TokenStream2) {
    if let FnArg::SelfRef(_) = sig.decl.inputs[0] {
        ()
    } else {
        panic!("&self is required for method");
    }

    let name = sig.ident.clone();
    let mut ty_args = TokenStream2::new();
    let mut params = TokenStream2::new();
    let mut call_passthru_params = TokenStream2::new();

    for param in &sig.decl.inputs {
        match param {
            FnArg::SelfRef(_) => {
                ty_args.extend(quote! { vmctx: *mut VMContext });
                params.extend(quote! {
                    ir::AbiParam::special(ir::types::I64, ir::ArgumentPurpose::VMContext)
                });
            }
            FnArg::Captured(ArgCaptured {
                pat:
                    Pat::Ident(PatIdent {
                        by_ref: None,
                        mutability: None,
                        ident,
                        subpat: None,
                    }),
                ty,
                ..
            }) => {
                ty_args.extend(quote! { , #ident: #ty });
                let param_type = convert_type(ty);
                params.extend(quote! { , ir::AbiParam::new(#param_type) });
                if !call_passthru_params.is_empty() {
                    call_passthru_params.extend(quote! { , });
                }
                call_passthru_params.extend(quote! { #ident });
            }
            _ => panic!("unsupported param type"),
        }
    }

    let mut ty_ret = TokenStream2::new();
    let mut returns = TokenStream2::new();
    match sig.decl.output {
        ReturnType::Default => (),
        ReturnType::Type(_, ref ty) => {
            ty_ret = quote! { -> #ty };
            let return_type = convert_type(ty);
            returns.extend(quote! {
                ir::AbiParam::new(#return_type)
            });
        }
    }

    let ty = quote! {
        unsafe extern "sysv64" fn #name(#ty_args) #ty_ret
    };

    (ty, params, returns, call_passthru_params)
}

fn wrap_method(
    method: &TraitItemMethod,
    extra_mod_indent: &Ident,
    definitions: &mut TokenStream2,
    wrapper_methods: &mut TokenStream2,
) {
    let (sig, params, returns, call_passthru_params) = convert_method_sig2(&method.sig);

    let method_name = method.sig.ident.clone();
    let wasm_name = method_name.to_string();

    definitions.extend(quote! {
        let sig = module.signatures.push(
            ir::Signature {
                params: vec![#params],
                returns: vec![#returns],
                call_conv: isa::CallConv::SystemV,
            }
        );
        let func = module.functions.push(sig);
        module.exports.insert(
            #wasm_name . to_owned(),
            Export::Function(func),
        );
        finished_functions.push(#extra_mod_indent :: #method_name as *const VMFunctionBody);
    });
    wrapper_methods.extend(quote! {
        pub (super) #sig {
            get_state(vmctx).subject.borrow().callback(#call_passthru_params)
        }
    });
}

#[proc_macro_attribute]
pub fn wasm_import(attr: TokenStream, item: TokenStream) -> TokenStream {
    if attr.to_string() != "" {
        panic!("expecting no attr");
    }

    let mut ast = parse_macro_input!(item as ItemTrait);
    let trait_ident = ast.ident.clone();
    let vis = ast.vis.clone();
    let extra_mod_name = format!("_{}_wasm_import", ast.ident.clone());
    let extra_mod_indent = Ident::new(&extra_mod_name, Span::call_site());

    let wrap_defs = quote! {
        use ::wasmtime_embed::{ContextToken, InstanceToken};
        use ::wasmtime_embed::extra::{
            Export, Module, Imports, InstanceHandle, InstantiationError, VMFunctionBody,
            ir, isa, PrimaryMap, DefinedFuncIndex,
        };
        use ::std::rc::Rc;
        use ::std::cell::RefCell;

        let mut context = ContextToken::create();
        let imports = Imports::none();
        let data_initializers = ::std::vec::Vec::new();
        let signatures = PrimaryMap::new();
        let global_exports = context.context().get_global_exports();
        let mut finished_functions = PrimaryMap::new();
        let mut module = Module::new();
    };

    let mut definitions = TokenStream2::new();
    let mut wrapper_methods = TokenStream2::new();
    for item in &ast.items {
        match item {
            TraitItem::Method(ref method) => {
                wrap_method(
                    method,
                    &extra_mod_indent,
                    &mut definitions,
                    &mut wrapper_methods,
                );
            }
            _ => {
                panic!("Unexpected trait type: {:?}", item);
            }
        }
    }

    let wrap_return = quote! {
        let mut contexts = ::std::collections::HashSet::new();
        contexts.insert(context);
        InstanceToken::new(
            InstanceHandle::new(
                Rc::new(module),
                global_exports,
                finished_functions.into_boxed_slice(),
                imports,
                &data_initializers,
                signatures.into_boxed_slice(),
                None,
                ::std::boxed::Box::new(#extra_mod_indent :: State {
                    subject: RefCell::new(::std::boxed::Box::new(subject))
                }),
            ).expect("handle"),
            contexts
        )
    };

    let wrap_method = TokenStream::from(quote! {
        fn wrap_wasm_imports<T: #trait_ident + 'static>(
            subject: T
        ) -> ::wasmtime_embed::InstanceToken where Self: Sized {
            #wrap_defs
            #definitions
            #wrap_return
        }
    });
    ast.items.extend(parse::<TraitItem>(wrap_method));

    let extra = quote! {
        #vis mod #extra_mod_indent {
            use ::wasmtime_embed::{InstanceToken, WasmExport, InstanceCallableExport};
            use ::wasmtime_embed::extra::{VMContext, VMFunctionBody, ir, isa};

            pub (super) struct State {
                pub subject: ::std::cell::RefCell<
                    ::std::boxed::Box<dyn super::#trait_ident + 'static>
                >,
            }
            unsafe fn get_state<'a>(vmctx: *mut VMContext) -> &'a mut State {
                &mut *(&mut *vmctx).host_state().downcast_mut::<State>().unwrap()
            }
            #wrapper_methods
        }
    };

    TokenStream::from(quote! {
        #ast
        #extra
    })
}
