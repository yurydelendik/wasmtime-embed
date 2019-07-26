use crate::instance::{InstanceExport, InstanceToken};
use std::collections::HashMap;

pub enum Import {
    InstanceExport(InstanceExport),
    I32(i32),
}

pub enum ImportSet {
    InstanceExports(InstanceToken),
    Fields(HashMap<String, Import>),
}
