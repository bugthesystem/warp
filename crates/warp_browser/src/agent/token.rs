use std::fs;
use std::io::{self, Write as _};
use std::path::Path;

/// Reads the bearer token stored at `path`, creating the file with a new token when it is missing
/// or empty. The file is readable only by its owner, since the token grants control of the user's
/// browser panes.
pub fn load_or_create_token(path: &Path) -> io::Result<String> {
    match fs::read_to_string(path) {
        Ok(token) if !token.trim().is_empty() => return Ok(token.trim().to_owned()),
        Ok(_) => {}
        Err(err) if err.kind() == io::ErrorKind::NotFound => {}
        Err(err) => return Err(err),
    }
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    let token = new_token();
    write_owner_only(path, &token)?;
    Ok(token)
}

pub fn new_token() -> String {
    uuid::Uuid::new_v4().simple().to_string()
}

#[cfg(unix)]
fn write_owner_only(path: &Path, contents: &str) -> io::Result<()> {
    use std::os::unix::fs::{OpenOptionsExt as _, PermissionsExt as _};

    let mut file = fs::OpenOptions::new()
        .write(true)
        .create(true)
        .truncate(true)
        .mode(0o600)
        .open(path)?;
    // `mode` only applies when the file is created, so also fix up an existing empty file.
    file.set_permissions(fs::Permissions::from_mode(0o600))?;
    file.write_all(contents.as_bytes())
}

#[cfg(not(unix))]
fn write_owner_only(path: &Path, contents: &str) -> io::Result<()> {
    fs::write(path, contents)
}

#[cfg(test)]
#[path = "token_tests.rs"]
mod tests;
