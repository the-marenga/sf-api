use std::fmt::Write;

use log::warn;

pub const HASH_CONST: &str = "ahHoj2woo1eeChiech6ohphoB7Aithoh";
pub const DEFAULT_CRYPTO_ID: &str = "0-00000000000000";
pub const DEFAULT_SESSION_ID: &str = "00000000000000000000000000000000";

#[must_use]
pub fn sha1_hash(val: &str) -> String {
    use sha1::{Digest, Sha1};
    let mut hasher = Sha1::new();
    hasher.update(val.as_bytes());
    let hash = hasher.finalize();
    let mut result = String::with_capacity(hash.len() * 2);
    for byte in &hash {
        _ = write!(result, "{byte:02x}");
    }
    result
}

/// Converts a S&F string from the server to their original unescaped
/// representation
#[must_use]
pub fn from_sf_string(val: &str) -> String {
    let mut new = String::with_capacity(val.len());
    let mut is_escaped = false;
    for char in val.chars() {
        if char == '$' {
            is_escaped = true;
            continue;
        }
        let escaped_char = match char {
            x if !is_escaped => x,
            'b' => '\n',
            'c' => ':',
            'P' => '%',
            's' => '/',
            'p' => '|',
            '+' => '&',
            'q' => '"',
            'r' => '#',
            'C' => ',',
            'S' => ';',
            'd' => '$',
            x => {
                warn!("Unkown escape sequence: ${x}");
                x
            }
        };
        new.push(escaped_char);
        is_escaped = false;
    }
    new
}

/// Makes a user controlled string, like a new character description safe to use
/// in a request
#[must_use]
pub fn to_sf_string(val: &str) -> String {
    let mut new = String::with_capacity(val.len());
    for char in val.chars() {
        match char {
            '\n' => new.push_str("$b"),
            ':' => new.push_str("$c"),
            '%' => new.push_str("$P"),
            '/' => new.push_str("$s"),
            '|' => new.push_str("$p"),
            '&' => new.push_str("$+"),
            '"' => new.push_str("$q"),
            '#' => new.push_str("$r"),
            ',' => new.push_str("$C"),
            ';' => new.push_str("$S"),
            '$' => new.push_str("$d"),
            _ => new.push(char),
        }
    }
    new
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_from_sf_string() {
        let input = "$bHello$cWorld$PThis$sis a test!";
        let expected_output = "\nHello:World%This/is a test!";
        let result = from_sf_string(input);
        assert_eq!(result, expected_output);

        let input = "$$$$$$$$$$$";
        let expected_output = "";
        let result = from_sf_string(input);
        assert_eq!(result, expected_output);

        let input = "$$b$c$P$s$p$+$q$r$C$S$d";
        let expected_output = "\n:%/|&\"#,;$";
        let result = from_sf_string(input);
        assert_eq!(result, expected_output);
    }
    #[test]
    fn test_to_sf_string() {
        let input = "\nHello:World%This/is a test!";
        let expected_output = "$bHello$cWorld$PThis$sis a test!";
        let result = to_sf_string(input);
        assert_eq!(result, expected_output);

        let input = "\n:%/|&\"#,;$";
        let expected_output = "$b$c$P$s$p$+$q$r$C$S$d";
        let result = to_sf_string(input);
        assert_eq!(result, expected_output);
    }
}
