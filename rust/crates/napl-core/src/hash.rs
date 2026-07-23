//! SHA-256 content hashing, hex-encoded to match the TS `contentHash`.

use sha2::{Digest, Sha256};

/// Lowercase hex-encoded SHA-256 of the UTF-8 bytes of `content`, identical to
/// the TypeScript `createHash('sha256').update(content, 'utf8').digest('hex')`.
#[must_use]
pub fn content_hash(content: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(content.as_bytes());
    hex::encode(hasher.finalize())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn deterministic_for_identical_content() {
        assert_eq!(content_hash("hello"), content_hash("hello"));
    }

    #[test]
    fn differs_for_different_content() {
        assert_ne!(content_hash("hello"), content_hash("world"));
    }

    #[test]
    fn produces_64_char_hex() {
        let h = content_hash("x");
        assert_eq!(h.len(), 64);
        assert!(h
            .chars()
            .all(|c| c.is_ascii_hexdigit() && !c.is_ascii_uppercase()));
    }

    #[test]
    fn matches_known_sha256_vector() {
        assert_eq!(
            content_hash("hello"),
            "2cf24dba5fb0a30e26e83b2ac5b9e29e1b161e5c1fa7425e73043362938b9824"
        );
    }
}
