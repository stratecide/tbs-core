#![feature(mapped_lock_guards)]
pub mod map;
pub mod player;
pub mod terrain;
pub mod units;
pub mod tokens;
pub mod game;
pub mod commander;
pub mod config;
pub mod script;
pub mod tags;
pub mod combat;

use uniform_smart_pointer::SendSyncBound;
pub use zipper;
pub use interfaces;

pub const VERSION: &str = env!("CARGO_PKG_VERSION");

#[cfg(feature = "tracing")]
macro_rules! debug {
    ($($arg:tt)*) => {{
        tracing::debug!($($arg)*);
    }};
}
#[cfg(not(feature = "tracing"))]
macro_rules! debug {
    ($($arg:tt)*) => {{
        println!($($arg)*);
    }};
}

#[cfg(feature = "tracing")]
#[macro_export]
macro_rules! warn {
    ($($arg:tt)*) => {{
        tracing::warn!($($arg)*);
    }};
}
#[cfg(not(feature = "tracing"))]
#[macro_export]
macro_rules! warn {
    ($($arg:tt)*) => {{
        println!($($arg)*);
    }};
}

#[cfg(feature = "tracing")]
macro_rules! error {
    ($($arg:tt)*) => {{
        tracing::error!($($arg)*);
    }};
}
#[cfg(not(feature = "tracing"))]
macro_rules! error {
    ($($arg:tt)*) => {{
        println!($($arg)*);
    }};
}

pub(crate) use {debug, error};

// TODO: create derive macro for FromConfig instead of adding it to listable_enum and enum_with_custom

macro_rules! listable_enum {(
    $(#[$meta:meta])*
    $vis:vis enum $name:ident {
        $(
            $member:ident,
        )*
    }) => {
        $(#[$meta])*
        $vis enum $name {
            $($member),*
        }

        impl $name {
            #[allow(dead_code)]
            pub const fn list() -> &'static [Self] {
                &[$($name::$member,)*]
            }
        }

        impl crate::config::parse::FromConfig for $name {
            fn from_conf<'a>(s: &'a str, _: &mut crate::config::file_loader::FileLoader) -> Result<(Self, &'a str), crate::config::ConfigParseError> {
                let (base, s) = crate::config::parse::string_base(s);
                match base {
                    $(stringify!($member) => Ok((Self::$member, s)),)*
                    _ => Err(crate::config::ConfigParseError::UnknownEnumMember(format!("{}::{base} - {s}", stringify!($name))))
                }
            }
        }

        impl std::fmt::Display for $name {
            fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                match self {
                    $(Self::$member => write!(f, "{}", stringify!($member)),)*
                }
            }
        }
    };
}
pub(crate) use listable_enum;

macro_rules! enum_with_custom {(
    $(#[$meta:meta])*
    $vis:vis enum $name:ident {
        $(
            $member:ident,
        )*
    }) => {
        $(#[$meta])*
        $vis enum $name {
            $($member),*,
            Custom(String)
        }

        impl crate::config::parse::FromConfig for $name {
            fn from_conf<'a>(s: &'a str, _: &mut crate::config::file_loader::FileLoader) -> Result<(Self, &'a str), crate::config::ConfigParseError> {
                let (base, s) = crate::config::parse::string_base(s);
                match base {
                    $(stringify!($member) => Ok((Self::$member, s)),)*
                    custom => Ok((Self::Custom(custom.to_string()), s))
                }
            }
        }

        impl std::fmt::Display for $name {
            fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                match self {
                    $(Self::$member => write!(f, "{}", stringify!($member)),)*
                    Self::Custom(c) => write!(f, "{}::{c}", stringify!($name)),
                }
            }
        }
    };
}
pub(crate) use enum_with_custom;

pub(crate) fn dyn_opt<T: 'static + SendSyncBound + Clone>(value: Option<T>) -> rhai::Dynamic {
    value.map(rhai::Dynamic::from).unwrap_or(rhai::Dynamic::UNIT)
}
