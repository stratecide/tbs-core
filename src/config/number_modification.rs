use std::ops::{Add, Neg, Mul};
use std::str::FromStr;
use std::marker::PhantomData;
use serde::Deserialize;
use serde::de::Visitor;

pub enum NumberMod<T: Add<T> + Neg<Output=T> + Mul + FromStr> {
    Keep,
    Replace(T),
    Add(T),
    Mul(T),
}

impl<'de, T: Add<T> + Neg<Output=T> + Mul + FromStr> Deserialize<'de> for NumberMod<T> {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        deserializer.deserialize_str(NumberModVisitor {data: PhantomData})
    }
}

struct NumberModVisitor<T: Add<T> + Neg<Output=T> + Mul + FromStr> {
    data: PhantomData<T>
}
impl<'de, T: Add<T> + Neg<Output=T> + Mul + FromStr> Visitor<'de> for NumberModVisitor<T> {
    type Value = NumberMod<T>;
    fn expecting(&self, formatter: &mut std::fmt::Formatter) -> std::fmt::Result {
        formatter.write_str("an empty string or a number with an optional sign (+-*)")
    }

    fn visit_str<E: serde::de::Error>(self, v: &str) -> Result<Self::Value, E> {
        if v.len() == 0 {
            return Ok(NumberMod::Keep)
        }
        if v.starts_with("+") {
            
        }
        match v.get(..1) {
            None => Ok(NumberMod::Keep),
            Some("+") => {
                Ok(NumberMod::Add(T::from_str(&v[1..]).map_err(|e| E::custom(format!("invalid value '{v}'")))?))
            }
            Some("-") => {
                Ok(NumberMod::Add(-T::from_str(&v[1..]).map_err(|e| E::custom(format!("invalid value '{v}'")))?))
            }
            Some("*") => {
                Ok(NumberMod::Mul(T::from_str(&v[1..]).map_err(|e| E::custom(format!("invalid value '{v}'")))?))
            }
            _ => Err(E::custom(format!("invalid value '{v}'")))
        }
    }
}
