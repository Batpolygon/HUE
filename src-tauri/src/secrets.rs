//! API key storage in the Windows Credential Manager (never in settings.json).

#[cfg(windows)]
const TARGET: &str = "HUE/api_key";

#[cfg(windows)]
fn wide(s: &str) -> Vec<u16> {
    s.encode_utf16().chain(std::iter::once(0)).collect()
}

/// Store (or delete, when empty) the API key.
pub fn store_api_key(key: &str) -> Result<(), String> {
    #[cfg(windows)]
    {
        use windows::core::{PCWSTR, PWSTR};
        use windows::Win32::Security::Credentials::{
            CredDeleteW, CredWriteW, CREDENTIALW, CRED_PERSIST_LOCAL_MACHINE, CRED_TYPE_GENERIC,
        };
        let mut target = wide(TARGET);
        if key.trim().is_empty() {
            unsafe {
                let _ = CredDeleteW(PCWSTR(target.as_ptr()), CRED_TYPE_GENERIC, 0);
            }
            return Ok(());
        }
        let mut user = wide("hue");
        let blob = key.trim().as_bytes().to_vec();
        let cred = CREDENTIALW {
            Type: CRED_TYPE_GENERIC,
            TargetName: PWSTR(target.as_mut_ptr()),
            CredentialBlobSize: blob.len() as u32,
            CredentialBlob: blob.as_ptr() as *mut u8,
            Persist: CRED_PERSIST_LOCAL_MACHINE,
            UserName: PWSTR(user.as_mut_ptr()),
            ..Default::default()
        };
        unsafe { CredWriteW(&cred, 0) }.map_err(|e| e.to_string())
    }
    #[cfg(not(windows))]
    {
        let _ = key;
        Err("pas de coffre sécurisé sur cet OS".into())
    }
}

pub fn load_api_key() -> Option<String> {
    #[cfg(windows)]
    {
        use windows::core::PCWSTR;
        use windows::Win32::Security::Credentials::{
            CredFree, CredReadW, CREDENTIALW, CRED_TYPE_GENERIC,
        };
        let target = wide(TARGET);
        let mut pcred: *mut CREDENTIALW = std::ptr::null_mut();
        unsafe {
            if CredReadW(PCWSTR(target.as_ptr()), CRED_TYPE_GENERIC, 0, &mut pcred).is_err()
                || pcred.is_null()
            {
                return None;
            }
            let c = &*pcred;
            let bytes =
                std::slice::from_raw_parts(c.CredentialBlob, c.CredentialBlobSize as usize).to_vec();
            CredFree(pcred as *const core::ffi::c_void);
            String::from_utf8(bytes).ok()
        }
    }
    #[cfg(not(windows))]
    {
        None
    }
}
