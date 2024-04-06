use std::collections::HashMap;
use std::collections::HashSet;

use crate::config::parse::*;
use crate::config::ConfigParseError;
use crate::game::event_handler::EventHandler;
use crate::map::direction::Direction;
use crate::map::map_view::MapView;
use crate::map::point::Point;
use crate::units::attributes::AttributeKey;
use crate::units::attributes::AttributeOverride;
use crate::units::unit::Unit;
use crate::units::unit_types::UnitType;

use super::unit::*;

#[derive(Debug, Clone)]
pub enum DeathScript {
    UnitScript(UnitScript),
    Type(UnitType),
    CopyAttacker(HashSet<AttributeKey>),
    Attributes(HashSet<AttributeOverride>),
    Revive,
}

impl FromConfig for DeathScript {
    fn from_conf(s: &str) -> Result<(Self, &str), ConfigParseError> {
        let (base, mut remainder) = string_base(s);
        Ok((match base {
            "Type" => {
                let (typ, r) = UnitType::from_conf(remainder)?;
                remainder = r;
                Self::Type(typ)
            }
            "CopyAttacker" => {
                let (attributes, r) = parse_inner_vec::<AttributeKey>(remainder, true)?;
                remainder = r;
                Self::CopyAttacker(attributes.into_iter().collect())
            }
            "Attributes" => {
                let (attributes, r) = parse_inner_vec::<AttributeOverride>(remainder, true)?;
                remainder = r;
                let mut map = HashMap::new();
                for a in attributes {
                    map.insert(a.key(), a);
                }
                Self::Attributes(map.into_iter().map(|(_, v)| v).collect())
            }
            "Revive" => Self::Revive,
            invalid => {
                if let Ok((us, r)) = UnitScript::from_conf(s) {
                    remainder = r;
                    Self::UnitScript(us)
                } else {
                    return Err(ConfigParseError::UnknownEnumMember(format!("DeathScript::{}", invalid)))
                }
            }
        }, remainder))
    }
}

impl DeathScript {
    pub fn trigger<D: Direction>(&self, handler: &mut EventHandler<D>, corpse: &mut Unit<D>, death_pos: Point, _unload_index: Option<usize>, killer: Option<(&Unit<D>, Point)>) {
        match self {
            Self::UnitScript(us) => {
                us.trigger(handler, death_pos, corpse);
            }
            Self::Type(typ) => {
                let new_unit = typ.instance(handler.environment()).copy_from(corpse).build_with_defaults();
                *corpse = new_unit;
            }
            Self::CopyAttacker(attribute_keys) => {
                if let Some((killer, _)) = killer {
                    for key in attribute_keys {
                        if let Some(attribute) = killer.get_attributes().get(key) {
                            corpse.set_attribute(attribute.clone());
                        }
                    }
                }
            }
            Self::Attributes(attributes) => {
                for attr in attributes {
                    corpse.set_attribute(attr.into());
                }
            }
            Self::Revive => {
                if handler.get_map().get_unit(death_pos).is_none() && corpse.get_hp() > 0 {
                    handler.unit_creation(death_pos, corpse.clone());
                }
            }
        }
    }
}
