/// Shared xorshift64 RNG used by transitions and entrance animations.
pub struct Rng(u64);

impl Rng {
    pub fn new(seed: u64) -> Self {
        Self(seed | 1)
    }

    pub fn next(&mut self) -> u64 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        self.0
    }

    pub fn next_f64(&mut self) -> f64 {
        (self.next() % 10000) as f64 / 10000.0
    }
}

pub const GLITCH_CHARS: &[char] = &[
    '!', '@', '#', '$', '%', '^', '&', '*', '<', '>', '{', '}', '[', ']', '|', '/', '\\', '~', '░',
    '▒', '▓', '█', '▄', '▀', '▌', '▐',
];
