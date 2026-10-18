//! A password that never prints itself and is overwritten when dropped.
use serde::{Deserialize, Deserializer, Serialize, Serializer};

/// A password typed by the person. It has no `Debug` or `Display`, so it can
/// never reach a log line or a panic message by accident, and its bytes are
/// overwritten when it is dropped or cleared. It crosses to the worker only
/// inside the JSON written to the worker's standard input.
#[derive(Clone, Default, PartialEq, Eq)]
pub struct Secret(String);

impl Secret {
    pub fn new(value: impl Into<String>) -> Self {
        Self(value.into())
    }

    pub fn expose(&self) -> &str {
        &self.0
    }

    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }

    pub fn chars(&self) -> usize {
        self.0.chars().count()
    }

    pub fn push(&mut self, text: &str) {
        self.0.push_str(text);
    }

    pub fn pop(&mut self) {
        self.0.pop();
    }

    /// Overwrites the bytes in place before letting them go.
    pub fn clear(&mut self) {
        // SAFETY: zero bytes are valid UTF-8, and the string is emptied
        // straight after, so no one observes the intermediate state.
        unsafe {
            for byte in self.0.as_bytes_mut() {
                std::ptr::write_volatile(byte, 0);
            }
        }
        self.0.clear();
    }
}

impl Drop for Secret {
    fn drop(&mut self) {
        self.clear();
    }
}

impl Serialize for Secret {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(&self.0)
    }
}

impl<'de> Deserialize<'de> for Secret {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        String::deserialize(deserializer).map(Secret)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn clearing_empties_the_value() {
        let mut secret = Secret::new("hunter22");
        secret.clear();
        assert!(secret.is_empty());
    }

    #[test]
    fn round_trips_through_json_without_a_debug_form() {
        let secret = Secret::new("zażółć gęślą");
        let json = serde_json::to_string(&secret).unwrap();
        let back: Secret = serde_json::from_str(&json).unwrap();
        assert_eq!(back.expose(), "zażółć gęślą");
    }
}
