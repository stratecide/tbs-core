use std::collections::{HashMap, HashSet};
use std::str::FromStr;
use interfaces::game_interface::ClientPerspective;

use crate::config::ConfigParseError;
use crate::details::Detail;
use crate::game::event_handler::EventHandler;
use crate::map::direction::Direction;
use crate::map::point::Point;
use crate::units::attributes::AttributeKey;
use crate::units::unit::UnitBuilder;

use super::unit::anger_kraken;

#[derive(Debug, Clone)]
pub enum PlayerScript {
    Kraken,
    MassDamage(u8),
    MassHeal(u8),
    ZombieResurrection(u8),
}

impl FromStr for PlayerScript {
    type Err = ConfigParseError;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let mut it = s.split(&['(', ' ', '-', ')'])
        .map(str::trim);
        Ok(match it.next().unwrap() {
            "Kraken" => Self::Kraken,
            "MassDamage" => {
                let damage = it.next().ok_or(ConfigParseError::NotEnoughValues(s.to_string()))?;
                let damage = damage.parse().map_err(|_| ConfigParseError::InvalidInteger(damage.to_string()))?;
                Self::MassDamage(1.max(damage))
            }
            "MassHeal" => {
                let heal = it.next().ok_or(ConfigParseError::NotEnoughValues(s.to_string()))?;
                let heal = heal.parse().map_err(|_| ConfigParseError::InvalidInteger(heal.to_string()))?;
                Self::MassHeal(1.max(heal))
            }
            "ZombieResurrection" => {
                let hp = it.next().ok_or(ConfigParseError::NotEnoughValues(s.to_string()))?;
                let hp = hp.parse().map_err(|_| ConfigParseError::InvalidInteger(hp.to_string()))?;
                Self::ZombieResurrection(1.max(100.min(hp)))
            }
            invalid => return Err(ConfigParseError::UnknownEnumMember(invalid.to_string())),
        })
    }
}

impl PlayerScript {
    pub fn trigger<D: Direction>(&self, handler: &mut EventHandler<D>, owner_id: i8) {
        match self {
            Self::Kraken => anger_kraken(handler),
            Self::MassDamage(damage) => mass_damage(handler, handler.environment().get_team(owner_id), *damage),
            Self::MassHeal(heal) => mass_heal(handler, owner_id, *heal),
            Self::ZombieResurrection(hp) => zombie_resurrection(handler, owner_id as u8, *hp),
        }
    }
}

pub(super) fn mass_damage<D: Direction>(handler: &mut EventHandler<D>, team: ClientPerspective, damage: u8) {
    let mut damage_map = HashMap::new();
    let mut dead = HashSet::new();
    for p in handler.get_map().all_points() {
        if let Some(unit) = handler.get_map().get_unit(p) {
            if unit.get_owner_id() > 0 && unit.get_team() != team && unit.has_attribute(AttributeKey::Hp) {
                damage_map.insert(p, damage as u16);
                if unit.get_hp() <= damage {
                    dead.insert(p);
                }
            }
        }
    }
    handler.unit_mass_damage(&damage_map);
    handler.trigger_all_unit_scripts(
        |game, unit, unit_pos, transporter, heroes| {
            if dead.contains(&unit_pos) {
                unit.on_death(game, unit_pos, transporter, None, heroes, &[])
            } else {
                Vec::new()
            }
        },
        |handler| handler.unit_mass_death(&dead),
        |handler, script, unit_pos, unit, _observation_id| {
            script.trigger(handler, unit_pos, unit);
        }
    );
}

pub(super) fn mass_heal<D: Direction>(handler: &mut EventHandler<D>, owner_id: i8, heal: u8) {
    let mut heal_map = HashMap::new();
    for p in handler.get_map().all_points() {
        if let Some(unit) = handler.get_map().get_unit(p) {
            if unit.get_owner_id() == owner_id && unit.has_attribute(AttributeKey::Hp) {
                heal_map.insert(p, heal);
            }
        }
    }
    handler.unit_mass_heal(heal_map);
}

pub(super) fn zombie_resurrection<D: Direction>(handler: &mut EventHandler<D>, owner_id: u8, hp: u8) {
    for p in handler.get_map().all_points() {
        resurrect_zombie(handler, p, owner_id as i8, hp);
    }
}

pub(super) fn resurrect_zombie<D: Direction>(handler: &mut EventHandler<D>, p: Point, owner_id: i8, hp: u8) {
    if handler.get_map().get_unit(p).is_some() {
        return;
    }
    for (index, detail) in handler.get_map().get_details(p).into_iter().enumerate() {
        match detail {
            Detail::Skull(o, unit_type) => {
                if o.0 == owner_id {
                    handler.detail_remove(p, index.into());
                    let unit = UnitBuilder::new(handler.environment(), unit_type)
                    .set_owner_id(owner_id)
                    .set_hp(hp)
                    .set_zombified(true)
                    .build_with_defaults();
                    handler.unit_creation(p, unit);
                }
                break;
            }
            _ => ()
        }
    }
}
