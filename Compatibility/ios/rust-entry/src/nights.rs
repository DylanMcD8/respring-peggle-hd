//! Restore the discontinued Nights entitlement before the guest reads preferences.
//! Content remains in the original IPA. Never copy archive session or profile data.
use std::{fs, io::{self, Write}, path::Path};
use plist::{Dictionary, Value};

pub fn initialize(documents: &Path, bundle: &str) -> Result<bool, String> {
    if !matches!(bundle, "com.popcap.Peggle" | "com.popcap.ios.PeggleHD") {
        return Err("Unsupported Nights bundle".into());
    }
    let run = || -> Result<bool, Box<dyn std::error::Error>> {
        let mut directory = documents.to_path_buf();
        for component in ["touchHLE_sandbox", bundle, "Library", "Preferences"] {
            directory.push(component);
            match fs::symlink_metadata(&directory) {
                Ok(m) if m.is_dir() && !m.file_type().is_symlink() => (),
                Ok(_) => return Err("Redirected Nights preferences directory".into()),
                Err(e) if e.kind() == io::ErrorKind::NotFound => fs::create_dir(&directory)?,
                Err(e) => return Err(e.into()),
            }
        }
        let path = directory.join(format!("{bundle}.plist"));
        let mut preferences = match fs::symlink_metadata(&path) {
            Ok(m) if m.is_file() && !m.file_type().is_symlink() && m.len() <= 1024 * 1024 =>
                Value::from_file(&path)?.into_dictionary().ok_or("Nights preferences must be a dictionary")?,
            Ok(_) => return Err("Invalid Nights preferences file".into()),
            Err(e) if e.kind() == io::ErrorKind::NotFound => Dictionary::new(),
            Err(e) => return Err(e.into()),
        };
        if preferences.get("store.nights.unlock") == Some(&Value::Boolean(true)) { return Ok(false); }
        preferences.insert("store.nights.unlock".into(), Value::Boolean(true));
        let mut bytes = Vec::new();
        Value::Dictionary(preferences).to_writer_xml(&mut bytes)?;
        let temporary = directory.join(format!(".nights-{}.plist", uuid::Uuid::new_v4()));
        let write = || -> io::Result<()> {
            let mut file = fs::OpenOptions::new().write(true).create_new(true).open(&temporary)?;
            file.write_all(&bytes)?;
            file.sync_all()?;
            fs::rename(&temporary, &path)
        };
        let result = write();
        if result.is_err() { let _ = fs::remove_file(&temporary); }
        result?;
        Ok(true)
    };
    run().map_err(|e| format!("Nights preferences: {e}"))
}

#[cfg(test)]
mod tests {
    use super::*;
    fn fixture() -> std::path::PathBuf {
        let root=std::env::temp_dir().join(format!("peggle-nights-{}",uuid::Uuid::new_v4()));
        fs::create_dir(&root).unwrap(); root
    }
    #[test]
    fn fresh_install_both_games_has_only_entitlement_and_no_synthetic_profile() {
        let root=fixture();
        for bundle in ["com.popcap.Peggle","com.popcap.ios.PeggleHD"] {
            assert!(initialize(&root,bundle).unwrap());
            let guest=root.join("touchHLE_sandbox").join(bundle);
            let value=Value::from_file(guest.join(format!("Library/Preferences/{bundle}.plist"))).unwrap();
            assert_eq!(value.as_dictionary().unwrap().len(),1);
            assert_eq!(value.as_dictionary().unwrap()["store.nights.unlock"],Value::Boolean(true));
            assert!(!guest.join("Documents").exists());
            assert!(!initialize(&root,bundle).unwrap());
        }
        fs::remove_dir_all(root).unwrap();
    }
    #[test]
    fn binary_preferences_and_save_bytes_survive_then_relaunch_is_byte_identical() {
        let root=fixture(); let bundle="com.popcap.Peggle"; initialize(&root,bundle).unwrap();
        let guest=root.join("touchHLE_sandbox").join(bundle);
        let path=guest.join(format!("Library/Preferences/{bundle}.plist"));
        let mut values=Dictionary::new();
        values.insert("CurUser".into(),Value::String("duo".into()));
        values.insert("volume".into(),Value::Real(0.42));
        values.insert("store.nights.unlock".into(),Value::Boolean(false));
        Value::Dictionary(values.clone()).to_file_binary(&path).unwrap();
        fs::create_dir_all(guest.join("Documents/userdata")).unwrap();
        let save=guest.join("Documents/userdata/stage1.sav"); fs::write(&save,b"existing progress").unwrap();
        assert!(initialize(&root,bundle).unwrap());
        values.insert("store.nights.unlock".into(),Value::Boolean(true));
        assert_eq!(Value::from_file(&path).unwrap(),Value::Dictionary(values));
        let bytes=fs::read(&path).unwrap(); assert!(!initialize(&root,bundle).unwrap());
        assert_eq!(fs::read(&path).unwrap(),bytes); assert_eq!(fs::read(save).unwrap(),b"existing progress");
        fs::remove_dir_all(root).unwrap();
    }
    #[test]
    fn restored_slot_receives_entitlement_without_touching_other_slots() {
        let root=fixture(); let slot=root.join("restored-slot"); fs::create_dir(&slot).unwrap();
        initialize(&slot,"com.popcap.Peggle").unwrap();
        assert!(!root.join("touchHLE_sandbox").exists()); fs::remove_dir_all(root).unwrap();
    }
    #[test]
    fn malformed_preferences_are_preserved() {
        let root=fixture(); let bundle="com.popcap.Peggle"; initialize(&root,bundle).unwrap();
        let path=root.join(format!("touchHLE_sandbox/{bundle}/Library/Preferences/{bundle}.plist"));
        fs::write(&path,b"broken preferences").unwrap();
        assert!(initialize(&root,bundle).is_err()); assert_eq!(fs::read(path).unwrap(),b"broken preferences");
        fs::remove_dir_all(root).unwrap();
    }
    #[cfg(unix)]
    #[test]
    fn redirected_directory_is_rejected() {
        let root=fixture(); let outside=fixture();
        std::os::unix::fs::symlink(&outside,root.join("touchHLE_sandbox")).unwrap();
        assert!(initialize(&root,"com.popcap.Peggle").is_err());
        assert_eq!(fs::read_dir(&outside).unwrap().count(),0);
        fs::remove_dir_all(root).unwrap(); fs::remove_dir_all(outside).unwrap();
    }
}
