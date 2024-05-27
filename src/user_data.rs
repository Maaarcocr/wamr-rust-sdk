use std::{ffi::c_void, marker::PhantomData};

pub struct UserData<'a, T> {
    _data: PhantomData<&'a T>,
    ptr: *mut c_void,
}

pub type ExecEnv = wamr_sys::wasm_exec_env_t;

impl<'a, T> UserData<'a, T> {
    pub fn from_env(env: ExecEnv) -> Self {
        let ptr = unsafe { wamr_sys::wasm_runtime_get_user_data(env) };
        UserData {
            _data: PhantomData,
            ptr,
        }
    }

    pub fn data(&self) -> &'a T {
        unsafe { &*(self.ptr as *const T) }
    }

    pub fn data_mut(&mut self) -> &'a mut T {
        unsafe { &mut *(self.ptr as *mut T) }
    }
}