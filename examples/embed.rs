use failure::Error;
use std::collections::HashMap;
use std::fs::File;
use std::io::prelude::*;
use std::path::PathBuf;
use wasi_common::preopen_dir;
use wasmtime_embed::{
    create_wasi, instantiate, instantiate_in_context, wasm_export_impl, wasm_import_wrapper,
    ContextToken, ImportSet, InstanceToken, RuntimeValue, WasmExport,
};

use wasmtime_embed_macro::{wasm_export, wasm_import};

#[wasm_export]
trait Test {
    fn gcd(&self, a: u32, b: u32) -> u32;
    fn test(&self) -> u32;
}

#[wasm_import]
trait TestCallback {
    fn callback(&self, c: u32);
}

struct TestCallbackC {
    _i: u32,
}

impl TestCallbackC {
    fn new() -> Self {
        TestCallbackC { _i: 0 }
    }
}

impl TestCallback for TestCallbackC {
    fn callback(&self, c: u32) {
        println!("callback (from TestCallbackC): {}", c);
    }
}

fn read_binary(path: &str) -> Result<Vec<u8>, Error> {
    let path = PathBuf::from(path);
    let mut buf: Vec<u8> = Vec::new();
    File::open(path)?.read_to_end(&mut buf)?;
    Ok(buf)
}

fn main() -> Result<(), Error> {
    // Instantiate gcd.wasm without imports
    let gcd_wasm = read_binary("gcd.wasm")?;
    let instance = instantiate(&gcd_wasm, HashMap::new())?;

    // "Map" `Test` trait to instance
    let t = wasm_export_impl!(instance as Test);
    // Direct call of wasm's `gcd` (no late binding)
    println!("gcd(6, 27) = {} (via Test)", t.gcd(6, 27));

    // Late binding
    let gcd = instance.get_export("gcd").expect("gcd test");
    let res = gcd.invoke(&[RuntimeValue::I32(6), RuntimeValue::I32(27)])?;
    println!("gcd(6, 27) = {}", res[0]);

    // InstanceHandle for wrapped Rust struct (TestCallback trait)
    let callback_host = TestCallbackC::new();
    let l0 = wasm_import_wrapper!(callback_host for <TestCallbackC as TestCallback>);

    // Instantiate l1.wasm with "test" and "gcd" imports. The former is Rust object
    // and the latter is wasm module. Communication using direct calls.
    let l1_wasm = read_binary("l1.wasm")?;
    let mut l1_imports = HashMap::new();
    l1_imports.insert(String::from("test"), ImportSet::InstanceExports(l0));
    l1_imports.insert(String::from("gcd"), ImportSet::InstanceExports(instance));
    let _l1 = instantiate(&l1_wasm, l1_imports)?;

    // For wasi, we need the same context (just to have a common "memory").
    let context = ContextToken::create();

    // Instantiate WASI (as InstanceHandle)
    let wasi = build_wasi(&context);
    // Instantiate hello.wasm with wasi as import (in the same context).
    let hello_wasm = read_binary("hello.wasm")?;
    let mut hello_imports = HashMap::new();
    hello_imports.insert(
        String::from("wasi_unstable"),
        ImportSet::InstanceExports(wasi),
    );
    let hello = instantiate_in_context(&hello_wasm, hello_imports, context)?;

    // Accessing memory slice (example).
    let memory = hello.get_export("memory").expect("memory");
    unsafe {
        let data: &[u8] = memory.get_memory_slice_mut(100000, 100, 1)?;
        println!("data: {:?}", data);
    }

    Ok(())
}

fn build_wasi(context: &ContextToken) -> InstanceToken {
    let preopen_dirs = compute_preopen_dirs(
        &vec![String::from(".")],
        &vec![String::from("/tmp:/var/tmp")],
    );
    let argv: Vec<String> = vec![String::from("test")];
    let environ: Vec<(String, String)> = vec![];
    create_wasi(context.clone(), &preopen_dirs, &argv, &environ)
}

fn compute_preopen_dirs(flag_dir: &[String], flag_mapdir: &[String]) -> Vec<(String, File)> {
    let mut preopen_dirs = Vec::new();

    for dir in flag_dir {
        let preopen_dir = preopen_dir(dir).unwrap_or_else(|err| {
            panic!("error while pre-opening directory {}: {}", dir, err);
        });
        preopen_dirs.push((dir.clone(), preopen_dir));
    }

    for mapdir in flag_mapdir {
        let parts: Vec<&str> = mapdir.split(':').collect();
        if parts.len() != 2 {
            panic!("--mapdir argument must contain exactly one colon, separating a guest directory name and a host directory name");
        }
        let (key, value) = (parts[0], parts[1]);
        let preopen_dir = preopen_dir(value).unwrap_or_else(|err| {
            panic!("error while pre-opening directory {}: {}", value, err);
        });
        preopen_dirs.push((key.to_string(), preopen_dir));
    }

    preopen_dirs
}
