//! The chat API key, encrypted with Windows DPAPI (current user, this machine) and kept in
//! `%APPDATA%\MyNoteTaker\chat_api_key.bin`, never in config.toml.

use std::path::PathBuf;

use anyhow::Context;
use windows_sys::Win32::Foundation::LocalFree;
use windows_sys::Win32::Security::Cryptography::{
    CryptProtectData, CryptUnprotectData, CRYPTPROTECT_UI_FORBIDDEN, CRYPT_INTEGER_BLOB,
};

/// Fixed app-specific entropy, so another program's DPAPI blobs aren't accepted here.
const ENTROPY: &[u8] = b"MyNoteTaker.chat_api_key";

fn blob(data: &[u8]) -> CRYPT_INTEGER_BLOB {
    CRYPT_INTEGER_BLOB { cbData: data.len() as u32, pbData: data.as_ptr() as *mut u8 }
}

/// Copy a DPAPI output buffer and free it.
///
/// # Safety
/// `out` must be an output blob filled in by a successful DPAPI call.
unsafe fn take(out: CRYPT_INTEGER_BLOB) -> Vec<u8> {
    let bytes = std::slice::from_raw_parts(out.pbData, out.cbData as usize).to_vec();
    LocalFree(out.pbData as _);
    bytes
}

pub fn protect(plain: &str) -> anyhow::Result<Vec<u8>> {
    let input = blob(plain.as_bytes());
    let entropy = blob(ENTROPY);
    let mut out = CRYPT_INTEGER_BLOB { cbData: 0, pbData: std::ptr::null_mut() };
    // SAFETY: the input blobs outlive the call; `out` is freed by `take` on success.
    unsafe {
        let ok = CryptProtectData(
            &input,
            std::ptr::null(),
            &entropy,
            std::ptr::null(),
            std::ptr::null(),
            CRYPTPROTECT_UI_FORBIDDEN,
            &mut out,
        );
        anyhow::ensure!(ok != 0, "CryptProtectData failed: {}", std::io::Error::last_os_error());
        Ok(take(out))
    }
}

pub fn unprotect(cipher: &[u8]) -> anyhow::Result<String> {
    let input = blob(cipher);
    let entropy = blob(ENTROPY);
    let mut out = CRYPT_INTEGER_BLOB { cbData: 0, pbData: std::ptr::null_mut() };
    // SAFETY: as in `protect`.
    let bytes = unsafe {
        let ok = CryptUnprotectData(
            &input,
            std::ptr::null_mut(),
            &entropy,
            std::ptr::null(),
            std::ptr::null(),
            CRYPTPROTECT_UI_FORBIDDEN,
            &mut out,
        );
        anyhow::ensure!(ok != 0, "CryptUnprotectData failed: {}", std::io::Error::last_os_error());
        take(out)
    };
    String::from_utf8(bytes).context("decrypted key is not text")
}

fn key_path() -> PathBuf {
    crate::config::app_dir().join("chat_api_key.bin")
}

pub fn has_api_key() -> bool {
    key_path().exists()
}

/// Encrypt and store the key, replacing any previous one.
pub fn save_api_key(plain: &str) -> anyhow::Result<()> {
    std::fs::create_dir_all(crate::config::app_dir()).context("creating config dir")?;
    std::fs::write(key_path(), protect(plain)?).context("writing chat_api_key.bin")
}

/// The stored key, if any. Errors when the file exists but can't be decrypted (copied from
/// another machine or user): the key has to be entered again.
pub fn load_api_key() -> anyhow::Result<Option<String>> {
    let bytes = match std::fs::read(key_path()) {
        Ok(b) => b,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(e) => return Err(e).context("reading chat_api_key.bin"),
    };
    unprotect(&bytes)
        .map(Some)
        .context("the saved API key can't be decrypted (it was saved by another user or machine); enter the key again")
}

pub fn clear_api_key() -> anyhow::Result<()> {
    match std::fs::remove_file(key_path()) {
        Err(e) if e.kind() != std::io::ErrorKind::NotFound => Err(e).context("removing chat_api_key.bin"),
        _ => Ok(()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn roundtrip() {
        let cipher = protect("sk-test-123").unwrap();
        assert!(!cipher.windows(11).any(|w| w == b"sk-test-123"));
        assert_eq!(unprotect(&cipher).unwrap(), "sk-test-123");
    }

    #[test]
    fn garbage_errors() {
        assert!(unprotect(b"definitely not a dpapi blob").is_err());
    }
}
