//! Optional administrator action only; never called by the plugin lifecycle.
use std::{
    fs::OpenOptions,
    io::{self, Write},
    os::unix::fs::OpenOptionsExt,
    path::Path,
};
const RULE: &[u8] = include_bytes!("../packaging/70-elgato-foot-pedal.rules");
pub fn create(path: &Path) -> io::Result<()> {
    let mut file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .mode(0o644)
        .open(path)?;
    file.write_all(RULE)?;
    file.sync_all()
}
pub fn install() -> crate::config::Result<()> {
    if unsafe { libc::geteuid() } != 0 {
        return Err(
            "This optional USB rule requires administrator access. Follow the README.".into(),
        );
    }
    create(Path::new("/etc/udev/rules.d/70-elgato-foot-pedal.rules")).map_err(|e|format!("USB rule was not installed: {e}. Existing paths are preserved; inspect ownership before making any changes."))?;
    println!(
        "Created the pedal-specific USB rule. Reload udev rules and reconnect the pedal as documented."
    );
    Ok(())
}
#[cfg(test)]
mod tests {
    use super::*;
    use std::{
        fs,
        os::unix::fs::{MetadataExt, symlink},
    };
    #[test]
    fn exclusive_creation_refuses_all_existing_paths() {
        let temp = tempfile::tempdir().unwrap();
        let path = temp.path().join("rule");
        create(&path).unwrap();
        let before = path.metadata().unwrap();
        assert!(create(&path).is_err());
        assert_eq!(path.metadata().unwrap().mtime_nsec(), before.mtime_nsec());
        assert_eq!(fs::read(&path).unwrap(), RULE);
        fs::write(&path, b"user rule").unwrap();
        assert!(create(&path).is_err());
        assert_eq!(fs::read(&path).unwrap(), b"user rule");
        fs::remove_file(&path).unwrap();
        let other = temp.path().join("other");
        symlink(&other, &path).unwrap();
        assert!(create(&path).is_err());
        assert!(!other.exists());
        fs::write(&other, b"package rule").unwrap();
        assert!(create(&path).is_err());
        assert_eq!(fs::read(&other).unwrap(), b"package rule");
        fs::remove_file(&path).unwrap();
        fs::create_dir(&path).unwrap();
        assert!(create(&path).is_err());
        fs::remove_dir(&path).unwrap();
        let p = std::ffi::CString::new(path.to_str().unwrap()).unwrap();
        assert_eq!(unsafe { libc::mkfifo(p.as_ptr(), 0o600) }, 0);
        assert!(create(&path).is_err());
    }
}
