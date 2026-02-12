use std::rc::Rc;

use interfaces::{ClientPerspective, GameInterface};
use num_rational::Rational32;
use rhai::*;

use crate::combat::rhai_combat::AttackContext;
use crate::config::file_loader::FileLoader;
use crate::config::parse::FromConfig;
use crate::config::ConfigParseError;
use crate::game::commands::cleanup_dead_material;
use crate::game::event_handler::EventHandler;
use crate::game::fog::get_visible_unit;
use crate::map::board::BoardView;
use crate::map::direction::Direction;
use crate::map::map::{get_line, NeighborMode};
use crate::map::point::*;
use crate::map::wrapping_map::OrientedPoint;
use crate::{dyn_opt, script::*};
use crate::units::hero::{HeroMap, HeroMapWithId};
use crate::units::movement::{Path, PathStep, TBallast};
use crate::units::{unit::*, UnitData, UnitId};

use super::{AttackCounterState, AttackTargetingFocus, AttackerInfo, AttackerPosition, SplashPattern};

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct AttackType(pub Option<usize>);

impl FromConfig for AttackType {
    fn from_conf<'a>(s: &'a str, loader: &mut crate::config::file_loader::FileLoader) -> Result<(Self, &'a str), crate::config::ConfigParseError> {
        let (base, s) = crate::config::parse::string_base(s);
        if base == "None" {
            return Ok((Self(None), s));
        }
        match loader.attack_types.iter().position(|name| name.as_str() == base) {
            Some(i) => Ok((Self(Some(i)), s)),
            None => Err(crate::config::ConfigParseError::UnknownEnumMember(format!("AttackType::{base}")))
        }
    }
}

impl AttackType {
    pub(crate) fn parse_new<'a>(s: &'a str, loader: &mut crate::config::file_loader::FileLoader) -> Result<Self, crate::config::ConfigParseError> {
        if s.len() == 0 {
            return Err(crate::config::ConfigParseError::NameTooShort.into());
        }
        if s == "None" {
            return Err(crate::config::ConfigParseError::InvalidColumnValue("AttackType".to_string(), "None".to_string()).into());
        }
        Ok(match loader.attack_types.iter().position(|name| name == s) {
            Some(i) => AttackType(Some(i)),
            None => {
                loader.attack_types.push(s.to_string());
                AttackType(Some(loader.attack_types.len() - 1))
            }
        })
    }
}

#[derive(Clone)]
pub struct ConfiguredAttack {
    pub typ: AttackType,
    pub splash_pattern: SplashPattern,
    pub splash_range: u8,
    pub priority: i8,
    pub splash: Vec<AttackInstance>,
    pub focus: AttackTargetingFocus,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DisplaceDirectionModifier {
    Keep,
    Reverse,
    SharpLeft,      // move target to the left and closer to the attacker if hex,same as BluntLeft if square
    BluntLeft,      // move target to the left and away from the attacker if hex,same as SharpLeft if square
    SharpRight,
    BluntRight,
}

impl DisplaceDirectionModifier {
    fn modify<D: Direction>(&self, dp: &OrientedPoint<D>) -> OrientedPoint<D> {
        let mut result = dp.clone();
        match self {
            Self::Keep => (),
            Self::Reverse => result.direction = result.direction.opposite_direction(),
            Self::SharpLeft if D::is_hex() => result.direction = result.direction.rotate_times(dp.mirrored, 2),
            Self::BluntLeft if D::is_hex() => result.direction = result.direction.rotate_times(dp.mirrored, 1),
            Self::SharpRight if D::is_hex() => result.direction = result.direction.rotate_times(!dp.mirrored, 2),
            Self::BluntRight if D::is_hex() => result.direction = result.direction.rotate_times(!dp.mirrored, 1),
            Self::SharpLeft |
            Self::BluntLeft => result.direction = result.direction.rotate(dp.mirrored),
            Self::SharpRight |
            Self::BluntRight => result.direction = result.direction.rotate(!dp.mirrored),
        }
        result
    }
}

impl FromConfig for DisplaceDirectionModifier {
    fn from_conf<'a>(s: &'a str, _loader: &mut FileLoader) -> Result<(Self, &'a str), ConfigParseError> {
        Ok((match s.trim() {
            "" | "Keep" => Self::Keep,
            "Reverse" => Self::Reverse,
            "SharpLeft" => Self::SharpLeft,
            "BluntLeft" => Self::BluntLeft,
            "SharpRight" => Self::SharpRight,
            "BluntRight" => Self::BluntRight,
            _ => return Err(crate::config::ConfigParseError::UnknownEnumMember(format!("DisplaceDirectionModifier::{}", s.to_string())))
        }, ""))
    }
}

#[derive(Clone)]
pub struct AttackInstance {
    pub splash_distance: usize,                     // main target is 0
    pub allows_counter_attack: bool,
    // an attack is possible if there's a valid target at any AttackInstance with can_be_targeted == true
    //pub can_be_targeted: bool,                      // always true for the main target
    pub priority: Rational32,
    //direction_source: DisplaceDirectionSource,
    pub direction_modifier: DisplaceDirectionModifier,
    pub script: AttackInstanceScript,
}

#[derive(Debug, Clone, Copy)]
pub enum AttackInstanceScript {
    /*DealDamage {
        damage: NumberMod<Rational32>,
    },*/
    Displace {
        distance: Rational32,                 // how far (in tiles) the defender gets pushed
        push_limit: Rational32,               // how many additional units can be displaced if there are units in the way
        throw: bool,                          // the displaced unit skips over units that stand in the way. push_limit is ignored if true
        neighbor_mode: NeighborMode,
    },
    Rhai {
        build_script: usize,
    },
}

#[derive(Debug, Clone, Copy)]
struct PushArguments<D: Direction> {
    direction: D,
    distance: usize,
    push_limit: usize,
}

impl AttackInstance {
    pub(crate) fn into_executable<D: Direction>(
        &self,
        handler: &mut EventHandler<D>,
        attack: &ConfiguredAttack,
        splash: &AttackInstance,
        attacker_pos: &AttackerPosition<D>,
        attack_direction: D,
        targets: &[OrientedPoint<D>],
        heroes: &HeroMap<D>,
        heroes_with_ids: &HeroMapWithId<D>,
        temporary_ballast: &[TBallast<D>],
        counter_state: &AttackCounterState<D>,
    ) -> Vec<AttackExecutable<D>> {
        let attacker_id = match attacker_pos {
            AttackerPosition::Real(id) => Some(*id),
            _ => None,
        };
        let Some(attacker) = attacker_pos.get_unit(handler) else {
            return Vec::new();
        };
        let Some((attacker_pos, _attacker_unload_index)) = attacker_pos.get_position(handler) else {
            return Vec::new();
        };
        let is_counter = counter_state.is_counter();
        let environment = handler.environment().clone();
        let result: Vec<AttackContextScript<D>>;
        match &self.script {
            AttackInstanceScript::Displace { distance, push_limit, throw, neighbor_mode } => {
                result = targets.into_iter()
                .filter_map(|dp| {
                    let defender = handler.get_game().get_unit(dp.point)?.clone();
                    let defender_id = handler.observe_unit(dp.point, None);
                    let defender_data = UnitData {
                        unit: &defender,
                        pos: dp.point,
                        unload_index: None,
                        ballast: &[], // TODO: should have a value if counter-attack
                        original_transporter: None, // TODO: should have a value if counter-attack
                    };
                    let distance: i32 = environment.config.unit_attack_bonus(&"PushDistance".to_string(), *distance, handler.get_board(), attack, splash, &attacker, attacker_pos, defender_data, heroes, temporary_ballast, counter_state.is_counter()).to_integer();
                    let push_limit: i32 = environment.config.unit_attack_bonus(&"PushLimit".to_string(), *push_limit, handler.get_board(), attack, splash, &attacker, attacker_pos, defender_data, heroes, temporary_ballast, counter_state.is_counter()).to_integer();
                    if distance <= 0 || push_limit < 0 {
                        return None;
                    }
                    Some(AttackContextScript {
                        defender_id: Some(defender_id),
                        priority: None,
                        script: AttackExecutableScript::Displace { throw: *throw, neighbor_mode: *neighbor_mode },
                        arguments: vec![Dynamic::from(PushArguments {
                            direction: (-defender_id.1).update_direction(self.direction_modifier.modify(dp).direction),
                            distance: distance as usize,
                            push_limit: push_limit as usize,
                        })],
                    })
                }).collect();
            }
            AttackInstanceScript::Rhai { build_script } => {
                let (default_ast, _) = environment.get_rhai_function(*build_script);
                let mut attack_context = AttackContext::new(
                    handler,
                    attack,
                    splash,
                    &attacker,
                    attacker_pos,
                    temporary_ballast,
                    heroes,
                    counter_state,
                    default_ast.clone(),
                );
                let mut first_argument = Map::new();
                first_argument.insert(CONST_NAME_SPLASH_DISTANCE.into(), Dynamic::from(splash.splash_distance as i32));
                first_argument.insert(CONST_NAME_ATTACKER_ID.into(), attacker_id.map(|id| Dynamic::from(id)).unwrap_or(().into()));
                first_argument.insert(CONST_NAME_ATTACKER.into(), Dynamic::from(attacker.clone()));
                first_argument.insert(CONST_NAME_ATTACKER_POSITION.into(), Dynamic::from(attacker_pos));
                first_argument.insert(CONST_NAME_ATTACK_DIRECTION.into(), Dynamic::from(attack_direction));
                first_argument.insert(CONST_NAME_HEROES.into(), Dynamic::from(heroes_with_ids.clone()));
                first_argument.insert(CONST_NAME_TARGETS.into(), Dynamic::from(targets.into_iter()
                    .map(|dp| Dynamic::from(self.direction_modifier.modify(dp)))
                    .collect::<Array>()));
                let executor = attack_context.executor(first_argument);
                match executor.run::<D, ()>(*build_script, ()) {
                    Ok(()) => {
                        result = attack_context.scripts.drain(..).collect();
                    }
                    Err(e) => {
                        environment.log_rhai_error("AttackSplash preparation", environment.get_rhai_function_name(*build_script), &e);
                        attack_context.handler.effect_glitch();
                        return Vec::new();
                    }
                }
            }
        }
        result.into_iter()
            .map(|scr| {
                let (defender, defender_pos) = if let Some(id) = scr.defender_id {
                    let defender_pos = handler.get_observed_unit_pos(id.0).unwrap().0;
                    let defender = handler.get_board().get_unit(defender_pos).cloned();
                    (defender, Some(defender_pos))
                } else {
                    (None, None)
                };
                AttackExecutable {
                    priority: scr.priority.unwrap_or(self.priority),
                    attacker: attacker.clone(),
                    attacker_id,
                    attacker_pos,
                    defender_id: scr.defender_id,
                    defender,
                    defender_pos,
                    is_counter,
                    arguments: scr.arguments,
                    script: scr.script,
                }
            }).collect()
    }
}

// for use in rhai scripts
#[derive(Debug, Clone)]
pub(crate) struct AttackScript {
    pub function_name: ImmutableString,
    pub arguments: Array,
}

#[derive(Debug, Clone)]
pub(crate) struct OnDefendScript<D: Direction> {
    pub column_name: ImmutableString,
    pub defender_id: UnitId<D>,
    pub arguments: Array,
}

#[derive(Clone)]
pub(crate) struct ScriptedAttack<D: Direction> {
    pub attacker: AttackerPosition<D>,
    pub defender_id: UnitId<D>,
    pub priority: i32,
}

pub(super) struct AttackContextScript<D: Direction> {
    pub(super) defender_id: Option<UnitId<D>>,
    pub(super) priority: Option<Rational32>,
    pub(super) script: AttackExecutableScript,
    pub(super) arguments: Vec<Dynamic>,
}

#[derive(Debug, Clone)]
pub struct AttackExecutable<D: Direction> {
    priority: Rational32,
    attacker: Unit<D>,
    attacker_id: Option<UnitId<D>>,
    attacker_pos: Point,
    defender: Option<Unit<D>>,
    defender_id: Option<UnitId<D>>,
    defender_pos: Option<Point>,
    is_counter: bool,
    arguments: Vec<Dynamic>,
    script: AttackExecutableScript,
}

impl<D: Direction> AttackExecutable<D> {
    pub fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        self.priority.cmp(&other.priority)
    }

    fn execute(mut self, handler: &mut EventHandler<D>, current_team: ClientPerspective, heroes: &HeroMap<D>, attack_priority: i32) -> Vec<ScriptedAttack<D>> {
        match self.script {
            AttackExecutableScript::Displace { throw, neighbor_mode } => {
                let PushArguments {
                    direction,
                    distance,
                    push_limit,
                } = self.arguments.pop().unwrap().cast();
                let id = self.defender_id.unwrap().0;
                let Some((point, None, distortion)) = handler.get_observed_unit(id) else {
                    return Vec::new();
                };
                let direction = distortion.update_direction(direction);
                let push_limit = if throw {
                    0
                } else {
                    push_limit
                };
                let line = get_line(&*handler.get_game(), point, direction, distance + push_limit, neighbor_mode);
                if line.len() < 2 {
                    return Vec::new();
                }
                let attacker_team = self.attacker.get_team();
                if throw {
                    let (p, _) = line.last().unwrap().clone();
                    if handler.get_game().get_unit(p).is_none() {
                        let mut path = Path::new(point);
                        for (_, distortion) in line.iter().take(line.len() - 1) {
                            path.steps.push(PathStep::Dir(distortion.update_direction(direction)));
                        }
                        handler.unit_path(None, &path, false, true);
                    } else if attacker_team == current_team && get_visible_unit(handler.get_board(), attacker_team, p).is_none() {
                        // could show the blocking unit, but giving TrueSight seems too much
                        handler.effect_fog_surprise(p);
                    }
                } else {
                    for i in 0..distance {
                        let mut push_count = 0;
                        let mut blocked = None;
                        let mut tested_points = Vec::with_capacity(push_limit + 2);
                        for (i, (p, _)) in line.iter().skip(i).take(push_limit + 2).enumerate() {
                            tested_points.push(*p);
                            if let Some(unit) = handler.get_game().get_unit(*p) {
                                if !unit.can_be_displaced(handler.get_board(), *p, &self.attacker, self.attacker_pos, &heroes, self.is_counter) {
                                    blocked = Some(i);
                                    break;
                                }
                            } else {
                                // found a free spot to push into and all units are pushable :)
                                push_count = i;
                                break;
                            }
                        }
                        if push_count == 0 || blocked.is_some() {
                            // too many units to push, or reached edge of map
                            // show fog surprise if any of the blocking units is invisible
                            // TODO: show all at the same time or one by one?
                            for (i, p) in tested_points.into_iter().enumerate() {
                                if attacker_team == current_team && (blocked == Some(i) || get_visible_unit(handler.get_board(), attacker_team, p).is_none()) {
                                    handler.effect_fog_surprise(p);
                                }
                            }
                            // can't push any further.
                            break;
                        } else {
                            let mut paths = Vec::with_capacity(push_count);
                            let mut units = Vec::with_capacity(push_count);
                            for (p, distortion) in line.iter().skip(i).take(push_count) {
                                let unit = handler.get_game().get_unit(*p).unwrap();
                                let path = Path::with_steps(*p, vec![PathStep::Dir(distortion.update_direction(direction))]);
                                let (path_end, path_distortion) = path.end(&*handler.get_game()).unwrap();
                                let (path, unit, _additional_vision) = handler.animate_unit_path(&unit, &path, true);
                                /*if unit.get_team() == current_team {
                                    add_vision(fog_changes, &additional_vision);
                                }*/
                                paths.push(path);
                                units.push((path_end, unit));
                                if let Some(UnitId(id, disto)) = handler.get_observed_unit_id(*p, None) {
                                    handler.move_observed_unit(id, path_end, None, disto + path_distortion);
                                }
                                handler.unit_remove(*p);
                            }
                            handler.effects(paths);
                            for (p, unit) in units {
                                handler.unit_creation(p, unit);
                            }
                        }
                    }
                }
                Vec::new()
            }
            AttackExecutableScript::Rhai { ast, script } => {
                let mut scripted_attacks = Vec::new();
                let mut first_argument = Map::new();
                first_argument.insert(CONST_NAME_ATTACKER.into(), Dynamic::from(self.attacker));
                first_argument.insert(CONST_NAME_ATTACKER_POSITION.into(), Dynamic::from(self.attacker_pos));
                first_argument.insert(CONST_NAME_ATTACKER_ID.into(), dyn_opt(self.attacker_id));
                first_argument.insert(CONST_NAME_DEFENDER.into(), dyn_opt(self.defender));
                first_argument.insert(CONST_NAME_DEFENDER_POSITION.into(), dyn_opt(self.defender_pos));
                first_argument.insert(CONST_NAME_DEFENDER_ID.into(), dyn_opt(self.defender_id));
                first_argument.insert(CONST_NAME_IS_COUNTER.into(), Dynamic::from(self.is_counter));
                first_argument.insert(CONST_NAME_ATTACK_PRIORITY.into(), Dynamic::from(attack_priority));
                let executor = handler.executor(first_argument);
                match executor.run_ast::<D, Dynamic>(&ast, &script, self.arguments) {
                    Ok(result) => {
                        // script had no errors
                        match result.try_cast_result::<Array>() {
                            Ok(result) => {
                                for attack in result {
                                    match attack.try_cast_result::<ScriptedAttack<D>>() {
                                        Ok(attack) => {
                                            scripted_attacks.push(attack);
                                            continue;
                                        }
                                        Err(attack) => {
                                            crate::warn!("AttackExecutableScript::Rhai result type {} in '{script}'", attack.type_name());
                                            handler.effect_glitch();
                                        }
                                    }
                                }
                            }
                            Err(result) => {
                                if !result.is_unit() {
                                    crate::warn!("AttackExecutableScript::Rhai result type {} in '{script}'", result.type_name());
                                    handler.effect_glitch();
                                }
                            }
                        }
                    }
                    Err(e) => {
                        handler.environment().log_rhai_error("AttackExecutableScript::Rhai", &script, &e);
                        handler.effect_glitch();
                    }
                }
                scripted_attacks
            }
        }
    }
}

#[derive(Debug, Clone)]
pub enum AttackExecutableScript {
    //DealDamage,
    Displace {
        throw: bool,                            // the displaced unit skips over units that stand in the way. push_limit is ignored if true
        neighbor_mode: NeighborMode,
    },
    Rhai {
        ast: Rc<AST>,
        script: ImmutableString,
    },
}

pub(super) fn execute_attacks_with_equal_priority<D: Direction>(
    handler: &mut EventHandler<D>,
    attacks: Vec<(AttackerInfo<D>, ConfiguredAttack)>,
    execute_scripts: bool,
) -> Vec<ScriptedAttack<D>> {
    let attack_priority = match attacks.first() {
        Some((_, atk)) => atk.priority as i32,
        _ => return Vec::new()
    };
    let current_team = handler.get_game().current_team();
    // all these attacks have the same priority, so they shouldn't influence one another
    let heroes = HeroMap::new(handler.get_board(), None);
    let heroes_with_ids = heroes.with_ids(handler);
    let mut attacks: Vec<AttackExecutable<D>> = attacks.into_iter()
    .filter_map(|(attacker, attack)| {
        let unit = attacker.attacker_position.get_unit(handler)?;
        //tracing::debug!("unit of type {} attacks!", unit.name());
        let (input, attack_direction, attack_pattern) = attacker.retarget(handler, &attack, &heroes)?;
        let splash_range = attack.splash.iter().map(|a| a.splash_distance).max()?;
        let ranges: Vec<Vec<OrientedPoint<D>>> = attack.splash_pattern.get_splash(handler.get_board(), &unit, attacker.temporary_ballast, &attack_pattern, input, splash_range);
        let mut result = Vec::new();
        for splash_instance in &attack.splash {
            if splash_instance.splash_distance >= ranges.len() {
                // can happen if splash_pattern uses SplashDamagePointSource::AttackPattern
                continue;
            }
            for exe in splash_instance.into_executable(handler, &attack, splash_instance, &attacker.attacker_position, attack_direction, &ranges[splash_instance.splash_distance], &heroes, &heroes_with_ids, attacker.temporary_ballast, &attacker.counter_state) {
                result.push(exe);
            }
        }
        Some(result)
    }).flatten()
    .collect();
    attacks.sort_by(AttackExecutable::cmp);
    //let mut fog_changes = FxHashMap::default();
    let mut scripted_attacks = Vec::new();
    for attack in attacks {
        scripted_attacks.extend(attack.execute(handler, current_team, &heroes, attack_priority));
    }
    if handler.get_game().has_secrets() {
        //handler.change_fog(current_team, fog_changes);
        handler.recalculate_fog();
    }
    cleanup_dead_material(handler, execute_scripts);
    scripted_attacks
}
