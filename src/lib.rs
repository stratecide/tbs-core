pub mod map;
pub mod player;
pub mod terrain;
pub mod units;
pub mod details;
pub mod game;
//pub mod commanders;
pub mod commander;
pub mod config;
pub mod script;

pub use zipper;
pub use interfaces;

pub const VERSION: &str = env!("CARGO_PKG_VERSION");

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
            fn from_conf(s: &str) -> Result<(Self, &str), crate::config::ConfigParseError> {
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
            Custom(u32)
        }

        impl $name {
            pub const fn list_simple() -> &'static [Self] {
                &[$($name::$member,)*]
            }
    
            /*pub fn get(id: usize) -> Self {
                if id < Self::list_simple().len() {
                    Self::list_simple()[id]
                } else {
                    Self::Custom(id)
                }
            }

            pub fn get_id(&self) -> usize {
                if let Self::Custom(id) = self {
                    id + Self::list_simple().len()
                } else {
                    Self::list_simple().iter().position(|s| s == self).unwrap()
                }
            }*/
        }

        impl crate::config::parse::FromConfig for $name {
            fn from_conf(s: &str) -> Result<(Self, &str), crate::config::ConfigParseError> {
                if let Some(index) = s.find(|c| !char::is_numeric(c)).or(Some(s.len())).filter(|i| *i > 0) {
                    let (number, remainder) = s.split_at(index);
                    if let Ok(custom) = number.parse() {
                        return Ok((Self::Custom(custom), remainder));
                    }
                }
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
                    Self::Custom(c) => write!(f, "{}{c}", stringify!($name)),
                }
            }
        }
    };
}

pub(crate) use enum_with_custom;

