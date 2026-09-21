use anyhow::{Context, Result};
use base64::Engine;

const PREFIX: &str = "wv1:";

/// Encrypt a credential field for SQLite. Legacy plaintext values remain readable.
pub fn seal_text(plain: &str) -> Result<String> {
    let protected = protect(plain.as_bytes())?;
    Ok(format!(
        "{}{}",
        PREFIX,
        base64::engine::general_purpose::STANDARD.encode(protected)
    ))
}

pub fn open_text(stored: &str) -> Result<String> {
    let Some(rest) = stored.strip_prefix(PREFIX) else {
        return Ok(stored.to_string());
    };
    let raw = base64::engine::general_purpose::STANDARD
        .decode(rest)
        .context("Invalid sealed credential")?;
    let plain = unprotect(&raw)?;
    String::from_utf8(plain).context("Sealed credential is not UTF-8")
}

#[cfg(windows)]
fn protect(plain: &[u8]) -> Result<Vec<u8>> {
    dpapi_protect(plain, true)
}

#[cfg(windows)]
fn unprotect(blob: &[u8]) -> Result<Vec<u8>> {
    dpapi_protect(blob, false)
}

#[cfg(windows)]
fn dpapi_protect(input: &[u8], encrypt: bool) -> Result<Vec<u8>> {
    use std::ptr;

    #[repr(C)]
    struct DataBlob {
        cb_data: u32,
        pb_data: *mut u8,
    }

    #[link(name = "crypt32")]
    extern "system" {
        fn CryptProtectData(
            data_in: *const DataBlob,
            descr: *const u16,
            optional_entropy: *const DataBlob,
            reserved: *mut std::ffi::c_void,
            prompt: *mut std::ffi::c_void,
            flags: u32,
            data_out: *mut DataBlob,
        ) -> i32;
        fn CryptUnprotectData(
            data_in: *const DataBlob,
            descr: *mut *mut u16,
            optional_entropy: *const DataBlob,
            reserved: *mut std::ffi::c_void,
            prompt: *mut std::ffi::c_void,
            flags: u32,
            data_out: *mut DataBlob,
        ) -> i32;
    }

    #[link(name = "kernel32")]
    extern "system" {
        fn LocalFree(h: *mut std::ffi::c_void) -> *mut std::ffi::c_void;
    }

    const CRYPTPROTECT_UI_FORBIDDEN: u32 = 0x1;
    let mut input_vec = input.to_vec();
    let blob_in = DataBlob {
        cb_data: input_vec.len() as u32,
        pb_data: input_vec.as_mut_ptr(),
    };
    let mut blob_out = DataBlob {
        cb_data: 0,
        pb_data: ptr::null_mut(),
    };
    let ok = unsafe {
        if encrypt {
            CryptProtectData(
                &blob_in,
                ptr::null(),
                ptr::null(),
                ptr::null_mut(),
                ptr::null_mut(),
                CRYPTPROTECT_UI_FORBIDDEN,
                &mut blob_out,
            )
        } else {
            CryptUnprotectData(
                &blob_in,
                ptr::null_mut(),
                ptr::null(),
                ptr::null_mut(),
                ptr::null_mut(),
                CRYPTPROTECT_UI_FORBIDDEN,
                &mut blob_out,
            )
        }
    };
    if ok == 0 {
        anyhow::bail!("Windows DPAPI operation failed");
    }
    let out = unsafe {
        std::slice::from_raw_parts(blob_out.pb_data, blob_out.cb_data as usize).to_vec()
    };
    unsafe {
        LocalFree(blob_out.pb_data as *mut std::ffi::c_void);
    }
    Ok(out)
}

#[cfg(not(windows))]
fn protect(plain: &[u8]) -> Result<Vec<u8>> {
    Ok(plain.to_vec())
}

#[cfg(not(windows))]
fn unprotect(blob: &[u8]) -> Result<Vec<u8>> {
    Ok(blob.to_vec())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_seal_roundtrip_and_legacy_plaintext() {
        let sealed = seal_text(r#"[{"name":"sid","value":"abc"}]"#).unwrap();
        assert!(sealed.starts_with("wv1:"));
        let opened = open_text(&sealed).unwrap();
        assert_eq!(opened, r#"[{"name":"sid","value":"abc"}]"#);
        assert_eq!(open_text("[]").unwrap(), "[]");
    }
}
