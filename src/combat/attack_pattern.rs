use num_rational::Rational32;
use rustc_hash::FxHashSet;
use rhai::{Array, Scope, Shared};
use zipper::*;
use zipper::zipper_derive::Zippable;

use crate::config::environment::Environment;
use crate::config::file_loader::FileLoader;
use crate::config::parse::{parse_inner_vec, parse_tuple1, parse_tuple2, string_base};
use crate::config::parse::FromConfig;
use crate::config::ConfigParseError;
use crate::game::game_view::GameView;
use crate::map::direction::Direction;
use crate::map::map::*;
use crate::map::point::*;
use crate::map::wrapping_map::{Distortion, OrientedPoint};
use crate::script::executor::Executor;
use crate::script::{CONST_NAME_ATTACK_DIRECTION, CONST_NAME_POSITION};
use crate::tags::{TagKey, TagValue};
use crate::units::hero::HeroMap;
use crate::units::movement::TBallast;
use crate::units::unit::*;

use super::{AttackCounterState, SplashDamagePointSource};

crate::enum_with_custom! {
    #[derive(Debug, Clone, PartialEq, Eq, Hash)]
    pub(crate) enum AttackPatternType {
        None,
        Adjacent,
        Straight,
        TriangleDiagonal,
        TriangleStraight,
    }
}

// TODO: ensure that min and max are >0 in config and min < max
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AttackPattern {
    None,
    Adjacent,                                                   // no range
    Straight{ min: Rational32, max: Rational32 },               // uses get_line, can get blocked by units that stand in the way
    TriangleDiagonal{ min: Rational32, max: Rational32 },       // uses range_in_layers
    TriangleStraight{ min: Rational32, max: Rational32 },       // uses cannon_range_in_layers, like BH cannons in advance wars
    Rhai{ function_index: usize, parameter_names: Shared<Vec<String>>, parameter_values: Vec<Rational32> },
}

impl FromConfig for AttackPattern {
    fn from_conf<'a>(s: &'a str, loader: &mut FileLoader) -> Result<(Self, &'a str), ConfigParseError> {
        let (base, mut remainder) = string_base(s);
        Ok((match base {
            "None" => Self::None,
            "Adjacent" => Self::Adjacent,
            s @ "Straight" | s @ "TriangleDiagonal" | s @ "TriangleStraight" => {
                let (min, max, r) = parse_tuple2::<Rational32, Rational32>(remainder, loader)?;
                remainder = r;
                match s {
                    "Straight" => Self::Straight{min, max},
                    "TriangleDiagonal" => Self::TriangleDiagonal{min, max},
                    "TriangleStraight" => Self::TriangleStraight{min, max},
                    _ => panic!("impossible AttackType error")
                }
            }
            script => {
                let (parameter_values, r) = parse_inner_vec::<Rational32>(remainder, false, loader)?;
                remainder = r;
                let parameter_count = parameter_values.len();
                let fun = loader.rhai_function(script, parameter_count..=parameter_count)?;
                Self::Rhai {
                    function_index: fun.index,
                    parameter_names: fun.parameters,
                    parameter_values,
                }
            }
            //invalid => return Err(ConfigParseError::UnknownEnumMember(invalid.to_string())),
        }, remainder))
    }
}

impl AttackPattern {
    pub(crate) const MIN_RANGE: &'static str = "MinRange";
    pub(crate) const MAX_RANGE: &'static str = "MaxRange";

    pub(crate) fn typ(&self, environment: &Environment) -> AttackPatternType {
        match self {
            Self::None => AttackPatternType::None,
            Self::Adjacent => AttackPatternType::Adjacent,
            Self::Straight {..} => AttackPatternType::Straight,
            Self::TriangleDiagonal {..} => AttackPatternType::TriangleDiagonal,
            Self::TriangleStraight {..} => AttackPatternType::TriangleStraight,
            Self::Rhai { function_index, .. } => AttackPatternType::Custom(environment.get_rhai_function_name(*function_index).clone())
        }
    }

    pub(crate) fn parameters(&mut self) -> Vec<(String, &mut Rational32)> {
        match self {
            Self::None |
            Self::Adjacent => Vec::new(),
            Self::Straight { min, max } |
            Self::TriangleDiagonal { min, max } |
            Self::TriangleStraight { min, max } => {
                vec![(Self::MIN_RANGE.to_string(), min), (Self::MAX_RANGE.to_string(), max)]
            }
            Self::Rhai { parameter_names, parameter_values, .. } => {
                parameter_names.iter().cloned().zip(parameter_values.iter_mut()).collect()
            }
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub enum AllowedAttackInputDirectionSource {
    AllDirections,
    Movement,
    UnitTag(TagKey),
    //Script(...)
}

impl FromConfig for AllowedAttackInputDirectionSource {
    fn from_conf<'a>(s: &'a str, loader: &mut FileLoader) -> Result<(Self, &'a str), ConfigParseError> {
        let (base, mut remainder) = string_base(s);
        Ok((match base {
            "AllDirections" => Self::AllDirections,
            "Movement" => Self::Movement,
            "Tag" => {
                let (tag, s) = parse_tuple1::<String>(remainder, loader)?;
                remainder = s;
                if let Some(i) = loader.tags.iter().position(|t| *t == tag) {
                    Self::UnitTag(TagKey(i))
                } else {
                    return Err(ConfigParseError::UnknownEnumMember(format!("Tag::{tag}")))
                }
            }
            //script if script.contains('>') => {...}
            _ => return Err(ConfigParseError::UnknownEnumMember(format!("AllowedAttackInputDirectionSource::{base}")))
        }, remainder))
    }
}

impl AllowedAttackInputDirectionSource {
    pub fn get_dirs<D: Direction>(&self, attacker: &Unit<D>, temporary_ballast: &[TBallast<D>]) -> Vec<D> {
        match self {
            Self::AllDirections => D::list(),
            Self::Movement => {
                let mut result = FxHashSet::default();
                for ballast in temporary_ballast {
                    match ballast {
                        TBallast::ForbiddenDirection(Some(d)) => {result.insert(d.opposite_direction());}
                        TBallast::Direction(d) => {result.extend(*d);}
                        TBallast::DiagonalDirection(d) => {result.extend(*d);}
                        _ => (),
                    }
                }
                let mut result: Vec<D> = result.into_iter().collect();
                result.sort_by_key(|d| d.list_index());
                result
            }
            Self::UnitTag(tag) => {
                match attacker.get_tag(tag.0) {
                    Some(TagValue::Direction(d)) => vec![d],
                    _ => return Vec::new()
                }
            }
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Zippable)]
#[zippable(bits=1, support_ref = Environment)]
pub enum AttackInput<D: Direction> {
    AttackPattern(Point, D),
    SplashPattern(OrientedPoint<D>),
}

impl<D: Direction> AttackInput<D> {
    pub fn target(&self) -> Point {
        match self {
            Self::AttackPattern(p, _) => *p,
            Self::SplashPattern(p) => p.point,
        }
    }

    pub fn splash_input(&self) -> Option<(D, bool)> {
        match self {
            Self::AttackPattern(_, _) => None,
            Self::SplashPattern(p) => Some((p.direction, p.mirrored)),
        }
    }

    pub fn attackable_positions(
        game: &impl GameView<D>,
        attacker: &Unit<D>,
        attacker_pos: Point,
        transporter: Option<(&Unit<D>, Point)>,
        temporary_ballast: &[TBallast<D>],
        heroes: &HeroMap<D>,
    ) -> Vec<Self> {
        let counter_state = AttackCounterState::NoCounter;
        let Some(attack) = attacker.environment().config.unit_configured_attacks(&*game, &attacker, attacker_pos, transporter, &counter_state, heroes, temporary_ballast).into_iter().next() else {
            return Vec::new();
        };
        let attack_pattern = attacker.attack_pattern(&*game, attacker_pos, &counter_state, &heroes, temporary_ballast);
        let allowed_directions = attacker.attack_pattern_directions(&*game, attacker_pos, &counter_state, &heroes, temporary_ballast);
        let allowed_directions = allowed_directions.get_dirs(&attacker, temporary_ballast);
        let mut layers = Vec::new();
        for d in allowed_directions.into_iter() {
            for (i, layer) in attack_pattern.possible_attack_targets(&*game, attacker_pos, d).into_iter().enumerate() {
                while i >= layers.len() {
                    layers.push(Vec::new());
                }
                layers[i].extend(layer.into_iter().map(|dp| {
                    match attack.splash_pattern.points {
                        SplashDamagePointSource::AttackPattern => AttackInput::AttackPattern(dp.point, d),
                        _ => AttackInput::SplashPattern(dp)
                    }
                }));
            }
        }
        let mut avoid_duplicates = FxHashSet::default();
        let mut result = Vec::new();
        for layer in layers {
            for input in layer {
                if avoid_duplicates.insert(input) {
                    result.push(input);
                }
            }
        }
        result
    }
}

impl AttackPattern {
    /**
     * Assumes that self's min and max ranges are pre-modified according to unit_powered.csv
     */
    pub fn possible_attack_targets<D: Direction>(&self, game: &impl GameView<D>, attacker_pos: Point, d: D) -> Vec<Vec<OrientedPoint<D>>> {
        let mut avoid_duplicates = Vec::new();
        let mut result = Vec::new();
        let mut add = |distance: usize, p: Point, distortion: Distortion<D>| {
            while distance >= result.len() {
                avoid_duplicates.push(FxHashSet::default());
                result.push(Vec::new());
            }
            if avoid_duplicates[distance].insert((p, distortion)) {
                result[distance].push(OrientedPoint::new(p, distortion.is_mirrored(), distortion.update_direction(d)));
            }
        };
        match self {
            Self::None => (),
            Self::Adjacent => {
                if let Some((p, distortion)) = game.get_neighbor(attacker_pos, d) {
                    add(0, p, distortion);
                }
            }
            Self::Straight { min, max } => {
                let min = min.to_integer().max(0).min(50) as usize;
                let max = max.to_integer().max(0).min(50) as usize;
                if min > max {
                    return result;
                }
                let points = get_line(game, attacker_pos, d, max, NeighborMode::FollowPipes);
                for (i, (p, distortion)) in points.into_iter().enumerate() {
                    if i >= min {
                        add(i - min, p, distortion);
                    } else if i > 0 && game.get_unit(p).is_some() {
                        // unit stands in the way before min-range is reached
                        // this prevents a Self::Straight attack
                        break;
                    }
                }
            }
            Self::TriangleDiagonal { min, max } => {
                let min = min.to_integer().max(0).min(50) as usize;
                let max = max.to_integer().max(0).min(50) as usize;
                if min > max {
                    return result;
                }
                let layers = range_in_layers(game, attacker_pos, max, &[d]);
                for (i, layer) in layers.into_iter().skip(min).enumerate() {
                    for (p, distortion) in layer {
                        add(i, p, distortion);
                    }
                }
            }
            Self::TriangleStraight { min, max } => {
                let min = min.to_integer().max(0).min(50) as usize;
                let max = max.to_integer().max(0).min(50) as usize;
                if min > max {
                    return result;
                }
                let layers = cannon_range_in_layers(game, attacker_pos, max, &[d]);
                for (i, layer) in layers.into_iter().skip(min).enumerate() {
                    for (p, distortion) in layer {
                        add(i, p, distortion);
                    }
                }
            }
            Self::Rhai { function_index, parameter_values, .. } => {
                let environment = game.environment();
                let mut scope = Scope::new();
                scope.push_constant(CONST_NAME_POSITION, attacker_pos);
                scope.push_constant(CONST_NAME_ATTACK_DIRECTION, d);
                let engine = environment.get_engine_board(game);
                let executor = Executor::new(engine, scope, environment);
                match executor.run::<Array>(*function_index, parameter_values.clone()) {
                    Ok(list) => {
                        // Array -> Vec<Vec<PointWithDistortion>>
                        for (i, p) in list.into_iter().enumerate() {
                            let p = match p.try_cast_result::<PointWithDistortion<D>>() {
                                Ok(p) => {
                                    add(i, p.point, p.distortion);
                                    continue;
                                }
                                Err(p) => p,
                            };
                            if let Some(list) = p.try_cast::<Array>() {
                                for p in list {
                                    if let Some(p) = p.try_cast::<PointWithDistortion<D>>() {
                                        add(i, p.point, p.distortion);
                                    }
                                }
                            }
                        }
                    }
                    Err(e) => {
                        let environment = game.environment();
                        environment.log_rhai_error("AttackPattern::Rhai", environment.get_rhai_function_name(*function_index), &e);
                    }
                }
            }
        }
        result
    }
}
