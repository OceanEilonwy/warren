//! Small deterministic PRNG (xoshiro256++ seeded with splitmix64) plus the few
//! distributions the simulation needs. No external crate so runs are reproducible
//! across platforms and versions.

#[derive(Clone, Debug)]
pub struct Rng {
    s: [u64; 4],
}

fn splitmix64(state: &mut u64) -> u64 {
    *state = state.wrapping_add(0x9E37_79B9_7F4A_7C15);
    let mut z = *state;
    z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
    z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
    z ^ (z >> 31)
}

impl Rng {
    pub fn new(seed: u64) -> Self {
        let mut sm = seed;
        Rng { s: [splitmix64(&mut sm), splitmix64(&mut sm), splitmix64(&mut sm), splitmix64(&mut sm)] }
    }

    fn next_u64(&mut self) -> u64 {
        let result = self.s[0].wrapping_add(self.s[3]).rotate_left(23).wrapping_add(self.s[0]);
        let t = self.s[1] << 17;
        self.s[2] ^= self.s[0];
        self.s[3] ^= self.s[1];
        self.s[1] ^= self.s[2];
        self.s[0] ^= self.s[3];
        self.s[2] ^= t;
        self.s[3] = self.s[3].rotate_left(45);
        result
    }

    /// Uniform in [0, 1).
    pub fn unit(&mut self) -> f64 {
        (self.next_u64() >> 11) as f64 / (1u64 << 53) as f64
    }

    /// True with probability `p`.
    pub fn chance(&mut self, p: f64) -> bool {
        self.unit() < p
    }

    /// Standard normal (Box–Muller).
    pub fn normal(&mut self) -> f64 {
        let u = 1.0 - self.unit();
        let v = self.unit();
        (-2.0 * u.ln()).sqrt() * (2.0 * std::f64::consts::PI * v).cos()
    }

    /// Poisson (Knuth). Fine for the small means used here.
    pub fn poisson(&mut self, mean: f64) -> u32 {
        let l = (-mean).exp();
        let mut k = 0u32;
        let mut p = 1.0;
        loop {
            k += 1;
            p *= self.unit();
            if p <= l {
                return k - 1;
            }
        }
    }

    pub fn binomial(&mut self, n: u32, p: f64) -> u32 {
        (0..n).filter(|_| self.unit() < p).count() as u32
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn deterministic_for_a_seed() {
        let mut a = Rng::new(42);
        let mut b = Rng::new(42);
        for _ in 0..100 {
            assert_eq!(a.unit().to_bits(), b.unit().to_bits());
        }
        assert_ne!(Rng::new(1).unit(), Rng::new(2).unit());
    }

    #[test]
    fn unit_is_in_range_and_unbiased() {
        let mut r = Rng::new(7);
        let n = 100_000;
        let mut sum = 0.0;
        for _ in 0..n {
            let u = r.unit();
            assert!((0.0..1.0).contains(&u));
            sum += u;
        }
        assert!((sum / n as f64 - 0.5).abs() < 0.01);
    }

    #[test]
    fn normal_moments() {
        let mut r = Rng::new(3);
        let n = 100_000;
        let xs: Vec<f64> = (0..n).map(|_| r.normal()).collect();
        let mean = xs.iter().sum::<f64>() / n as f64;
        let var = xs.iter().map(|x| (x - mean).powi(2)).sum::<f64>() / n as f64;
        assert!(mean.abs() < 0.02, "mean {mean}");
        assert!((var - 1.0).abs() < 0.03, "var {var}");
    }

    #[test]
    fn poisson_and_binomial_means() {
        let mut r = Rng::new(9);
        let n = 50_000;
        let pm = (0..n).map(|_| r.poisson(8.0) as f64).sum::<f64>() / n as f64;
        assert!((pm - 8.0).abs() < 0.1, "poisson mean {pm}");
        let bm = (0..n).map(|_| r.binomial(20, 0.3) as f64).sum::<f64>() / n as f64;
        assert!((bm - 6.0).abs() < 0.1, "binomial mean {bm}");
        assert_eq!(r.binomial(0, 0.5), 0);
        assert_eq!(r.binomial(10, 0.0), 0);
        assert_eq!(r.binomial(10, 1.0), 10);
    }
}
