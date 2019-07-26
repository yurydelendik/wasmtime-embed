use crate::context::ContextToken;
use crate::imports::ImportSet;
use crate::instance::InstanceToken;
use failure::Error;
use std::collections::{HashMap, HashSet};

pub fn instantiate_in_context(
    data: &[u8],
    imports: HashMap<String, ImportSet>,
    mut context_token: ContextToken,
) -> Result<InstanceToken, Error> {
    let mut contexts = HashSet::new();
    let instance = {
        let mut context = context_token.context();

        for (name, set) in imports {
            match set {
                ImportSet::InstanceExports(i) => {
                    context.name_instance(name.clone(), i.handle().clone());
                    contexts.extend(i.contexts().clone());
                }
                _ => panic!("unsupported ImportSet"),
            }
        }
        context.instantiate_module(None, &data)?
    };
    contexts.insert(context_token);

    Ok(InstanceToken::new(instance, contexts))
}

pub fn instantiate(
    data: &[u8],
    imports: HashMap<String, ImportSet>,
) -> Result<InstanceToken, Error> {
    let context_grip = ContextToken::create();
    instantiate_in_context(data, imports, context_grip)
}
