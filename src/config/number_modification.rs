use std::fmt::Debug;
use std::ops::{Add, Div, Mul, Sub};
use std::str::FromStr;

use super::ConfigParseError;

#[derive(Debug, Clone)]
pub enum NumberMod<T: Debug + Clone + Add<T, Output = T> + Sub<T, Output = T> + Mul<T, Output = T> + Div<T, Output = T> + FromStr> {
    Keep,
    Replace(T),
    Add(T),
    Sub(T),
    Mul(T),
    Div(T),
    MulAdd(T, T),
    MulSub(T, T),
    DivAdd(T, T),
    DivSub(T, T),
}

impl<T: Debug + Clone + Add<T, Output = T> + Sub<T, Output = T> + Mul<T, Output = T> + Div<T, Output = T> + FromStr> FromStr for NumberMod<T> {
    type Err = ConfigParseError;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.get(..1) {
            None => Ok(Self::Keep),
            Some("=") => Ok(Self::Replace(s.get(1..).and_then(|s| T::from_str(s).ok()).ok_or(ConfigParseError::InvalidNumber(s.to_string()))?)),
            Some("+") => Ok(Self::Add(s.get(1..).and_then(|s| T::from_str(s).ok()).ok_or(ConfigParseError::InvalidNumber(s.to_string()))?)),
            Some("-") => Ok(Self::Sub(s.get(1..).and_then(|s| T::from_str(s).ok()).ok_or(ConfigParseError::InvalidNumber(s.to_string()))?)),
            Some("*") => {
                let s = s.get(1..)
                .ok_or(ConfigParseError::InvalidNumberModifier(s.to_string()))?;
                if let Some(index) = s.rfind("-").filter(|i| *i > 0) {
                    let (first, second) = s.split_at(index);
                    let first = T::from_str(first).map_err(|_| ConfigParseError::InvalidNumber(s.to_string()))?;
                    let second = second.get(1..).and_then(|s| T::from_str(s).ok()).ok_or(ConfigParseError::InvalidNumber(s.to_string()))?;
                    Ok(Self::MulSub(first, second))
                } else if let Some(index) = s.rfind("+") {
                    let (first, second) = s.split_at(index);
                    let first = T::from_str(first).map_err(|_| ConfigParseError::InvalidNumber(s.to_string()))?;
                    let second = second.get(1..).and_then(|s| T::from_str(s).ok()).ok_or(ConfigParseError::InvalidNumber(s.to_string()))?;
                    Ok(Self::MulAdd(first, second))
                } else {
                    Ok(Self::Mul(s.get(1..).and_then(|s| T::from_str(s).ok()).ok_or(ConfigParseError::InvalidNumber(s.to_string()))?))
                }
            }
            Some("/") => {
                let s = s.get(1..)
                .ok_or(ConfigParseError::InvalidNumberModifier(s.to_string()))?;
                if let Some(index) = s.rfind("-").filter(|i| *i > 0) {
                    let (first, second) = s.split_at(index);
                    let first = T::from_str(first).map_err(|_| ConfigParseError::InvalidNumber(s.to_string()))?;
                    let second = second.get(1..).and_then(|s| T::from_str(s).ok()).ok_or(ConfigParseError::InvalidNumber(s.to_string()))?;
                    Ok(Self::DivSub(first, second))
                } else if let Some(index) = s.rfind("+") {
                    let (first, second) = s.split_at(index);
                    let first = T::from_str(first).map_err(|_| ConfigParseError::InvalidNumber(s.to_string()))?;
                    let second = second.get(1..).and_then(|s| T::from_str(s).ok()).ok_or(ConfigParseError::InvalidNumber(s.to_string()))?;
                    Ok(Self::DivAdd(first, second))
                } else {
                    Ok(Self::Div(s.get(1..).and_then(|s| T::from_str(s).ok()).ok_or(ConfigParseError::InvalidNumber(s.to_string()))?))
                }
            }
            Some(invalid) => Err(ConfigParseError::InvalidNumberModifier(invalid.to_string()))
        }
    }
}

impl<T: Debug + Clone + Add<T, Output = T> + Sub<T, Output = T> + Mul<T, Output = T> + Div<T, Output = T> + FromStr + 'static> NumberMod<T> {
    pub fn ignores_previous_value(&self) -> bool {
        match self {
            Self::Replace(_) => true,
            _ => false
        }
    }

    pub fn update_value(&self, value: T) -> T {
        match self.clone() {
            Self::Keep => value,
            Self::Replace(v) => v,
            Self::Add(a) => value + a,
            Self::Sub(a) => value - a,
            Self::Mul(a) => value * a,
            Self::Div(a) => value / a,
            Self::MulAdd(a, b) => value * a + b,
            Self::MulSub(a, b) => value * a - b,
            Self::DivAdd(a, b) => value / a + b,
            Self::DivSub(a, b) => value / a - b,
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
