/*
 * Copyright (C) 2019 Intel Corporation. All rights reserved.
 * SPDX-License-Identifier: Apache-2.0 WITH LLVM-exception
 */

/// This is a wrapper of a host defined(Rust) function.
use std::ffi::{c_void, CString};
use std::ptr;

use wamr_sys::NativeSymbol;

pub enum ParamTy {
    I32,
    I64,
    F32,
    F64,
    Str,
    Pointer,
    Buffer,
}

impl ParamTy {
    pub fn encode(&self, str: &mut Vec<u8>) {
        match self {
            ParamTy::I32 => str.push(b'i'),
            ParamTy::I64 => str.push(b'I'),
            ParamTy::F32 => str.push(b'f'),
            ParamTy::F64 => str.push(b'F'),
            ParamTy::Str => str.push(b'$'),
            ParamTy::Pointer => str.push(b'*'),
            ParamTy::Buffer => str.extend_from_slice(b"*~"),
        }
    }
}

pub enum ResultTy {
    I32,
    I64,
    F32,
    F64,
    Void,
}

impl ResultTy {
    pub fn encode(&self, str: &mut Vec<u8>) {
        match self {
            ResultTy::I32 => str.push(b'i'),
            ResultTy::I64 => str.push(b'I'),
            ResultTy::F32 => str.push(b'f'),
            ResultTy::F64 => str.push(b'F'),
            ResultTy::Void => {}
        }
    }
}

#[allow(dead_code)]
#[derive(Debug)]
struct HostFunction {
    function_name: CString,
    function_ptr: *mut c_void,
    signature: CString,
}

#[derive(Debug)]
pub struct HostFunctionList {
    pub module_name: CString,
    // keep ownership of the content of `native_symbols`
    host_functions: Vec<HostFunction>,
    pub native_symbols: Vec<NativeSymbol>,
}

impl HostFunctionList {
    pub fn new(module_name: &str) -> Self {
        HostFunctionList {
            module_name: CString::new(module_name).unwrap(),
            host_functions: Vec::new(),
            native_symbols: Vec::new(),
        }
    }

    pub fn register_host_function(&mut self, function_name: &str, function_ptr: *mut c_void, params: &[ParamTy], result: ResultTy) {
        let mut signature = Vec::new();
        for param in params {
            param.encode(&mut signature);
        }
        result.encode(&mut signature);
        let signature = CString::new(signature).unwrap();

        self.host_functions.push(HostFunction {
            function_name: CString::new(function_name).unwrap(),
            function_ptr,
            signature,
        });

        let last = self.host_functions.last().unwrap();
        self.native_symbols
            .push(pack_host_function(&(last.function_name), function_ptr, &(last.signature)));
    }

    pub fn get_native_symbols(&mut self) -> &mut Vec<NativeSymbol> {
        &mut self.native_symbols
    }

    pub fn get_module_name(&mut self) -> &CString {
        &self.module_name
    }
}

fn pack_host_function(function_name: &CString, function_ptr: *mut c_void, signature: &CString) -> NativeSymbol {
    NativeSymbol {
        symbol: function_name.as_ptr(),
        func_ptr: function_ptr,
        signature: signature.as_ptr(),
        attachment: ptr::null_mut(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::user_data::{ExecEnv, Caller};
    use crate::{
        function::Function, instance::Instance, module::Module, runtime::Runtime, value::WasmValue,
    };
    use std::env;
    use std::path::PathBuf;

    extern "C" fn extra() -> i32 {
        100
    }

    #[test]
    fn test_host_function() {
        let runtime = Runtime::builder()
            .use_system_allocator()
            .register_host_function("extra", extra as *mut c_void, &[], ResultTy::I32)
            .build()
            .unwrap();

        let mut d = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
        d.push("resources/test");
        d.push("add_extra_wasm32_wasi.wasm");
        let module = Module::from_file(&runtime, d.as_path());
        assert!(module.is_ok());
        let module = module.unwrap();

        let instance = Instance::new(&runtime, &module, 1024 * 64, ());
        assert!(instance.is_ok());
        let instance: &Instance<()> = &instance.unwrap();

        let function = Function::find_export_func(instance, "add");
        assert!(function.is_ok());
        let function = function.unwrap();

        let params: Vec<WasmValue> = vec![WasmValue::I32(8), WasmValue::I32(8)];
        let result = function.call(instance, &params);
        assert_eq!(result.unwrap(), WasmValue::I32(116));
    }

    struct Counter {
        count: i32,
    }

    extern "C" fn extra_with_side_effect(env: ExecEnv) -> i32 {
        let mut user_data: Caller<Counter> = Caller::from_env(env);
        let count = user_data.data_mut();
        count.count += 1;
        count.count
    }

    #[test]
    fn test_host_function_with_side_effect() {
        let runtime = Runtime::builder()
            .use_system_allocator()
            .register_host_function("extra", extra_with_side_effect as *mut c_void, &[], ResultTy::I32)
            .build()
            .unwrap();

        let mut d = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
        d.push("resources/test");
        d.push("add_extra_wasm32_wasi.wasm");
        let module = Module::from_file(&runtime, d.as_path());
        assert!(module.is_ok());
        let module = module.unwrap();

        let instance = Instance::new(&runtime, &module, 1024 * 64, Counter { count: 0 });
        assert!(instance.is_ok());
        let instance: &Instance<Counter> = &instance.unwrap();

        let function = Function::find_export_func(instance, "add");
        assert!(function.is_ok());
        let function = function.unwrap();

        let params: Vec<WasmValue> = vec![WasmValue::I32(8), WasmValue::I32(8)];
        let result = function.call(instance, &params);
        assert_eq!(result.unwrap(), WasmValue::I32(17));

        let params: Vec<WasmValue> = vec![WasmValue::I32(8), WasmValue::I32(8)];
        let result = function.call(instance, &params);
        assert_eq!(result.unwrap(), WasmValue::I32(18));
    }
}
