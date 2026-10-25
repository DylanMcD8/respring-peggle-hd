//! First-launch profile initialization in a separate host app container.
//! The original guest IPA and its gameplay data are never changed.
use std::fs::{self, OpenOptions};
use std::io::{self, Write};
use std::path::Path;

// Statically verified Peggle HD 2.0.9/4840.14.07.16.01 ProfileMgr::SyncState, PlayerInfo::SyncSummary:
// u32 version=1, u16 count=1, u16 name length=4, bytes "User", two u32 IDs=1.
pub const USER_INDEX: &[u8] = b"\x01\x00\x00\x00\x01\x00\x04\x00User\x01\x00\x00\x00\x01\x00\x00\x00";
pub const USER_PREFERENCES: &[u8] = br#"<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0"><dict><key>CurUser</key><string>User</string></dict></plist>
"#;
// The original store.plist maps peggle.nights.unlock to this sole preference.
// Uploader restore/session keys and promotional files are deliberately omitted.
pub const USER_NIGHTS_PREFERENCES: &[u8] = br#"<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0"><dict><key>CurUser</key><string>User</string><key>store.nights.unlock</key><true/></dict></plist>
"#;

#[derive(Debug, PartialEq)]
pub enum Outcome { Disabled, ExistingGuestPreserved, CreatedCleanUser, CreatedCleanUserNights }

pub fn initialize(resources: &Path, documents: &Path) -> io::Result<Outcome> {
    let user = resources.join("CleanUserSeed.enabled").is_file();
    let nights = resources.join("CleanNightsSeed.enabled").is_file();
    if nights && !user {
        return Err(io::Error::new(io::ErrorKind::InvalidInput, "Nights requires the separate clean User variant"));
    }
    if !user { return Ok(Outcome::Disabled); }
    let parent = documents.join("touchHLE_sandbox");
    match fs::symlink_metadata(&parent) {
        Ok(meta) if meta.file_type().is_symlink() || !meta.is_dir() =>
            return Err(io::Error::new(io::ErrorKind::InvalidInput, "Refusing a redirected sandbox parent")),
        Ok(_) => (),
        Err(e) if e.kind() == io::ErrorKind::NotFound => fs::create_dir(&parent)?,
        Err(e) => return Err(e),
    }
    let guest = parent.join("com.popcap.ios.PeggleHD");
    // An existing guest, even an incomplete or empty one, is preserved whole.
    // create_dir is exclusive: no existing save/preferences can be replaced.
    match fs::create_dir(&guest) {
        Ok(()) => (),
        Err(e) if e.kind() == io::ErrorKind::AlreadyExists => return Ok(Outcome::ExistingGuestPreserved),
        Err(e) => return Err(e),
    }
    fs::create_dir_all(guest.join("Documents/userdata"))?;
    fs::create_dir_all(guest.join("Library/Preferences"))?;
    fs::create_dir(guest.join("tmp"))?;
    for (relative, bytes) in [
        ("Documents/userdata/users.dat", USER_INDEX),
        ("Library/Preferences/com.popcap.ios.PeggleHD.plist", if nights { USER_NIGHTS_PREFERENCES } else { USER_PREFERENCES }),
    ] {
        OpenOptions::new().write(true).create_new(true).open(guest.join(relative))?.write_all(bytes)?;
    }
    Ok(if nights { Outcome::CreatedCleanUserNights } else { Outcome::CreatedCleanUser })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicUsize, Ordering};
    static NEXT: AtomicUsize = AtomicUsize::new(0);
    fn fixture() -> (std::path::PathBuf, std::path::PathBuf, std::path::PathBuf) {
        let root = std::env::temp_dir().join(format!("peggle-clean-user-test-{}-{}", std::process::id(), NEXT.fetch_add(1, Ordering::SeqCst)));
        let resources = root.join("resources"); let documents = root.join("Documents");
        fs::create_dir_all(&resources).unwrap(); fs::create_dir(&documents).unwrap();
        (root, resources, documents)
    }
    #[test]
    fn disabled_candidate_does_not_create_guest_state() {
        let (root, resources, documents) = fixture();
        assert_eq!(initialize(&resources, &documents).unwrap(), Outcome::Disabled);
        assert!(!documents.join("touchHLE_sandbox").exists()); fs::remove_dir_all(root).unwrap();
    }
    #[test]
    fn clean_profile_contains_only_name_index_and_current_name() {
        let (root, resources, documents) = fixture(); fs::write(resources.join("CleanUserSeed.enabled"), b"User").unwrap();
        assert_eq!(initialize(&resources, &documents).unwrap(), Outcome::CreatedCleanUser);
        let guest = documents.join("touchHLE_sandbox/com.popcap.ios.PeggleHD");
        assert_eq!(fs::read(guest.join("Documents/userdata/users.dat")).unwrap(), USER_INDEX);
        assert_eq!(fs::read_dir(guest.join("Documents/userdata")).unwrap().count(), 1);
        assert!(!guest.join("Documents/userdata/user1.dat").exists());
        assert_eq!(fs::read(guest.join("Library/Preferences/com.popcap.ios.PeggleHD.plist")).unwrap(), USER_PREFERENCES);
        fs::remove_dir_all(root).unwrap();
    }
    #[test]
    fn existing_profile_progress_and_settings_are_byte_preserved() {
        let (root, resources, documents) = fixture(); fs::write(resources.join("CleanUserSeed.enabled"), b"User").unwrap();
        let guest = documents.join("touchHLE_sandbox/com.popcap.ios.PeggleHD");
        fs::create_dir_all(guest.join("Documents/userdata")).unwrap(); fs::create_dir_all(guest.join("Library/Preferences")).unwrap();
        for path in ["Documents/userdata/users.dat", "Documents/userdata/user1.dat", "Documents/userdata/stage1.sav", "Library/Preferences/com.popcap.ios.PeggleHD.plist"] {
            fs::write(guest.join(path), format!("existing:{path}")).unwrap();
        }
        assert_eq!(initialize(&resources, &documents).unwrap(), Outcome::ExistingGuestPreserved);
        for path in ["Documents/userdata/users.dat", "Documents/userdata/user1.dat", "Documents/userdata/stage1.sav", "Library/Preferences/com.popcap.ios.PeggleHD.plist"] {
            assert_eq!(fs::read(guest.join(path)).unwrap(), format!("existing:{path}").as_bytes());
        }
        fs::remove_dir_all(root).unwrap();
    }
    #[test]
    fn repeated_launch_does_not_reset_updated_progress() {
        let (root, resources, documents) = fixture(); fs::write(resources.join("CleanUserSeed.enabled"), b"User").unwrap();
        initialize(&resources, &documents).unwrap();
        let progress = documents.join("touchHLE_sandbox/com.popcap.ios.PeggleHD/Documents/userdata/user1.dat");
        fs::write(&progress, b"progress from guest").unwrap();
        assert_eq!(initialize(&resources, &documents).unwrap(), Outcome::ExistingGuestPreserved);
        assert_eq!(fs::read(&progress).unwrap(), b"progress from guest"); fs::remove_dir_all(root).unwrap();
    }
    #[test]
    fn partial_existing_guest_is_preserved_without_inventing_profile() {
        let (root, resources, documents) = fixture(); fs::write(resources.join("CleanUserSeed.enabled"), b"User").unwrap();
        fs::create_dir_all(documents.join("touchHLE_sandbox/com.popcap.ios.PeggleHD")).unwrap();
        assert_eq!(initialize(&resources, &documents).unwrap(), Outcome::ExistingGuestPreserved);
        assert!(!documents.join("touchHLE_sandbox/com.popcap.ios.PeggleHD/Documents/userdata/users.dat").exists()); fs::remove_dir_all(root).unwrap();
    }
    #[cfg(unix)]
    #[test]
    fn redirected_parent_cannot_write_outside_host_documents() {
        let (root, resources, documents) = fixture(); fs::write(resources.join("CleanUserSeed.enabled"), b"User").unwrap();
        let unrelated = root.join("unrelated"); fs::create_dir(&unrelated).unwrap();
        std::os::unix::fs::symlink(&unrelated, documents.join("touchHLE_sandbox")).unwrap();
        assert_eq!(initialize(&resources, &documents).unwrap_err().kind(), io::ErrorKind::InvalidInput);
        assert_eq!(fs::read_dir(unrelated).unwrap().count(), 0); fs::remove_dir_all(root).unwrap();
    }
    #[test]
    fn nights_marker_without_user_marker_creates_nothing() {
        let (root, resources, documents) = fixture();
        fs::write(resources.join("CleanNightsSeed.enabled"), b"Nights").unwrap();
        assert_eq!(initialize(&resources, &documents).unwrap_err().kind(), io::ErrorKind::InvalidInput);
        assert!(!documents.join("touchHLE_sandbox").exists()); fs::remove_dir_all(root).unwrap();
    }
    #[test]
    fn fresh_nights_seed_adds_only_unlock_to_clean_user() {
        let (root, resources, documents) = fixture();
        fs::write(resources.join("CleanUserSeed.enabled"), b"User").unwrap();
        fs::write(resources.join("CleanNightsSeed.enabled"), b"Nights").unwrap();
        assert_eq!(initialize(&resources, &documents).unwrap(), Outcome::CreatedCleanUserNights);
        let guest = documents.join("touchHLE_sandbox/com.popcap.ios.PeggleHD");
        assert_eq!(fs::read(guest.join("Documents/userdata/users.dat")).unwrap(), USER_INDEX);
        assert_eq!(fs::read_dir(guest.join("Documents/userdata")).unwrap().count(), 1);
        assert_eq!(fs::read(guest.join("Library/Preferences/com.popcap.ios.PeggleHD.plist")).unwrap(), USER_NIGHTS_PREFERENCES);
        fs::remove_dir_all(root).unwrap();
    }
    #[test]
    fn nights_variant_cannot_overwrite_existing_unlock_progress_or_settings() {
        let (root, resources, documents) = fixture();
        fs::write(resources.join("CleanUserSeed.enabled"), b"User").unwrap();
        initialize(&resources, &documents).unwrap();
        let guest = documents.join("touchHLE_sandbox/com.popcap.ios.PeggleHD");
        let files = ["Documents/userdata/users.dat", "Documents/userdata/user1.dat",
                     "Documents/userdata/stage1.sav", "Documents/userdata/stagenights1.sav",
                     "Documents/userdata/trophynights1.sav", "Library/Preferences/com.popcap.ios.PeggleHD.plist"];
        for relative in files { fs::write(guest.join(relative), format!("existing:{relative}")).unwrap(); }
        fs::write(resources.join("CleanNightsSeed.enabled"), b"Nights").unwrap();
        assert_eq!(initialize(&resources, &documents).unwrap(), Outcome::ExistingGuestPreserved);
        for relative in files { assert_eq!(fs::read(guest.join(relative)).unwrap(), format!("existing:{relative}").as_bytes()); }
        fs::remove_dir_all(root).unwrap();
    }
    #[test]
    fn repeated_nights_launch_preserves_guest_updated_preferences_and_saves() {
        let (root, resources, documents) = fixture();
        fs::write(resources.join("CleanUserSeed.enabled"), b"User").unwrap();
        fs::write(resources.join("CleanNightsSeed.enabled"), b"Nights").unwrap();
        initialize(&resources, &documents).unwrap();
        let guest = documents.join("touchHLE_sandbox/com.popcap.ios.PeggleHD");
        let preferences = guest.join("Library/Preferences/com.popcap.ios.PeggleHD.plist");
        let save = guest.join("Documents/userdata/stagenights1.sav");
        fs::write(&preferences, b"guest updated settings").unwrap(); fs::write(&save, b"real later progress").unwrap();
        assert_eq!(initialize(&resources, &documents).unwrap(), Outcome::ExistingGuestPreserved);
        assert_eq!(fs::read(&preferences).unwrap(), b"guest updated settings");
        assert_eq!(fs::read(&save).unwrap(), b"real later progress"); fs::remove_dir_all(root).unwrap();
    }
}
