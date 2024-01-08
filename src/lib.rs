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
            pub const fn list() -> &'static [Self] {
                &[$($name::$member,)*]
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
            Custom(usize)
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
    };
}

pub(crate) use enum_with_custom;

