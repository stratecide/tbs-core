use std::collections::HashSet;

use num_rational::Rational32;
use rhai::*;

use crate::combat::AttackPatternType;
use crate::commander::commander_type::CommanderType;
use crate::game::fog::FogIntensity;
use crate::game::game_view::GameView;
use crate::script::executor::Executor;
use crate::units::UnitData;
use crate::{dyn_opt, script::*};
use crate::tags::{FlagKey, TagKey};
use crate::terrain::TerrainType;
use crate::tokens::token_types::TokenType;
use crate::units::hero::{HeroMap, HeroType};
use crate::units::movement::MovementType;
use crate::units::unit_types::UnitType;
use crate::map::direction::Direction;

use super::file_loader::FileLoader;
use super::movement_type_config::MovementPattern;
use super::parse::{parse_inner_vec, parse_inner_vec_dyn, parse_tuple1, parse_tuple2, string_base, FromConfig};
use super::ConfigParseError;

/**
 * UnitFilter and custom actions are the first things to replace with Rhai
 */
#[derive(Debug, Clone)]
pub(crate) enum UnitFilter {
    Rhai(usize),
    // commander
    Commander(CommanderType, Option<u8>),
    CommanderCharge(u32),
    // hero
    Hero(HashSet<(HeroType, Option<u8>)>),
    HeroGlobal(HashSet<(HeroType, Option<u8>)>),
    IsHero(HashSet<(HeroType, Option<u8>)>),
    HeroCharge(Rational32),
    // this unit
    Unit(HashSet<UnitType>),
    Flag(HashSet<FlagKey>),
    Tag(HashSet<TagKey>),
    Movement(HashSet<MovementType>),
    SubMovement(HashSet<MovementType>),
    MovementPattern(HashSet<MovementPattern>),
    AttackPattern(HashSet<AttackPatternType>),
    Unowned,
    // situation/environment
    OwnerTurn,
    Carried,
    Counter,
    Terrain(HashSet<TerrainType>),
    Token(HashSet<TokenType>),
    Fog(HashSet<FogIntensity>),
    Moved, // as in: it moved along a path with at least 1 step
    // recursive
    OtherUnit(Vec<Self>),
    Not(Vec<Self>),
}

impl FromConfig for UnitFilter {
    fn from_conf<'a>(s: &'a str, loader: &mut FileLoader) -> Result<(Self, &'a str), ConfigParseError> {
        let (base, mut remainder) = string_base(s);
        Ok((match base {
            "Rhai" | "Script" => {
                let (name, r) = parse_tuple1::<String>(remainder, loader)?;
                remainder = r;
                Self::Rhai(loader.rhai_function(&name, 0..=0)?.index)
            }
            "Unit" | "U" => {
                let (list, r) = parse_inner_vec::<UnitType>(remainder, true, loader)?;
                remainder = r;
                Self::Unit(list.into_iter().collect())
            }
            "Flag" | "F" => {
                let (list, r) = parse_inner_vec::<FlagKey>(remainder, true, loader)?;
                remainder = r;
                Self::Flag(list.into_iter().collect())
            }
            "Tag" => {
                let (list, r) = parse_inner_vec::<TagKey>(remainder, true, loader)?;
                remainder = r;
                Self::Tag(list.into_iter().collect())
            }
            "Movement" | "M" => {
                let (list, r) = parse_inner_vec::<MovementType>(remainder, true, loader)?;
                remainder = r;
                Self::Movement(list.into_iter().collect())
            }
            "SubMovement" | "SM" => {
                let (list, r) = parse_inner_vec::<MovementType>(remainder, true, loader)?;
                remainder = r;
                Self::SubMovement(list.into_iter().collect())
            }
            "Terrain" | "T" => {
                let (list, r) = parse_inner_vec::<TerrainType>(remainder, true, loader)?;
                remainder = r;
                Self::Terrain(list.into_iter().collect())
            }
            "Token" => {
                let (list, r) = parse_inner_vec::<TokenType>(remainder, true, loader)?;
                remainder = r;
                Self::Token(list.into_iter().collect())
            }
            "MP" | "MovementPattern" => {
                let (list, r) = parse_inner_vec::<MovementPattern>(remainder, true, loader)?;
                remainder = r;
                Self::MovementPattern(list.into_iter().collect())
            }
            "H" | "Hero" => {
                let (list, r) = parse_inner_vec::<(HeroType, Option<u8>)>(remainder, true, loader)?;
                remainder = r;
                Self::Hero(list.into_iter().collect())
            }
            "HG" | "HeroGlobal" => {
                let (list, r) = parse_inner_vec::<(HeroType, Option<u8>)>(remainder, true, loader)?;
                remainder = r;
                Self::HeroGlobal(list.into_iter().collect())
            }
            "IH" | "IsHero" => {
                let (list, r) = parse_inner_vec::<(HeroType, Option<u8>)>(remainder, false, loader)?;
                remainder = r;
                Self::IsHero(list.into_iter().collect())
            }
            "HC" | "HeroCharge" => {
                let (charge, r) = parse_tuple1(remainder, loader)?;
                remainder = r;
                Self::HeroCharge(charge)
            }
            "AP" | "AttackPattern" => {
                let (list, r) = parse_inner_vec::<AttackPatternType>(remainder, true, loader)?;
                remainder = r;
                Self::AttackPattern(list.into_iter().collect())
            }
            "CC" | "CommanderCharge" => {
                let (charge, r) = parse_tuple1(remainder, loader)?;
                remainder = r;
                Self::CommanderCharge(charge)
            }
            "Fog" => {
                let (list, r) = parse_inner_vec::<FogIntensity>(remainder, true, loader)?;
                remainder = r;
                Self::Fog(list.into_iter().collect())
            }
            "Moved" => Self::Moved,
            "Unowned" => Self::Unowned,
            "Commander" | "Co" => {
                if let Ok((commander, power, r)) = parse_tuple2(remainder, loader) {
                    remainder = r;
                    Self::Commander(commander, Some(power))
                } else {
                    let (commander, r) = parse_tuple1(remainder, loader)?;
                    remainder = r;
                    Self::Commander(commander, None)
                }
            }
            "OwnerTurn" => Self::OwnerTurn,
            "Carried" => Self::Carried,
            "Counter" => Self::Counter,
            "OU" | "OtherUnit" => {
                let (list, r) = parse_inner_vec::<Self>(remainder, false, loader)?;
                remainder = r;
                Self::OtherUnit(list.into_iter().collect())
            }
            "Not" => {
                let (list, r) = parse_inner_vec_dyn::<Self>(remainder, true, |s| Self::from_conf(s, loader))?;
                remainder = r;
                Self::Not(list)
            }
            invalid => return Err(ConfigParseError::UnknownEnumMember(format!("UnitFilter::{invalid}"))),
        }, remainder))
    }
}

impl UnitFilter {
    pub fn check<D: Direction>(
        &self,
        game: &impl GameView<D>,
        unit_data: UnitData<D>,
        other_unit_data: Option<UnitData<D>>,
        heroes: &HeroMap<D>,
        // true only during counter-attacks
        is_counter: bool,
    ) -> bool {
        match self {
            Self::Rhai(function_index) => {
                let environment = game.environment();
                let engine = environment.get_engine_board(game);
                let executor = Executor::new(engine, unit_filter_scope(game, unit_data, other_unit_data, heroes, is_counter), environment);
                match executor.run(*function_index, ()) {
                    Ok(result) => result,
                    Err(e) => {
                        let environment = game.environment();
                        environment.log_rhai_error("UnitFilter::Rhai", environment.get_rhai_function_name(*function_index), &e);
                        false
                    }
                }
            }
            Self::Unit(u) => u.contains(&unit_data.unit.typ()),
            Self::Flag(flags) => flags.iter().any(|flag| unit_data.unit.has_flag(flag.0)),
            Self::Tag(tags) => tags.iter().any(|tag| unit_data.unit.get_tag(tag.0).is_some()),
            Self::Movement(m) => m.contains(&unit_data.unit.base_movement_type()),
            Self::SubMovement(m) => m.contains(&unit_data.unit.sub_movement_type()),
            Self::Terrain(t) => t.contains(&game.get_terrain(unit_data.pos).unwrap().typ()),
            Self::Token(t) => {
                for token in game.get_tokens(unit_data.pos) {
                    if t.contains(&token.typ()) {
                        return true;
                    }
                }
                false
            }
            Self::MovementPattern(m) => m.contains(&unit_data.unit.movement_pattern()),
            Self::Hero(h) => {
                for (_, hero, _, _, _) in heroes.get(unit_data.pos, unit_data.unit.get_owner_id()) {
                    let power = hero.get_active_power() as u8;
                    if h.iter().any(|h| h.0 == hero.typ() && h.1.unwrap_or(power) == power) {
                        return true;
                    }
                }
                false
            }
            Self::HeroGlobal(h) => {
                for (_, hero, _, _, _) in heroes.iter_owned(unit_data.unit.get_owner_id()) {
                    let power = hero.get_active_power() as u8;
                    if h.iter().any(|h| h.0 == hero.typ() && h.1.unwrap_or(power) == power) {
                        return true;
                    }
                }
                false
            }
            Self::IsHero(h) => {
                if let Some(hero) = unit_data.unit.get_hero() {
                    let power = hero.get_active_power() as u8;
                    let hero = hero.typ();
                    h.len() == 0 || h.iter().any(|h| h.0 == hero && h.1.unwrap_or(power) == power)
                } else {
                    false
                }
            }
            Self::HeroCharge(charge_ratio) => {
                if let Some(hero) = unit_data.unit.get_hero() {
                    let environment = game.environment();
                    Rational32::new(hero.get_charge() as i32, hero.max_charge(&environment) as i32) >= *charge_ratio
                } else {
                    false
                }
            }
            Self::AttackPattern(a) => {
                let attack_type = game.environment().config.default_attack_pattern(unit_data.unit.typ()).typ(&game.environment());
                a.iter().any(|a| *a == attack_type)
            }
            Self::CommanderCharge(charge) => {
                unit_data.unit.get_commander(game).get_charge() >= *charge
            }
            Self::Fog(f) => {
                let fog = game.get_fog_setting().intensity();
                f.iter().any(|f| *f == fog)
            }
            Self::Moved => {
                unit_data.ballast.len() > 0
            }
            Self::Unowned => unit_data.unit.get_owner_id() < 0,
            Self::Commander(commander_type, power) => {
                let commander = unit_data.unit.get_commander(game);
                commander.typ() == *commander_type
                && (power.is_none() || power.clone().unwrap() as usize == commander.get_active_power())
            }
            Self::OwnerTurn => unit_data.unit.get_owner_id() == game.current_owner(),
            Self::Carried => unit_data.unload_index.is_some(),
            Self::Counter => is_counter,
            Self::OtherUnit(filter) => {
                match other_unit_data {
                    Some(other_unit_data) => filter.iter()
                        .all(|f| f.check(game, other_unit_data, Some(unit_data), heroes, is_counter)),
                    None => false,
                }
                
            }
            Self::Not(negated) => {
                // returns true if at least one check returns false
                // if you need all checks to return false, put them into separate Self::Not wrappers instead
                negated.iter()
                .any(|negated| !negated.check(game, unit_data, other_unit_data, heroes, is_counter))
            }
        }
    }
}

pub(crate) fn unit_filter_scope<D: Direction>(
    game: &impl GameView<D>,
    unit_data: UnitData<D>,
    other_unit_data: Option<UnitData<D>>,
    _heroes: &HeroMap<D>,
    // true only during counter-attacks
    is_counter: bool,
) -> Scope<'static> {
    let mut scope = Scope::new();
    scope.push_constant(CONST_NAME_UNIT, unit_data.unit.clone());
    scope.push_constant(CONST_NAME_POSITION, unit_data.pos);
    scope.push_constant(CONST_NAME_TRANSPORT_INDEX, dyn_opt(unit_data.unload_index.map(|i| i as i32)));
    scope.push_constant(CONST_NAME_TRANSPORTER, dyn_opt(unit_data.original_transporter.map(|(u, _)| u.clone())));
    scope.push_constant(CONST_NAME_TRANSPORTER_POSITION, dyn_opt(unit_data.original_transporter.map(|(_, p)| p)));
    scope.push_constant(CONST_NAME_OTHER_UNIT, dyn_opt(other_unit_data.map(|ud| ud.unit.clone())));
    scope.push_constant(CONST_NAME_OTHER_POSITION, dyn_opt(other_unit_data.map(|ud| ud.pos)));
    //scope.push_constant(CONST_NAME_OTHER_TRANSPORT_INDEX, dyn_opt(other_unit_data.map(|ud| ud.unload_index.map(|i| i as i32))));
    //scope.push_constant(CONST_NAME_OTHER_TRANSPORTER, dyn_opt(other_unit_data.map(|ud| ud.original_transporter.map(|(u, _)| u.clone()))));
    //scope.push_constant(CONST_NAME_OTHER_TRANSPORTER_POSITION, dyn_opt(other_unit_data.map(|ud| ud.original_transporter.map(|(_, p)| p))));
    // TODO: heroes and ballast (put them into Arc<>s ?)
    scope.push_constant(CONST_NAME_IS_COUNTER, is_counter);
    scope.push_constant(CONST_NAME_OWNER_ID, game.current_owner() as i32);
    scope

}
