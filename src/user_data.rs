use std::{ffi::c_void, marker::PhantomData};

pub struct Caller<'a, T> {
    _data: PhantomData<&'a T>,
    ptr: *mut c_void,
}

pub type ExecEnv = wamr_sys::wasm_exec_env_t;

impl<'a, T> Caller<'a, T> {
    pub fn from_env(env: ExecEnv) -> Self {
        let ptr = unsafe { wamr_sys::wasm_runtime_get_user_data(env) };
        Caller {
            _data: PhantomData,
            ptr,
        }
    }

    pub fn data(&'a self) -> &'a T {
        unsafe { &*(self.ptr as *const T) }
    }

    pub fn data_mut(&'a mut self) -> &'a mut T {
        unsafe { &mut *(self.ptr as *mut T) }
    }
}