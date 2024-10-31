use std::fmt::Debug;
use std::ops::{Add, Div, Mul, Sub};

use num_rational::Rational32;

use crate::script::executor::Executor;

use super::file_loader::FileLoader;
use super::parse::{parse_tuple1, string_base, FromConfig};
use super::ConfigParseError;

pub trait MulRational32: Debug + Clone + Add<Self, Output = Self> + Sub<Self, Output = Self> + Mul<Self, Output = Self> + Div<Self, Output = Self> {
    fn mul_r32(self, other: Rational32) -> Self;
}

#[derive(Debug, Clone, Copy)]
pub enum NumberMod<T: MulRational32 + FromConfig> {
    Keep,
    Replace(T),
    Add(T),
    Sub(T),
    Mul(Rational32),
    MulAdd(Rational32, T),
    MulSub(Rational32, T),
    Rhai(usize),
    RhaiReplace(usize),
}

impl<T: MulRational32 + FromConfig> FromConfig for NumberMod<T> {
    fn from_conf<'a>(s: &'a str, loader: &mut FileLoader) -> Result<(Self, &'a str), ConfigParseError> {
        if s.len() == 0 {
            return Ok((Self::Keep, ""));
        }
        match s.get(..1) {
            Some("=") => T::from_conf(&s[1..], loader).map(|(n, s)| (Self::Replace(n), s)),
            Some("+") => T::from_conf(&s[1..], loader).map(|(n, s)| (Self::Add(n), s)),
            Some("-") => T::from_conf(&s[1..], loader).map(|(n, s)| (Self::Sub(n), s)),
            Some("*") => {
                let (first, s) = Rational32::from_conf(&s[1..], loader)?;
                if s.starts_with('-') {
                    let (second, s) = T::from_conf(&s[1..], loader)?;
                    Ok((Self::MulSub(first, second), s))
                } else if s.starts_with('+') {
                    let (second, s) = T::from_conf(&s[1..], loader)?;
                    Ok((Self::MulAdd(first, second), s))
                } else {
                    Ok((Self::Mul(first), s))
                }
            }
            _ => {
                let (base, s) = string_base(s);
                match base {
                    "Rhai" | "Script" => {
                        let (name, s) = parse_tuple1::<String>(s, loader)?;
                        let f = loader.rhai_function(&name, 0..=1)?;
                        if f.parameter_count == 0 {
                            Ok((Self::RhaiReplace(f.index), s))
                        } else {
                            Ok((Self::Rhai(f.index), s))
                        }
                    }
                    _ => {
                        return Err(ConfigParseError::InvalidNumberModifier(s.to_string()))
                    }
                }
            }
        }
    }
}

impl<T: MulRational32 + FromConfig + Clone + Send + Sync + 'static> NumberMod<T> {
    pub fn ignores_previous_value(&self) -> bool {
        match self {
            Self::Replace(_) => true,
            Self::RhaiReplace(_) => true,
            Self::Mul(value) => *value.numer() == 0,
            _ => false
        }
    }

    // TODO: prevent overflow / underflow
    pub fn update_value(&self, value: T, executor: &Executor) -> T {
        match self.clone() {
            Self::Keep => value,
            Self::Replace(v) => v,
            Self::Add(a) => value + a,
            Self::Sub(a) => value - a,
            Self::Mul(a) => value.mul_r32(a),
            Self::MulAdd(a, b) => value.mul_r32(a) + b,
            Self::MulSub(a, b) => value.mul_r32(a) - b,
            Self::Rhai(function_index) => {
                match executor.run(function_index, (value.clone(), )) {
                    Ok(t) => t,
                    Err(e) => {
                        // TODO: log error
                        println!("NumberMod::Rhai {e}");
                        value
                    }
                }
            }
            Self::RhaiReplace(function_index) => {
                match executor.run(function_index, ()) {
                    Ok(t) => t,
                    Err(e) => {
                        // TODO: log error
                        println!("NumberMod::RhaiReplace {e}");
                        value
                    }
                }
            }
        }
    }

    pub fn update_value_repeatedly<'a>(mut value: T, iter: impl DoubleEndedIterator<Item = Self>, executor: &Executor) -> T {
        let mut stack = Vec::new();
        for v in iter.rev() {
            let ignores_previous = v.ignores_previous_value();
            stack.push(v);
            if ignores_previous {
                break;
            }
        }
        while let Some(v) = stack.pop() {
            value = v.update_value(value, executor);
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
