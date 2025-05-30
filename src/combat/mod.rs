use rustc_hash::{FxHashMap, FxHashSet};

mod attack_pattern;
mod attack;
pub mod rhai_combat;
mod splash_damage;
#[cfg(test)]
mod test;

pub use attack_pattern::*;
pub use attack::*;
pub use splash_damage::*;

use crate::config::unit_filter::unit_filter_scope;
use crate::config::file_loader::FileLoader;
use crate::config::parse::FromConfig;
use crate::config::ConfigParseError;
use crate::game::event_handler::EventHandler;
use crate::game::game_view::GameView;
use crate::map::direction::Direction;
use crate::map::point::*;
use crate::map::wrapping_map::OrientedPoint;
use crate::script::executor::Executor;
use crate::units::hero::HeroMap;
use crate::units::movement::TBallast;
use crate::units::unit::Unit;
use crate::units::{UnitData, UnitId};

/*
 * TO CONSIDER
 * -----------
 * units can attack and counter-attack multiple times
 * attack and counter-attack order can be arbitrary
 * pull/push are treated like attacks
 * it's possible that the same target can be reached with different directions
 * should an attack be possible if no direct target exists, only via splash damage?
 * when can a unit hit by splash damage counter attack?
 * can a counter-attack be blocked by individual attacks or only for the whole combat?
 * 
 * for the craziest effects, allow disabling 'normal' attacks so they can be replaced with custom actions
 * possibly allow setting a "default"-flag for the first custom action so this feels like a 'normal' attack
*/

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AttackCounterState<D: Direction> {
    RealCounter {
        id: usize,
        unit: Unit<D>,
        pos: Point,
        ballast: Vec<TBallast<D>>,
        original_transporter: Option<(Unit<D>, Point)>,
    },
    FakeCounter,
    AllowCounter,
    NoCounter,
}
impl<D: Direction> AttackCounterState<D> {
    pub fn allows_counter(&self) -> bool {
        *self == Self::AllowCounter
    }

    pub fn is_counter(&self) -> bool {
        match self {
            Self::RealCounter{..} |
            Self::FakeCounter => true,
            _ => false
        }
    }

    pub fn attacker(&self) -> Option<UnitData<D>> {
        match self {
            Self::RealCounter { unit, pos, ballast, original_transporter, .. } => Some(UnitData {
                unit,
                pos: *pos,
                unload_index: None,
                ballast: &ballast,
                original_transporter: original_transporter.as_ref().map(|(u, p)| (u, *p)),
            }),
            _ => None
        }
    }
}

#[derive(Clone)]
pub enum AttackerPosition<D: Direction> {
    Ghost(Point, Unit<D>),
    Real(UnitId<D>),
}

impl<D: Direction> AttackerPosition<D> {
    fn get_position(&self, handler: &EventHandler<D>) -> Option<(Point, Option<usize>)> {
        match self {
            Self::Ghost(p, _) => Some((*p, None)),
            Self::Real(id) => handler.get_observed_unit_pos(id.0),
        }
    }

    fn get_unit(&self, handler: &EventHandler<D>) -> Option<Unit<D>> {
        match self {
            Self::Ghost(_, unit) => Some(unit.clone()),
            Self::Real(id) => handler.get_observed_unit_pos(id.0).map(|(p, unload_index)| {
                let unit = handler.get_game().get_unit(p).unwrap();
                match unload_index {
                    Some(i) => unit.get_transported()[i].clone(),
                    None => unit,
                }
            }),
        }
    }
}

#[derive(Clone)]
pub struct AttackTargeting<D: Direction> {
    pub target: OrientedPoint<D>,
    pub direction_hint: D,
    pub unit_id: Option<usize>,
}

crate::listable_enum! {
    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub enum AttackTargetingFocus {
        Unit,               // re-targets to attack the unit found by this id
        Position,           // tries to hit the selected position, even if no unit is there anymore
        Relative,           // tries to attack in the same direction, even if the attacker was displaced
    }
}

#[derive(Clone)]
struct AttackerInfo<'a, D: Direction> {
    pub attacker_position: AttackerPosition<D>,
    pub targeting: AttackTargeting<D>,
    pub transporter: Option<(&'a Unit<D>, Point)>,
    pub temporary_ballast: &'a [TBallast<D>],
    pub counter_state: AttackCounterState<D>,
}

impl<'a, D: Direction> AttackerInfo<'a, D> {
    pub fn retarget(
        &'a self,
        handler: &EventHandler<D>,
        attack: &ConfiguredAttack,
        heroes: &HeroMap<D>,
    ) -> Option<(AttackInput<D>, D, Vec<Vec<OrientedPoint<D>>>)> {
        let attacker = self.attacker_position.get_unit(handler)?;
        let attacker_pos = self.attacker_position.get_position(handler)?.0;
        let game = handler.get_game();
        let pattern = attacker.attack_pattern(&*game, attacker_pos, &self.counter_state, heroes, self.temporary_ballast);
        let allowed_directions = attacker.attack_pattern_directions(&*game, attacker_pos, &self.counter_state, heroes, self.temporary_ballast);
        let mut allowed_directions = allowed_directions.get_dirs(&attacker, self.temporary_ballast);
        let mut direction_hint = self.targeting.direction_hint;
        if let AttackerPosition::Real(UnitId(id, distortion)) = self.attacker_position {
            let current_distortion = handler.get_observed_unit(id)?.2;
            direction_hint = (-distortion + current_distortion).update_direction(direction_hint);
        }
        if let Some(i) = allowed_directions.iter().position(|d| *d == direction_hint) {
            allowed_directions.remove(i);
            allowed_directions.insert(0, direction_hint);
        }
        let target = match attack.focus {
            AttackTargetingFocus::Unit => {
                if let Some((p, _)) = handler.get_observed_unit_pos(self.targeting.unit_id?) {
                    Some(p)
                } else {
                    return None;
                }
            }
            AttackTargetingFocus::Position => Some(self.targeting.target.point),
            AttackTargetingFocus::Relative => {
                if !allowed_directions.contains(&direction_hint) {
                    return None;
                }
                allowed_directions = vec![direction_hint];
                None
            }
        };
        let mut result = None;
        let possible_attack_targets: Vec<_> = allowed_directions.into_iter()
            .map(|d| (d, pattern.possible_attack_targets(&*game, attacker_pos, d)))
            .collect();
        // prefer attacks at minimum range
        let max_range = possible_attack_targets.iter().map(|(_, layers)| layers.len()).max()?;
        'outer: for range in 0..max_range {
            for (d, layers) in &possible_attack_targets {
                let Some(layer) = layers.get(range) else {
                    continue;
                };
                for dp in layer.iter().filter(|dp| target.is_none() || target == Some(dp.point)) {
                    let attack_input = match attack.splash_pattern.points {
                        SplashDamagePointSource::AttackPattern => AttackInput::AttackPattern(dp.point, *d),
                        _ => AttackInput::SplashPattern(*dp)
                    };
                    if *d == direction_hint
                    && (attack.splash_pattern.points == SplashDamagePointSource::AttackPattern || self.targeting.target == *dp) {
                        // found a perfect match, return immediately
                        return Some((attack_input, *d, layers.clone()));
                    }
                    // TODO: check if dp is a better fit than the result so far instead of returning right away
                    if result.is_none() {
                        result = Some((attack_input, *d, layers));
                        break 'outer;
                    }
                }
            }
        }
        result.map(|(dp, d, layers)| (dp, d, layers.clone()))
    }
}

pub fn execute_attack<D: Direction>(
    handler: &mut EventHandler<D>,
    attacker_position: AttackerPosition<D>,
    input: AttackInput<D>,
    transporter: Option<(&Unit<D>, Point)>,
    temporary_ballast: &[TBallast<D>],
    counter_state: AttackCounterState<D>,
    execute_scripts: bool,
) {
    let heroes = HeroMap::new(&*handler.get_game(), None);
    let attackers = {
        let attacker_pos = attacker_position.get_position(handler).unwrap().0;
        let attacker = attacker_position.get_unit(handler).unwrap();
        let unit_id = handler.get_game().get_visible_unit(handler.get_game().current_team(), input.target());
        let unit_id = unit_id.map(|_| handler.observe_unit(input.target(), None).0);
        let game = handler.get_game();
        let attack_pattern = attacker.attack_pattern(&*game, attacker_pos, &AttackCounterState::NoCounter, &heroes, temporary_ballast);
        let allowed_directions = attacker.attack_pattern_directions(&*game, attacker_pos, &counter_state, &heroes, temporary_ballast);
        let allowed_directions = allowed_directions.get_dirs(&attacker, temporary_ballast);
        if allowed_directions.len() == 0 {
            return;
        }
        let (target, direction_hint, attack_pattern) = match input {
            AttackInput::AttackPattern(point, d) => {
                if !allowed_directions.contains(&d) {
                    return;
                }
                let attack_pattern = attack_pattern.possible_attack_targets(&*game, attacker_pos, d);
                let Some(dp) = attack_pattern.iter()
                .flatten()
                .find(|dp| dp.point == point)
                .cloned() else {
                    return;
                };
                (dp, d, attack_pattern)
            }
            AttackInput::SplashPattern(dp) => {
                let mut patterns = allowed_directions.into_iter()
                    .map(|d| (d, attack_pattern.possible_attack_targets(&*game, attacker_pos, d)))
                    .filter_map(|(d, pattern)| {
                        for (i, layer) in pattern.iter().enumerate() {
                            if layer.contains(&dp) {
                                return Some((dp, d, pattern, i));
                            }
                        }
                        None
                    })
                    .collect::<Vec<_>>();
                if patterns.len() == 0 {
                    return;
                }
                patterns.sort_by_key(|(_, _, _, distance)| *distance);
                let pattern = patterns.swap_remove(0);
                (pattern.0, pattern.1, pattern.2)
            }
        };
        drop(game);
        let mut attackers: Vec<AttackerInfo<D>> = vec![AttackerInfo {
            attacker_position: attacker_position.clone(),
            targeting: AttackTargeting {
                target,
                direction_hint,
                unit_id,
            },
            transporter,
            temporary_ballast,
            counter_state: counter_state.clone(),
        }];
        match attacker_position {
            AttackerPosition::Real(id) if counter_state.allows_counter() => {
                // add all counter-attackers to attackers list
                let attacker_id = id;
                let counter_attackers = find_counter_attackers(&*handler.get_game(), &attacker, attacker_pos, &attack_pattern, input, transporter, temporary_ballast, &heroes);
                for (p, counter_direction_hint) in counter_attackers {
                    let unit_id = handler.observe_unit(p, None);
                    attackers.push(AttackerInfo {
                        attacker_position: AttackerPosition::Real(unit_id),
                        targeting: AttackTargeting {
                            target: OrientedPoint::new(attacker_pos, target.mirrored, direction_hint.opposite_direction()),
                            direction_hint: counter_direction_hint,
                            unit_id: Some(attacker_id.0),
                        },
                        transporter: None,
                        temporary_ballast: &[],
                        counter_state: AttackCounterState::RealCounter {
                            id: attacker_id.0,
                            unit: attacker.clone(),
                            pos: attacker_pos,
                            ballast: temporary_ballast.to_vec(),
                            original_transporter: transporter.map(|(u, p)| (u.clone(), p)),
                        },
                    });
                }
            }
            _ => (),
        }
        attackers
    };
    let game = handler.get_game();
    let mut attack_map: FxHashMap<i8, Vec<(AttackerInfo<D>, ConfiguredAttack)>> = FxHashMap::default();
    for attacker in attackers {
        let unit = attacker.attacker_position.get_unit(handler).unwrap();
        let pos = attacker.attacker_position.get_position(handler).unwrap().0;
        for attack in unit.environment().config.unit_configured_attacks(&*game, &unit, pos, attacker.transporter, &attacker.counter_state, &heroes, attacker.temporary_ballast) {
            let priority = attack.priority;
            let value = (attacker.clone(), attack);
            if let Some(list) = attack_map.get_mut(&priority) {
                list.push(value);
            } else {
                attack_map.insert(priority, vec![value]);
            }
        }
    }
    drop(game);
    let mut priorities: Vec<i8> = attack_map.keys().cloned().collect();
    priorities.sort_by(|a, b| b.cmp(a));
    while let Some(priority) = priorities.pop() {
        let attacks = attack_map.remove(&priority).unwrap();
        let scripted_attacks = execute_attacks_with_equal_priority(handler, attacks, execute_scripts);
        // on_defend scripts can add attackers. add them to the map here
        if scripted_attacks.len() > 0 {
            let game = handler.get_game();
            for atk in scripted_attacks {
                let Some((pos, None)) = atk.attacker.get_position(handler) else {
                    continue;
                };
                let Some((defender_pos, _)) = handler.get_observed_unit_pos(atk.defender_id.0) else {
                    continue;
                };
                let unit = atk.attacker.get_unit(handler).unwrap();
                let targeting = AttackTargeting {
                    target: OrientedPoint::simple(defender_pos, D::angle_0()),
                    direction_hint: D::angle_0(),
                    unit_id: Some(atk.defender_id.0),
                };
                let transporter = None;
                let temporary_ballast = &[];
                let counter_state = AttackCounterState::FakeCounter;
                for attack in unit.environment().config.unit_configured_attacks(&*game, &unit, pos, transporter, &counter_state, &heroes, temporary_ballast) {
                    let prio = attack.priority as i32 + atk.priority;
                    if prio <= priority as i32 || prio > i8::MAX as i32 {
                        // don't add attack instances that should have happened in the past
                        continue;
                    }
                    let priority = prio as i8;
                    let value = (AttackerInfo {
                        attacker_position: atk.attacker.clone(),
                        targeting: targeting.clone(),
                        transporter,
                        temporary_ballast,
                        counter_state: counter_state.clone(),
                    }, attack);
                    if let Some(list) = attack_map.get_mut(&priority) {
                        list.push(value);
                    } else {
                        attack_map.insert(priority, vec![value]);
                    }
                }
            }
            priorities = attack_map.keys().cloned().collect();
            priorities.sort_by(|a, b| b.cmp(a));
        }
    }
}

fn find_counter_attackers<D: Direction>(
    game: &impl GameView<D>,
    attacker: &Unit<D>,
    attacker_pos: Point,
    attack_pattern: &Vec<Vec<OrientedPoint<D>>>,
    target: AttackInput<D>,
    transporter: Option<(&Unit<D>, Point)>,
    temporary_ballast: &[TBallast<D>],
    heroes: &HeroMap<D>,
) -> Vec<(Point, D)> {
    let configured_attacks = attacker.environment().config.unit_configured_attacks(game, attacker, attacker_pos, transporter, &AttackCounterState::NoCounter, heroes, temporary_ballast);
    let mut checked = FxHashSet::default();
    let mut result = Vec::new();
    let attacker_data = UnitData {
        unit: attacker,
        pos: attacker_pos,
        unload_index: None,
        ballast: temporary_ballast,
        original_transporter: transporter,
    };
    for attack in &configured_attacks {
        // don't need to consider splashes that don't allow counter attacks
        let Some(splash_range) = attack.splash.iter().filter(|a| a.allows_counter_attack).map(|a| a.splash_distance).max() else {
            continue;
        };
        let ranges: Vec<Vec<OrientedPoint<D>>> = attack.splash_pattern.get_splash(game, attacker, temporary_ballast, attack_pattern, target, splash_range);
        for dp in ranges.into_iter()
        .enumerate()
        .filter(|(i, _)| attack.splash.iter().any(|a| a.allows_counter_attack && a.splash_distance == *i)) // skip ranges where counter-attack isn't allowed
        .map(|(_, range)| range)
        .flatten() {
            if checked.insert(dp.point) {
                if let Some(unit) = game.get_unit(dp.point).filter(|u| u.get_team() != attacker.get_team()) {
                    if unit.can_target(game, dp.point, None, attacker_data, true, &heroes) {
                        // could reverse SplashDirectionModifier for direction_hint here
                        result.push((dp.point, dp.direction.opposite_direction()));
                    }
                }
            }
        }
    }
    result
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ValidAttackTargets {
    Enemy,
    Friendly,
    All,
    Rhai(usize),
}

impl FromConfig for ValidAttackTargets {
    fn from_conf<'a>(s: &'a str, loader: &mut FileLoader) -> Result<(Self, &'a str), ConfigParseError> {
        Ok((match s.trim() {
            "" => Self::Enemy,
            "Enemy" => Self::Enemy,
            "Friendly" => Self::Friendly,
            "All" => Self::All,
            name => {
                Self::Rhai(loader.rhai_function(&name, 0..=0)?.index)
            }
        }, ""))
    }
}

impl ValidAttackTargets {
    pub fn check<D: Direction>(
        &self,
        game: &impl GameView<D>,
        unit_data: UnitData<D>,
        other_unit_data: UnitData<D>,
        heroes: &HeroMap<D>,
        // true only during counter-attacks
        is_counter: bool,
    ) -> bool {
        match self {
            Self::Enemy => unit_data.unit.get_team() != other_unit_data.unit.get_team(),
            Self::Friendly => unit_data.unit.get_team() == other_unit_data.unit.get_team(),
            Self::All => true,
            Self::Rhai(function_index) => {
                let environment = game.environment();
                let engine = environment.get_engine_board(game);
                let executor = Executor::new(engine, unit_filter_scope(game, unit_data, Some(other_unit_data), heroes, is_counter), environment);
                match executor.run(*function_index, ()) {
                    Ok(result) => result,
                    Err(e) => {
                        let environment = game.environment();
                        environment.log_rhai_error("ValidAttackTargets::Rhai", environment.get_rhai_function_name(*function_index), &e);
                        false
                    }
                }
            }
        }
    }
}
