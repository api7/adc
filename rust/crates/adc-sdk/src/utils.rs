use sha1::{Digest, Sha1};

/// Deterministic resource id: the sha1 (not sha256) hex digest of `name`.
/// SHA-1 is used only to derive a stable identifier from a name — not for
/// integrity checks, signatures, or password handling, where its known
/// collision weaknesses would matter. It's fixed at sha1 specifically to
/// keep generated ids identical to the reference ADC implementation's.
pub fn generate_id(name: &str) -> String {
    let mut hasher = Sha1::new();
    hasher.update(name.as_bytes());
    // Manual hex encoding rather than `format!("{:x}", ...)`: sha1 0.11's
    // `finalize()` output type (`hybrid_array::Array`) doesn't implement
    // `LowerHex`, unlike the `generic_array::GenericArray` it replaced.
    hasher.finalize().iter().map(|byte| format!("{byte:02x}")).collect()
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
