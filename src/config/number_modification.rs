use std::fmt::Debug;
use std::ops::{Add, Div, Mul, Sub};

use num_rational::Rational32;

use super::parse::FromConfig;
use super::ConfigParseError;

pub trait MulRational32: Debug + Clone + Add<Self, Output = Self> + Sub<Self, Output = Self> + Mul<Self, Output = Self> + Div<Self, Output = Self> {
    fn mul_r32(self, other: Rational32) -> Self;
}

#[derive(Debug, Clone)]
pub enum NumberMod<T: MulRational32 + FromConfig> {
    Keep,
    Replace(T),
    Add(T),
    Sub(T),
    Mul(Rational32),
    MulAdd(Rational32, T),
    MulSub(Rational32, T),
}

impl<T: MulRational32 + FromConfig> FromConfig for NumberMod<T> {
    fn from_conf(s: &str) -> Result<(Self, &str), ConfigParseError> {
        if s.len() == 0 {
            return Ok((Self::Keep, ""));
        }
        match s.get(..1) {
            Some("=") => T::from_conf(&s[1..]).map(|(n, s)| (Self::Replace(n), s)),
            Some("+") => T::from_conf(&s[1..]).map(|(n, s)| (Self::Add(n), s)),
            Some("-") => T::from_conf(&s[1..]).map(|(n, s)| (Self::Sub(n), s)),
            Some("*") => {
                let (first, s) = Rational32::from_conf(&s[1..])?;
                if s.starts_with('-') {
                    let (second, s) = T::from_conf(&s[1..])?;
                    Ok((Self::MulSub(first, second), s))
                } else if s.starts_with('+') {
                    let (second, s) = T::from_conf(&s[1..])?;
                    Ok((Self::MulAdd(first, second), s))
                } else {
                    Ok((Self::Mul(first), s))
                }
            }
            _ => return Err(ConfigParseError::InvalidNumberModifier(s.to_string()))
        }
    }
}

impl<T: MulRational32 + FromConfig + 'static> NumberMod<T> {
    pub fn ignores_previous_value(&self) -> bool {
        match self {
            Self::Replace(_) => true,
            _ => false
        }
    }

    // TODO: prevent overflow / underflow
    pub fn update_value(&self, value: T) -> T {
        match self.clone() {
            Self::Keep => value,
            Self::Replace(v) => v,
            Self::Add(a) => value + a,
            Self::Sub(a) => value - a,
            Self::Mul(a) => value.mul_r32(a),
            Self::MulAdd(a, b) => value.mul_r32(a) + b,
            Self::MulSub(a, b) => value.mul_r32(a) - b,
        }
    }

    pub fn update_value_repeatedly<'a>(mut value: T, iter: impl DoubleEndedIterator<Item = &'a Self>) -> T {
        let mut stack = Vec::new();
        for v in iter.rev() {
            stack.push(v);
            if v.ignores_previous_value() {
                break;
            }
        }
        while let Some(v) = stack.pop() {
            value = v.update_value(value);
        }
        value
    }
}

impl MulRational32 for Rational32 {
    fn mul_r32(self, other: Rational32) -> Self {
        self * other
    }
}

impl MulRational32 for u8 {
    fn mul_r32(self, other: Rational32) -> Self {
        (Rational32::from_integer(self as i32) * other).round().to_integer().min(u8::MAX as i32).max(0) as Self
    }
}

impl MulRational32 for i8 {
    fn mul_r32(self, other: Rational32) -> Self {
        (Rational32::from_integer(self as i32) * other).round().to_integer().min(i8::MAX as i32).max(i8::MIN as i32) as Self
    }
}

impl MulRational32 for u32 {
    fn mul_r32(self, other: Rational32) -> Self {
        (Rational32::from_integer(self as i32) * other).round().to_integer().max(0) as Self
    }
}

impl MulRational32 for i32 {
    fn mul_r32(self, other: Rational32) -> Self {
        (Rational32::from_integer(self as i32) * other).round().to_integer() as Self
    }
}
