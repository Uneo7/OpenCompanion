use std::ops::{Add, Mul};

#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct C64 {
    pub re: f64,
    pub im: f64,
}

impl C64 {
    pub const ZERO: C64 = C64 { re: 0.0, im: 0.0 };
    pub const ONE: C64 = C64 { re: 1.0, im: 0.0 };

    pub const fn new(re: f64, im: f64) -> Self {
        Self { re, im }
    }

    /// e^(-jw): one sample of delay at angular frequency `w` (rad / sample).
    pub fn delay(w: f64) -> Self {
        let (s, c) = w.sin_cos();
        Self::new(c, -s)
    }

    pub fn scale(self, k: f64) -> Self {
        Self::new(self.re * k, self.im * k)
    }

    pub fn norm_sqr(self) -> f64 {
        self.re * self.re + self.im * self.im
    }

    pub fn abs(self) -> f64 {
        self.norm_sqr().sqrt()
    }

    pub fn div(self, o: Self) -> Self {
        let d = o.norm_sqr();
        Self::new((self.re * o.re + self.im * o.im) / d, (self.im * o.re - self.re * o.im) / d)
    }
}

impl Add for C64 {
    type Output = C64;
    fn add(self, o: C64) -> C64 {
        C64::new(self.re + o.re, self.im + o.im)
    }
}

impl Mul for C64 {
    type Output = C64;
    fn mul(self, o: C64) -> C64 {
        C64::new(self.re * o.re - self.im * o.im, self.re * o.im + self.im * o.re)
    }
}

pub type Spectrum = Vec<C64>;

pub fn scaled(x: &[C64], k: f64) -> Spectrum {
    x.iter().map(|v| v.scale(k)).collect()
}

pub fn product(x: &[C64], h: &[C64]) -> Spectrum {
    x.iter().zip(h).map(|(a, b)| *a * *b).collect()
}

pub fn mix(x: &[C64], a: f64, y: &[C64], b: f64) -> Spectrum {
    x.iter().zip(y).map(|(u, v)| u.scale(a) + v.scale(b)).collect()
}

