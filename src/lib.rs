#![allow(non_upper_case_globals)]
#![allow(non_camel_case_types)]
#![allow(non_snake_case)]
#![allow(improper_ctypes)]

// Needed so that the Rust compiler links the code in this crate with libz's
extern crate libz_sys;

include!(concat!(env!("OUT_DIR"), "/bindings.rs"));

pub mod built_info {
    pub const TARGET: &str = include_str!(concat!(env!("OUT_DIR"), "/target"));
}

impl From<aiString> for String {
    fn from(string: aiString) -> Self {
        unsafe {
            std::str::from_utf8(std::slice::from_raw_parts(
                string.data.as_ptr() as *const u8,
                string.length as _,
            ))
        }
        .unwrap()
        .into()
    }
}

impl From<&aiString> for String {
    fn from(string: &aiString) -> Self {
        unsafe {
            std::str::from_utf8(std::slice::from_raw_parts(
                string.data.as_ptr() as *const u8,
                string.length as _,
            ))
        }
        .unwrap()
        .into()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A simple test to make sure assimp library linked properly.
    #[test]
    fn test_version() {
        let _ = unsafe { aiGetVersionMajor() };
    }
}
