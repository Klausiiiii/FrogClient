/// Java's `UUID.nameUUIDFromBytes("OfflinePlayer:" + name)`, without dashes.
pub fn offline_uuid(name: &str) -> String {
    use md5::{Digest, Md5};
    let mut hash: [u8; 16] = Md5::digest(format!("OfflinePlayer:{name}").as_bytes()).into();
    hash[6] = (hash[6] & 0x0f) | 0x30; // version 3
    hash[8] = (hash[8] & 0x3f) | 0x80; // IETF variant
    hex::encode(hash)
}

/// Minecraft usernames: 3–16 characters, letters, digits and underscore.
pub fn validate_offline_name(name: &str) -> Result<(), &'static str> {
    if !(3..=16).contains(&name.len()) {
        return Err("Der Name muss 3 bis 16 Zeichen lang sein.");
    }
    if !name.chars().all(|c| c.is_ascii_alphanumeric() || c == '_') {
        return Err("Erlaubt sind nur Buchstaben, Ziffern und _.");
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn offline_uuid_matches_java() {
        assert_eq!(offline_uuid("Notch"), "b50ad385829d3141a2167e7d7539ba7f");
    }

    #[test]
    fn offline_name_validation() {
        assert!(validate_offline_name("ab").is_err());
        assert!(validate_offline_name("Frog_Main").is_ok());
        assert!(validate_offline_name("äöü").is_err());
        assert!(validate_offline_name("abcdefghijklmnopq").is_err());
        assert!(validate_offline_name("with space").is_err());
    }
}
