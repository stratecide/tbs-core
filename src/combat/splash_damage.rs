use executor::Executor;
use rhai::*;
use rustc_hash::FxHashSet;

use crate::config::file_loader::FileLoader;
use crate::config::parse::FromConfig;
use crate::config::ConfigParseError;
use crate::game::game_view::GameView;
use crate::map::direction::Direction;
use crate::map::map::*;
use crate::map::point::*;
use crate::map::wrapping_map::{Distortion, OrientedPoint};
use crate::script::*;
use crate::tags::{TagKey, TagValue};
use crate::units::movement::TBallast;
use crate::units::unit::*;

use super::AttackInput;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct SplashType(pub usize);

impl FromConfig for SplashType {
    fn from_conf<'a>(s: &'a str, loader: &mut crate::config::file_loader::FileLoader) -> Result<(Self, &'a str), crate::config::ConfigParseError> {
        let (base, s) = crate::config::parse::string_base(s);
        match loader.splash_types.iter().position(|name| name.as_str() == base) {
            Some(i) => Ok((Self(i), s)),
            None => Err(crate::config::ConfigParseError::UnknownEnumMember(format!("SplashType::{base}")))
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SplashPattern {
    pub points: SplashDamagePointSource,
    pub directions: SplashDamageDirectionSource,
}

impl SplashPattern {
    pub fn get_splash<D: Direction>(
        &self,
        game: &impl GameView<D>,
        attacker: &Unit<D>,
        temporary_ballast: &[TBallast<D>],
        possible_attack_targets: &Vec<Vec<OrientedPoint<D>>>,
        input: AttackInput<D>,
        range: usize,
    ) -> Vec<Vec<OrientedPoint<D>>> {
        match input {
            AttackInput::AttackPattern(_, _) => {
                debug_assert_eq!(self.points, SplashDamagePointSource::AttackPattern);
                possible_attack_targets.clone()
            }
            AttackInput::SplashPattern(input) => {
                let mut result = Vec::with_capacity(range + 1);
                let mut avoid_duplicates = Vec::with_capacity(range + 1);
                for _ in 0..=range {
                    result.push(Vec::new());
                    avoid_duplicates.push(FxHashSet::default());
                }
                result[0].push(input);
                for d in self.directions.get_dirs(attacker, input) {
                    self.points.add_splash_in_direction(game, input, range, |i, p, distortion| {
                        let dp = OrientedPoint::new(p, distortion.is_mirrored(), distortion.update_direction(d));
                        if avoid_duplicates[i].insert(dp) {
                            result[i].push(dp);
                        }
                    });
                }
                result
            }
        }
    }
}

// TODO: ensure that min and max are >0 in config and min < max
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SplashDamagePointSource {
    AttackPattern,          // uses the attack's attack pattern instead
    Straight,               // uses get_line, can get blocked by units that stand in the way
    TriangleDiagonal,       // uses range_in_layers
    TriangleStraight,       // uses cannon_range_in_layers
    Rhai(usize),
}

impl SplashDamagePointSource {
    fn add_splash_in_direction<
        D: Direction,
    >(&self, game: &impl GameView<D>, main_target: OrientedPoint<D>, range: usize, mut add: impl FnMut(usize, Point, Distortion<D>)) {
        match self {
            Self::AttackPattern => {
                panic!("Shouldn't call SplashDamagePointSource::AttackPattern.add_splash_in_direction");
            }
            Self::Straight => {
                let points = get_line(game, main_target.point, main_target.direction, range, NeighborMode::FollowPipes);
                for (i, (p, distortion)) in points.into_iter().enumerate().skip(1) {
                    add(i, p, distortion);
                }
            }
            Self::TriangleDiagonal => {
                let layers = range_in_layers(game, main_target.point, range, &[main_target.direction]);
                for (i, layer) in layers.into_iter().enumerate().skip(1) {
                    for (p, distortion) in layer {
                        add(i, p, distortion);
                    }
                }
            }
            Self::TriangleStraight => {
                let layers = cannon_range_in_layers(game, main_target.point, range, &[main_target.direction]);
                for (i, layer) in layers.into_iter().enumerate().skip(1) {
                    for (p, distortion) in layer {
                        add(i, p, distortion);
                    }
                }
            }
            Self::Rhai(function_index) => {
                let environment = game.environment();
                let mut scope = Scope::new();
                scope.push_constant(CONST_NAME_POSITION, main_target.point);
                scope.push_constant(CONST_NAME_ATTACK_DIRECTION, main_target.direction);
                scope.push_constant(CONST_NAME_MIRRORED, main_target.mirrored);
                let engine = environment.get_engine_board(game);
                let executor = Executor::new(engine, scope, environment);
                match executor.run::<Array>(*function_index, ()) {
                    Ok(lists) => {
                        // Array -> Vec<Vec<PointWithDistortion>>
                        for (i, list) in lists.into_iter().enumerate().take(range) {
                            if let Some(list) = list.try_cast::<Array>() {
                                for p in list {
                                    if let Some(p) = p.try_cast::<PointWithDistortion<D>>() {
                                        add(i + 1, p.point, p.distortion);
                                    }
                                }
                            }
                        }
                    }
                    Err(e) => {
                        // TODO: log error
                        println!("unit OnDeath {function_index}: {e:?}");
                    }
                }
            }
        }
    }
}

impl FromConfig for SplashDamagePointSource {
    fn from_conf<'a>(s: &'a str, loader: &mut FileLoader) -> Result<(Self, &'a str), ConfigParseError> {
        Ok((match s.trim() {
            "AttackPattern" => Self::AttackPattern,
            "Straight" => Self::Straight,
            "TriangleDiagonal" => Self::TriangleDiagonal,
            "TriangleStraight" => Self::TriangleStraight,
            s => Self::Rhai(loader.rhai_function(&format!("{s}"), 0..=0)?.index)
        }, ""))
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SplashDamageDirectionSource {
    AttackInput,
    AllDirections,
    UnitAttribute(TagKey),
    //Script(...)
}

impl SplashDamageDirectionSource {
    pub fn get_dirs<D: Direction>(&self, attacker: &Unit<D>, input: OrientedPoint<D>) -> Vec<D> {
        match self {
            Self::AttackInput => vec![input.direction],
            Self::AllDirections => D::list(),
            Self::UnitAttribute(tag) => {
                match attacker.get_tag(tag.0) {
                    Some(TagValue::Direction(d)) => vec![d],
                    _ => return Vec::new()
                }
            }
        }
    }
}

impl FromConfig for SplashDamageDirectionSource {
    fn from_conf<'a>(s: &'a str, loader: &mut FileLoader) -> Result<(Self, &'a str), ConfigParseError> {
        let (base, s) = crate::config::parse::string_base(s);
        Ok((match base {
            "AttackInput" => Self::AttackInput,
            "AllDirections" => Self::AllDirections,
            "UnitAttribute" => Self::UnitAttribute(TagKey::from_conf(s, loader)?.0),
            _ => return Err(crate::config::ConfigParseError::UnknownEnumMember(format!("SplashDamageDirectionSource::{}", base.to_string())))
        }, ""))
    }
}
