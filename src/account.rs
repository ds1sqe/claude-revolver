use std::fs;
use std::os::unix::fs::PermissionsExt;

use anyhow::{Context, Result};

use crate::error::RevolverError;
use crate::paths;
use crate::types::Credentials;
use crate::util::{atomic_write, ensure_dir, validate_name};

/// List all stored account names.
pub fn list_accounts() -> Result<Vec<String>> {
    let data = paths::data_dir()?;
    if !data.exists() {
        return Ok(Vec::new());
    }

    let mut accounts = Vec::new();
    for entry in fs::read_dir(&data)? {
        let entry = entry?;
        if entry.file_type()?.is_dir() {
            let name = entry.file_name().to_string_lossy().to_string();
            if paths::account_cred_file(&name)?.exists() {
                accounts.push(name);
            }
        }
    }
    accounts.sort();
    Ok(accounts)
}

/// Get the active account name, if any.
pub fn get_active() -> Result<Option<String>> {
    let path = paths::active_file()?;
    if !path.exists() {
        return Ok(None);
    }
    let name = fs::read_to_string(&path)?.trim().to_string();
    if name.is_empty() {
        Ok(None)
    } else {
        Ok(Some(name))
    }
}

/// Set the active account name.
pub fn set_active(name: &str) -> Result<()> {
    let path = paths::active_file()?;
    atomic_write(&path, name.as_bytes(), 0o644)
}

/// Read credentials from an account's store.
pub fn read_credentials(name: &str) -> Result<Credentials> {
    let path = paths::account_cred_file(name)?;
    if !path.exists() {
        return Err(RevolverError::AccountNotFound(name.to_string()).into());
    }
    let content = fs::read_to_string(&path)
        .with_context(|| format!("cannot read credentials for '{name}'"))?;
    Ok(serde_json::from_str(&content)?)
}

/// Read live credentials from ~/.claude/.credentials.json.
pub fn read_live_credentials() -> Result<Credentials> {
    let path = paths::cred_file()?;
    if !path.exists() {
        return Err(RevolverError::NoCredentials(path.display().to_string()).into());
    }
    let content = fs::read_to_string(&path)?;
    Ok(serde_json::from_str(&content)?)
}

/// Save credentials to an account's store.
pub fn save_credentials(name: &str, creds: &Credentials) -> Result<()> {
    let dir = paths::account_dir(name)?;
    ensure_dir(&dir, 0o700)?;
    let path = paths::account_cred_file(name)?;
    atomic_write(
        &path,
        serde_json::to_string_pretty(creds)?.as_bytes(),
        0o600,
    )
}

/// Save live credentials to the active account's store (if live file is newer).
pub fn sync_back() -> Result<bool> {
    let active = match get_active()? {
        Some(a) => a,
        None => return Ok(false),
    };

    let live_path = paths::cred_file()?;
    let stored_path = paths::account_cred_file(&active)?;

    if !live_path.exists() || !stored_path.parent().map_or(false, |p| p.exists()) {
        return Ok(false);
    }

    let live_modified = fs::metadata(&live_path)?.modified()?;
    let stored_modified = if stored_path.exists() {
        fs::metadata(&stored_path)?.modified()?
    } else {
        std::time::SystemTime::UNIX_EPOCH
    };

    if live_modified > stored_modified {
        fs::copy(&live_path, &stored_path)?;
        fs::set_permissions(&stored_path, fs::Permissions::from_mode(0o600))?;
        return Ok(true);
    }

    Ok(false)
}

/// Swap credentials: save outgoing, load incoming, update active.
pub fn swap_credentials(from: &str, to: &str) -> Result<()> {
    let live_path = paths::cred_file()?;

    // Save outgoing (if live creds exist and from account dir exists)
    if live_path.exists() && paths::account_dir(from)?.exists() {
        let live_creds = read_live_credentials()?;
        save_credentials(from, &live_creds)?;
    }

    // Load incoming
    let incoming = read_credentials(to)?;
    atomic_write(
        &live_path,
        serde_json::to_string_pretty(&incoming)?.as_bytes(),
        0o600,
    )?;

    set_active(to)?;
    Ok(())
}

/// Add a new account by copying current live credentials.
pub fn add_account(name: &str) -> Result<()> {
    if !validate_name(name) {
        return Err(RevolverError::InvalidName(name.to_string()).into());
    }

    ensure_dir(&paths::data_dir()?, 0o700)?;

    if paths::account_cred_file(name)?.exists() {
        return Err(RevolverError::AccountExists(name.to_string()).into());
    }

    let creds = read_live_credentials()?;
    save_credentials(name, &creds)?;

    // Set as active if first account
    if get_active()?.is_none() {
        set_active(name)?;
    }

    Ok(())
}

/// Remove an account from the store.
pub fn remove_account(name: &str) -> Result<bool> {
    let dir = paths::account_dir(name)?;
    if !dir.exists() {
        return Err(RevolverError::AccountNotFound(name.to_string()).into());
    }

    let was_active = get_active()?.as_deref() == Some(name);
    if was_active {
        atomic_write(&paths::active_file()?, b"", 0o644)?;
    }

    fs::remove_dir_all(&dir)?;
    Ok(was_active)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_validate_name() {
        assert!(validate_name("personal"));
        assert!(validate_name("work-account"));
        assert!(validate_name("acc_123"));
        assert!(!validate_name(""));
        assert!(!validate_name("has space"));
        assert!(!validate_name("has/slash"));
        assert!(!validate_name("has.dot"));
    }
}
