const TARGET_PREFIX: &str = "AgentBar.CalendarConnection.";
const MAX_SECRET_BYTES: usize = 2_400;

fn target_name(id: &str) -> Result<String, String> {
    if id.is_empty()
        || id.len() > 96
        || !id
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || byte == b'-' || byte == b'_')
    {
        return Err("invalid calendar connection identifier".to_string());
    }
    Ok(format!("{TARGET_PREFIX}{id}"))
}

#[cfg(windows)]
pub fn write(id: &str, secret: &str) -> Result<(), String> {
    use windows::core::PWSTR;
    use windows::Win32::Security::Credentials::{
        CredWriteW, CREDENTIALW, CRED_PERSIST_LOCAL_MACHINE, CRED_TYPE_GENERIC,
    };

    if secret.is_empty() || secret.len() > MAX_SECRET_BYTES {
        return Err(format!(
            "calendar connection secret must contain 1 to {MAX_SECRET_BYTES} bytes"
        ));
    }

    let mut target = target_name(id)?
        .encode_utf16()
        .chain(Some(0))
        .collect::<Vec<_>>();
    let mut username = "Agent Bar"
        .encode_utf16()
        .chain(Some(0))
        .collect::<Vec<_>>();
    let mut blob = secret.as_bytes().to_vec();
    let credential = CREDENTIALW {
        Type: CRED_TYPE_GENERIC,
        TargetName: PWSTR(target.as_mut_ptr()),
        CredentialBlobSize: blob.len() as u32,
        CredentialBlob: blob.as_mut_ptr(),
        Persist: CRED_PERSIST_LOCAL_MACHINE,
        UserName: PWSTR(username.as_mut_ptr()),
        ..Default::default()
    };

    let result = unsafe { CredWriteW(&credential, 0) }
        .map_err(|error| format!("could not store calendar credential: {error}"));
    blob.fill(0);
    result
}

#[cfg(windows)]
pub fn read(id: &str) -> Result<String, String> {
    use std::ptr::null_mut;
    use windows::core::PCWSTR;
    use windows::Win32::Security::Credentials::{
        CredFree, CredReadW, CREDENTIALW, CRED_TYPE_GENERIC,
    };

    let target = target_name(id)?
        .encode_utf16()
        .chain(Some(0))
        .collect::<Vec<_>>();
    let mut credential: *mut CREDENTIALW = null_mut();
    unsafe {
        CredReadW(
            PCWSTR(target.as_ptr()),
            CRED_TYPE_GENERIC,
            None,
            &mut credential,
        )
        .map_err(|error| format!("could not read calendar credential: {error}"))?;
        if credential.is_null() {
            return Err("calendar credential was empty".to_string());
        }
        let stored = &*credential;
        let bytes =
            std::slice::from_raw_parts(stored.CredentialBlob, stored.CredentialBlobSize as usize)
                .to_vec();
        CredFree(credential.cast());
        String::from_utf8(bytes)
            .map_err(|_| "calendar credential contains invalid UTF-8".to_string())
    }
}

#[cfg(windows)]
pub fn delete(id: &str) -> Result<(), String> {
    use windows::core::{HRESULT, PCWSTR};
    use windows::Win32::Foundation::ERROR_NOT_FOUND;
    use windows::Win32::Security::Credentials::{CredDeleteW, CRED_TYPE_GENERIC};

    let target = target_name(id)?
        .encode_utf16()
        .chain(Some(0))
        .collect::<Vec<_>>();
    match unsafe { CredDeleteW(PCWSTR(target.as_ptr()), CRED_TYPE_GENERIC, None) } {
        Ok(()) => Ok(()),
        Err(error) if error.code() == HRESULT::from_win32(ERROR_NOT_FOUND.0) => Ok(()),
        Err(error) => Err(format!("could not delete calendar credential: {error}")),
    }
}

#[cfg(not(windows))]
pub fn write(_id: &str, _secret: &str) -> Result<(), String> {
    Err("secure calendar credentials are currently available on Windows only".to_string())
}

#[cfg(not(windows))]
pub fn read(_id: &str) -> Result<String, String> {
    Err("secure calendar credentials are currently available on Windows only".to_string())
}

#[cfg(not(windows))]
pub fn delete(_id: &str) -> Result<(), String> {
    Err("secure calendar credentials are currently available on Windows only".to_string())
}

#[cfg(test)]
mod tests {
    use super::target_name;
    #[cfg(windows)]
    use super::{delete, read, write};

    #[test]
    fn credential_target_rejects_untrusted_identifiers() {
        assert!(target_name("cal-safe_123").is_ok());
        assert!(target_name("../calendar").is_err());
        assert!(target_name("").is_err());
    }

    #[cfg(windows)]
    #[test]
    #[ignore = "writes a short-lived entry to the current Windows Credential Manager"]
    fn windows_credential_round_trip() {
        let id = format!("test-{}", std::process::id());
        write(&id, "https://calendar.example/private-token").unwrap();
        assert_eq!(read(&id).unwrap(), "https://calendar.example/private-token");
        delete(&id).unwrap();
    }
}
