//! Helpers for displaying identifiers.

use uuid::Uuid;

/// First 8 characters of the UUID in simple (hyphen-less, lowercase hex) form.
///
/// Full UUIDs are kept internally; the short form is only for display and
/// file names.
pub fn short_id(id: &Uuid) -> String {
    let mut s = id.simple().to_string();
    s.truncate(8);
    s
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn short_id_is_first_eight_hex_chars() {
        let id = Uuid::parse_str("6f1c2b7e-0000-4000-8000-000000000001").unwrap();
        assert_eq!(short_id(&id), "6f1c2b7e");
    }

    #[test]
    fn short_id_of_random_uuid_is_lowercase_hex() {
        let s = short_id(&Uuid::new_v4());
        assert_eq!(s.len(), 8);
        assert!(
            s.chars()
                .all(|c| c.is_ascii_digit() || ('a'..='f').contains(&c))
        );
    }
}
