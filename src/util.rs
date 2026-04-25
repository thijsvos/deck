/// Shared xorshift64 RNG used by transitions and entrance animations.
///
/// Deterministic: the same seed always produces the same sequence. Suitable for
/// per-frame visual randomness, not cryptography.
pub struct Rng(u64);

impl Rng {
    /// Construct from any seed. The low bit is forced on so seed 0 is not
    /// stuck producing zeros.
    pub fn new(seed: u64) -> Self {
        Self(seed | 1)
    }

    /// Next 64-bit pseudorandom value.
    pub fn next(&mut self) -> u64 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        self.0
    }

    /// Next f64 in `[0.0, 1.0)`. Uses the top 53 bits of `next()` to fill the
    /// f64 mantissa — full entropy, no modulo bias.
    pub fn next_f64(&mut self) -> f64 {
        (self.next() >> 11) as f64 / (1u64 << 53) as f64
    }
}

/// Glitch character palette used by decrypt entrances and glitch transitions.
pub const GLITCH_CHARS: &[char] = &[
    '!', '@', '#', '$', '%', '^', '&', '*', '<', '>', '{', '}', '[', ']', '|', '/', '\\', '~', '░',
    '▒', '▓', '█', '▄', '▀', '▌', '▐',
];

/// FNV-1a 64-bit hash. Used for cache keys where collisions are tolerable
/// (we fall back to a recompute on miss). Not cryptographic.
pub fn fnv1a(s: &str) -> u64 {
    let mut h: u64 = 0xcbf29ce484222325;
    for b in s.bytes() {
        h ^= b as u64;
        h = h.wrapping_mul(0x100000001b3);
    }
    h
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn next_f64_in_unit_range() {
        let mut rng = Rng::new(123);
        for _ in 0..1000 {
            let v = rng.next_f64();
            assert!((0.0..1.0).contains(&v), "got {v}");
        }
    }

    #[test]
    fn fnv1a_is_deterministic() {
        assert_eq!(fnv1a("hello"), fnv1a("hello"));
        assert_ne!(fnv1a("hello"), fnv1a("world"));
    }

    #[test]
    fn fnv1a_empty_is_offset_basis() {
        assert_eq!(fnv1a(""), 0xcbf29ce484222325);
    }
}
