use sha1::{Digest, Sha1};

/// Mirrors `utils.generateId` in `libs/sdk/src/utils.ts` (`sha1` hex digest, not sha256).
pub fn generate_id(name: &str) -> String {
    let mut hasher = Sha1::new();
    hasher.update(name.as_bytes());
    format!("{:x}", hasher.finalize())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn matches_node_sha1_hex() {
        // echo -n "hello" | sha1sum -> aaf4c61ddcc5e8a2dabede0f3b482cd9aea9434d
        assert_eq!(generate_id("hello"), "aaf4c61ddcc5e8a2dabede0f3b482cd9aea9434d");
    }
}
