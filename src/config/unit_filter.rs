use std::collections::HashSet;

use crate::commander::commander_type::CommanderType;
use crate::game::fog::FogIntensity;
use crate::game::game_view::GameView;
use crate::map::point::Point;
use crate::script::executor::Executor;
use crate::tags::FlagKey;
use crate::terrain::TerrainType;
use crate::tokens::token_types::TokenType;
use crate::units::combat::AttackTypeKey;
use crate::units::hero::{HeroInfluence, HeroType};
use crate::units::movement::{MovementType, TBallast};
use crate::units::unit::Unit;
use crate::units::unit_types::UnitType;
use crate::map::direction::Direction;

use super::file_loader::FileLoader;
use super::movement_type_config::MovementPattern;
use super::parse::{parse_inner_vec, parse_inner_vec_dyn, parse_tuple1, parse_tuple2, string_base, FromConfig};
use super::ConfigParseError;
use super::config::Config;



#[derive(Debug, Clone)]
pub(super) enum UnitTypeFilter {
    Unit(HashSet<UnitType>),
    MovementPattern(HashSet<MovementPattern>),
}

impl FromConfig for UnitTypeFilter {
    fn from_conf<'a>(s: &'a str, loader: &mut FileLoader) -> Result<(Self, &'a str), ConfigParseError> {
        let (base, mut remainder) = string_base(s);
        Ok((match base {
            "U" | "Unit" => {
                let (list, r) = parse_inner_vec::<UnitType>(remainder, true, loader)?;
                remainder = r;
                Self::Unit(list.into_iter().collect())
            }
            "MP" | "MovementPattern" => {
                let (list, r) = parse_inner_vec::<MovementPattern>(remainder, true, loader)?;
                remainder = r;
                Self::MovementPattern(list.into_iter().collect())
            }
            _ => return Err(ConfigParseError::UnknownEnumMember(s.to_string()))
        }, remainder))
    }
}

impl UnitTypeFilter {
    pub fn check(&self, config: &Config, unit_type: UnitType) -> bool {
        match self {
            Self::Unit(u) => u.contains(&unit_type),
            Self::MovementPattern(m) => m.contains(&config.movement_pattern(unit_type)),
        }
    }
}

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
    // this unit
    Unit(HashSet<UnitType>),
    Flag(HashSet<FlagKey>),
    Movement(HashSet<MovementType>),
    SubMovement(HashSet<MovementType>),
    MovementPattern(HashSet<MovementPattern>),
    AttackType(HashSet<AttackTypeKey>),
    Unowned,
    // situation/environment
    Counter,
    Terrain(HashSet<TerrainType>),
    Token(HashSet<TokenType>),
    Fog(HashSet<FogIntensity>),
    // recursive
    Not(Vec<Self>),
    // replace with Rhai
    Moved,
    /*Hp(u8),
    Status(HashSet<ActionStatus>),
    Sludge,
    TerrainOwner,
    Level(u8),*/
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
            "A" | "AttackType" => {
                let (list, r) = parse_inner_vec::<AttackTypeKey>(remainder, true, loader)?;
                remainder = r;
                Self::AttackType(list.into_iter().collect())
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
            /*"S" | "Status" => {
                let (list, r) = parse_inner_vec::<ActionStatus>(remainder, true, loader)?;
                remainder = r;
                Self::Status(list.into_iter().collect())
            }
            "Sludge" => Self::Sludge,*/
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
            /*"Hp" => {
                let (hp, r) = parse_tuple1(remainder, loader)?;
                remainder = r;
                Self::Hp(hp)
            }
            "TerrainOwner" => Self::TerrainOwner,*/
            "Counter" => Self::Counter,
            /*"Level" => {
                let (level, r) = parse_tuple1(remainder, loader)?;
                remainder = r;
                Self::Level(level)
            }*/
            "Not" => {
                let (list, r) = parse_inner_vec_dyn::<Self>(remainder, true, |s| Self::from_conf(s, loader))?;
                remainder = r;
                Self::Not(list)
            }
            invalid => return Err(ConfigParseError::UnknownEnumMember(invalid.to_string())),
        }, remainder))
    }
}

impl UnitFilter {
    pub fn check<D: Direction>(
        &self,
        game: &impl GameView<D>,
        unit: &Unit<D>,
        unit_pos: (Point, Option<usize>),
        // when moving out of a transporter, or start_turn for transported units
        transporter: Option<(&Unit<D>, Point)>,
        // the attacked unit, the unit this one was destroyed by, ...
        other_unit: Option<(&Unit<D>, Point)>,
        // the heroes affecting this unit. shouldn't be taken from game since they could have died before this function is called
        heroes: &[HeroInfluence<D>],
        // empty if the unit hasn't moved
        temporary_ballast: &[TBallast<D>],
        // true only during counter-attacks
        is_counter: bool,
        executor: &Executor,
    ) -> bool {
        match self {
            Self::Rhai(function_index) => {
                match executor.run(*function_index, ()) {
                    Ok(result) => result,
                    Err(e) => {
                        // TODO: log error
                        println!("UnitFilter error: {e:?}");
                        false
                    }
                }
            }
            Self::Unit(u) => u.contains(&unit.typ()),
            Self::Flag(flags) => flags.iter().any(|flag| unit.has_flag(flag.0)),
            Self::Movement(m) => m.contains(&unit.base_movement_type()),
            Self::SubMovement(m) => m.contains(&unit.sub_movement_type()),
            Self::Terrain(t) => t.contains(&game.get_terrain(unit_pos.0).unwrap().typ()),
            Self::Token(t) => {
                for token in game.get_tokens(unit_pos.0) {
                    if t.contains(&token.typ()) {
                        return true;
                    }
                }
                false
            }
            Self::MovementPattern(m) => m.contains(&unit.movement_pattern()),
            Self::Hero(h) => {
                for (_, hero, _, _, _) in heroes {
                    let power = hero.get_active_power() as u8;
                    if h.iter().any(|h| h.0 == hero.typ() && h.1.unwrap_or(power) == power) {
                        return true;
                    }
                }
                false
            }
            Self::HeroGlobal(h) => {
                for p in game.all_points() {
                    if let Some(hero) = game.get_unit(p)
                    .filter(|u| u.get_owner_id() == unit.get_owner_id())
                    .and_then(|u| u.get_hero().cloned()) {
                        let power = hero.get_active_power() as u8;
                        let hero = hero.typ();
                        if h.iter().any(|h| h.0 == hero && h.1.unwrap_or(power) == power) {
                            return true;
                        }
                    }
                }
                false
            }
            Self::IsHero(h) => {
                if let Some(hero) = unit.get_hero() {
                    let power = hero.get_active_power() as u8;
                    let hero = hero.typ();
                    h.len() == 0 && hero != HeroType::None
                    || h.iter().any(|h| h.0 == hero && h.1.unwrap_or(power) == power)
                } else {
                    false
                }
            }
            Self::AttackType(a) => {
                let attack_type = game.environment().config.default_attack_pattern(unit.typ()).key();
                a.iter().any(|a| *a == attack_type)
            }
            Self::CommanderCharge(charge) => {
                unit.get_commander(game).get_charge() >= *charge
            }
            Self::Fog(f) => {
                let fog = game.get_fog_setting().intensity();
                f.iter().any(|f| *f == fog)
            }
            Self::Moved => {
                temporary_ballast.len() > 0
            }
            Self::Unowned => unit.get_owner_id() < 0,
            /*Self::Status(status) => {
                let s = unit.get_status();
                status.iter().any(|a| *a == s)
            }
            Self::Sludge => {
                game.get_details(unit_pos.0).iter()
                .any(|d| match d {
                    Detail::SludgeToken(_) => true,
                    _ => false
                })
            }*/
            Self::Commander(commander_type, power) => {
                let commander = unit.get_commander(game);
                commander.typ() == *commander_type
                && (power.is_none() || power.clone().unwrap() as usize == commander.get_active_power())
            }
            /*Self::Hp(hp) => unit.get_hp() >= *hp,
            Self::TerrainOwner => {
                game.get_terrain(unit_pos.0).unwrap().get_owner_id() == unit.get_owner_id()
            }*/
            Self::Counter => is_counter,
            //Self::Level(level) => unit.get_level() >= *level,
            Self::Not(negated) => {
                // returns true if at least one check returns false
                // if you need all checks to return false, put them into separate Self::Not wrappers instead
                negated.iter()
                .any(|negated| !negated.check(game, unit, unit_pos, transporter, other_unit, heroes, temporary_ballast, is_counter, executor))
            }
        }
    }
}
