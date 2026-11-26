// This read-only mapping is one of two alternating GPU output slots.
use super::io::Res;
use crate::cli::Dimensions;
use std::{ffi::c_void, fs::OpenOptions, os::fd::AsRawFd, path::Path};
unsafe extern "C" {
    fn mmap(addr: *mut c_void, len: usize, prot: i32, flags: i32, fd: i32, off: i64)
    -> *mut c_void;
    fn munmap(addr: *mut c_void, len: usize) -> i32;
}
pub struct FrameMap(*mut c_void, usize);
impl FrameMap {
    pub fn new(path: &Path, size: Dimensions) -> Res<Self> {
        let len = size.rgba_len();
        let f = OpenOptions::new()
            .read(true)
            .write(true)
            .create_new(true)
            .open(path)?;
        f.set_len(len as u64)?;
        let addr = unsafe { mmap(std::ptr::null_mut(), len, 1, 1, f.as_raw_fd(), 0) }; // PROT_READ, MAP_SHARED
        if addr as isize == -1 {
            return Err(std::io::Error::last_os_error().into());
        }
        Ok(Self(addr, len))
    }
    pub fn bytes(&self) -> &[u8] {
        unsafe { std::slice::from_raw_parts(self.0.cast(), self.1) }
    }
}
impl Drop for FrameMap {
    fn drop(&mut self) {
        unsafe {
            munmap(self.0, self.1);
        }
    }
}
