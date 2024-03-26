use std::collections::{HashMap, HashSet};
use interfaces::game_interface::ClientPerspective;

use crate::config::parse::{parse_tuple1, parse_tuple2, string_base, FromConfig};
use crate::config::ConfigParseError;
use crate::details::Detail;
use crate::game::event_handler::EventHandler;
use crate::map::direction::Direction;
use crate::map::map_view::MapView;
use crate::map::point::Point;
use crate::units::attributes::AttributeKey;

use super::unit::anger_kraken;

#[derive(Debug, Clone)]
pub enum PlayerScript {
    Kraken,
    MassDamage(u8),
    MassDamageAura(u8, u8),
    MassHeal(u8),
    ZombieResurrection(u8),
}

impl FromConfig for PlayerScript {
    fn from_conf(s: &str) -> Result<(Self, &str), ConfigParseError> {
        let (base, mut s) = string_base(s);
        Ok((match base {
            "Kraken" => Self::Kraken,
            "MassDamage" => {
                let (damage, r) = parse_tuple1(s)?;
                s = r;
                Self::MassDamage(1.max(damage))
            }
            "MassDamageAura" => {
                let (range, damage, r) = parse_tuple2(s)?;
                s = r;
                Self::MassDamageAura(range, 1.max(damage))
            }
            "MassHeal" => {
                let (heal, r) = parse_tuple1(s)?;
                s = r;
                Self::MassHeal(1.max(heal))
            }
            "ZombieResurrection" => {
                let (hp, r) = parse_tuple1(s)?;
                s = r;
                Self::ZombieResurrection(1.max(100.min(hp)))
            }
            invalid => return Err(ConfigParseError::UnknownEnumMember(format!("PlayerScript::{}", invalid))),
        }, s))
    }
}

impl PlayerScript {
    pub fn trigger<D: Direction>(&self, handler: &mut EventHandler<D>, owner_id: i8) {
        match self {
            Self::Kraken => anger_kraken(handler),
            Self::MassDamage(damage) => mass_damage(handler, handler.environment().get_team(owner_id), *damage),
            Self::MassDamageAura(range, damage) => mass_damage_aura(handler, owner_id, *range, *damage),
            Self::MassHeal(heal) => mass_heal(handler, owner_id, *heal),
            Self::ZombieResurrection(hp) => zombie_resurrection(handler, owner_id as u8, *hp),
        }
    }
}

pub(super) fn mass_damage<D: Direction>(handler: &mut EventHandler<D>, team: ClientPerspective, damage: u8) {
    let points = handler.get_map().all_points();
    deal_damage(handler, points.into_iter(), team, damage);
}

pub(super) fn mass_damage_aura<D: Direction>(handler: &mut EventHandler<D>, owner_id: i8, range: u8, damage: u8) {
    let team = handler.environment().get_team(owner_id);
    let mut aura = HashSet::new();
    for p in handler.get_map().all_points() {
        if let Some(unit) = handler.get_map().get_unit(p) {
            if unit.get_owner_id() == owner_id {
                for layer in handler.get_map().range_in_layers(p, range as usize) {
                    for p in layer {
                        aura.insert(p);
                    }
                }
            }
        }
    }
    deal_damage(handler, aura.into_iter(), team, damage);
}

pub(super) fn deal_damage<D: Direction>(handler: &mut EventHandler<D>, points: impl Iterator<Item = Point>, team: ClientPerspective, damage: u8) {
    let mut damage_map = HashMap::new();
    let mut dead = HashSet::new();
    for p in points {
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
    for (index, detail) in handler.get_map().get_details(p).iter().enumerate() {
        match detail {
            Detail::Skull(skull) => {
                if skull.get_owner_id() == owner_id {
                    let unit = skull.unit(handler.environment(), hp);
                    handler.detail_remove(p, index.into());
                    handler.unit_creation(p, unit);
                }
                break;
            }
            _ => ()
        }
    }
}
