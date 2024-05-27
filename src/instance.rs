/*
 * Copyright (C) 2019 Intel Corporation. All rights reserved.
 * SPDX-License-Identifier: Apache-2.0 WITH LLVM-exception
 */

//! an instantiated module. The module is instantiated with the given imports.
//! get one via `Instance::new()`

#![allow(unused_variables)]

use core::ffi::c_char;
use std::marker::PhantomData;

use wamr_sys::{
    wasm_module_inst_t, wasm_runtime_deinstantiate, wasm_runtime_destroy_thread_env,
    wasm_runtime_init_thread_env, wasm_runtime_instantiate,
};

use crate::{
    helper::error_buf_to_string, helper::DEFAULT_ERROR_BUF_SIZE, module::Module, runtime::Runtime,
    RuntimeError,
};

#[derive(Debug)]
pub struct Instance<T> {
    instance: wasm_module_inst_t,
    _data: PhantomData<T>
}

impl<T> Instance<T> {
    /// instantiate a module with stack size
    ///
    /// # Error
    ///
    /// Return `RuntimeError::CompilationError` if failed.
    pub fn new(runtime: &Runtime, module: &Module, stack_size: u32, data: T) -> Result<Self, RuntimeError> {
        Self::new_with_args(runtime, module, stack_size, 0, data)
    }

    /// instantiate a module with stack size and host managed heap size
    ///
    /// heap_size is used for `-nostdlib` Wasm and wasm32-unknown
    ///
    /// # Error
    ///
    /// Return `RuntimeError::CompilationError` if failed.
    pub fn new_with_args(
        _runtime: &Runtime,
        module: &Module,
        stack_size: u32,
        heap_size: u32,
        data: T,
    ) -> Result<Self, RuntimeError> {
        let init_thd_env = unsafe { wasm_runtime_init_thread_env() };
        if !init_thd_env {
            return Err(RuntimeError::InstantiationFailure(String::from(
                "thread signal env initialized failed",
            )));
        }

        let mut error_buf = [0 as c_char; DEFAULT_ERROR_BUF_SIZE];
        let instance = unsafe {
            wasm_runtime_instantiate(
                module.get_inner_module(),
                stack_size,
                heap_size,
                error_buf.as_mut_ptr(),
                error_buf.len() as u32,
            )
        };

        if instance.is_null() {
            match error_buf.len() {
                0 => {
                    return Err(RuntimeError::InstantiationFailure(String::from(
                        "instantiation failed",
                    )))
                }
                _ => {
                    return Err(RuntimeError::InstantiationFailure(error_buf_to_string(
                        &error_buf,
                    )))
                }
            }
        }

        let boxed_data = Box::new(data);
        let raw = Box::into_raw(boxed_data);
        let exec_env = unsafe { wamr_sys::wasm_runtime_get_exec_env_singleton(instance) };
        unsafe {
            wamr_sys::wasm_runtime_set_user_data(exec_env, raw as *mut std::ffi::c_void);
        }

        Ok(Instance { instance, _data: PhantomData })
    }

    pub fn get_inner_instance(&self) -> wasm_module_inst_t {
        self.instance
    }

    pub fn data(&self) -> &T {
        let raw_user_data = unsafe {
            let instance = self.get_inner_instance();
            let exec_env = wamr_sys::wasm_runtime_get_exec_env_singleton(instance);
            wamr_sys::wasm_runtime_get_user_data(exec_env)
        };
        unsafe { &*(raw_user_data as *const T) }
    }

    pub fn data_mut(&mut self) -> &mut T {
        let raw_user_data = unsafe {
            let instance = self.get_inner_instance();
            let exec_env = wamr_sys::wasm_runtime_get_exec_env_singleton(instance);
            wamr_sys::wasm_runtime_get_user_data(exec_env)
        };
        unsafe { &mut *(raw_user_data as *mut T) }
    }
}

impl<T> Drop for Instance<T> {
    fn drop(&mut self) {
        let raw_user_data = unsafe {
            let instance = self.get_inner_instance();
            let exec_env = wamr_sys::wasm_runtime_get_exec_env_singleton(instance);
            wamr_sys::wasm_runtime_get_user_data(exec_env)
        };
        if !raw_user_data.is_null() {
            let _ = unsafe { Box::from_raw(raw_user_data as *mut T) };
        }
        unsafe {
            wasm_runtime_destroy_thread_env();
            wasm_runtime_deinstantiate(self.instance);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::runtime::Runtime;
    use wamr_sys::{
        wasm_runtime_get_running_mode, RunningMode_Mode_Interp, RunningMode_Mode_LLVM_JIT,
    };

    #[test]
    fn test_instance_new() {
        let runtime = Runtime::new().unwrap();

        // (module
        //   (func (export "add") (param i32 i32) (result i32)
        //     (local.get 0)
        //     (local.get 1)
        //     (i32.add)
        //   )
        // )
        let binary = vec![
            0x00, 0x61, 0x73, 0x6d, 0x01, 0x00, 0x00, 0x00, 0x01, 0x07, 0x01, 0x60, 0x02, 0x7f,
            0x7f, 0x01, 0x7f, 0x03, 0x02, 0x01, 0x00, 0x07, 0x07, 0x01, 0x03, 0x61, 0x64, 0x64,
            0x00, 0x00, 0x0a, 0x09, 0x01, 0x07, 0x00, 0x20, 0x00, 0x20, 0x01, 0x6a, 0x0b,
        ];
        let binary = binary.into_iter().map(|c| c as u8).collect::<Vec<u8>>();

        let module = Module::from_buf(&runtime, &binary, "add");
        assert!(module.is_ok());

        let module = &module.unwrap();

        let instance = Instance::new_with_args(&runtime, module, 1024, 1024, ());
        assert!(instance.is_ok());

        let instance = Instance::new_with_args(&runtime, module, 1024, 0, ());
        assert!(instance.is_ok());

        let instance = instance.unwrap();
        assert_eq!(
            unsafe { wasm_runtime_get_running_mode(instance.get_inner_instance()) },
            if cfg!(feature = "llvmjit") {
                RunningMode_Mode_LLVM_JIT
            } else {
                RunningMode_Mode_Interp
            }
        );
    }

    #[test]
    #[ignore]
    fn test_instance_running_mode_default() {
        let runtime = Runtime::builder().use_system_allocator().build().unwrap();

        // (module
        //   (func (export "add") (param i32 i32) (result i32)
        //     (local.get 0)
        //     (local.get 1)
        //     (i32.add)
        //   )
        // )
        let binary = vec![
            0x00, 0x61, 0x73, 0x6d, 0x01, 0x00, 0x00, 0x00, 0x01, 0x07, 0x01, 0x60, 0x02, 0x7f,
            0x7f, 0x01, 0x7f, 0x03, 0x02, 0x01, 0x00, 0x07, 0x07, 0x01, 0x03, 0x61, 0x64, 0x64,
            0x00, 0x00, 0x0a, 0x09, 0x01, 0x07, 0x00, 0x20, 0x00, 0x20, 0x01, 0x6a, 0x0b,
        ];
        let binary = binary.into_iter().map(|c| c as u8).collect::<Vec<u8>>();

        let module = Module::from_buf(&runtime, &binary, "");
        assert!(module.is_ok());

        let module = &module.unwrap();

        let instance = Instance::new_with_args(&runtime, module, 1024, 1024, ());
        assert!(instance.is_ok());

        let instance = instance.unwrap();
        assert_eq!(
            unsafe { wasm_runtime_get_running_mode(instance.get_inner_instance()) },
            if cfg!(feature = "llvmjit") {
                RunningMode_Mode_LLVM_JIT
            } else {
                RunningMode_Mode_Interp
            }
        );
    }

    #[test]
    #[ignore]
    fn test_instance_running_mode_interpreter() {
        let runtime = Runtime::builder()
            .run_as_interpreter()
            .use_system_allocator()
            .build()
            .unwrap();

        // (module
        //   (func (export "add") (param i32 i32) (result i32)
        //     (local.get 0)
        //     (local.get 1)
        //     (i32.add)
        //   )
        // )
        let binary = vec![
            0x00, 0x61, 0x73, 0x6d, 0x01, 0x00, 0x00, 0x00, 0x01, 0x07, 0x01, 0x60, 0x02, 0x7f,
            0x7f, 0x01, 0x7f, 0x03, 0x02, 0x01, 0x00, 0x07, 0x07, 0x01, 0x03, 0x61, 0x64, 0x64,
            0x00, 0x00, 0x0a, 0x09, 0x01, 0x07, 0x00, 0x20, 0x00, 0x20, 0x01, 0x6a, 0x0b,
        ];
        let binary = binary.into_iter().map(|c| c as u8).collect::<Vec<u8>>();

        let module = Module::from_buf(&runtime, &binary, "add");
        assert!(module.is_ok());

        let module = &module.unwrap();

        let instance = Instance::new_with_args(&runtime, module, 1024, 1024, ());
        assert!(instance.is_ok());

        let instance = instance.unwrap();
        assert_eq!(
            unsafe { wasm_runtime_get_running_mode(instance.get_inner_instance()) },
            RunningMode_Mode_Interp
        );
    }
}
