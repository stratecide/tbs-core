use std::collections::HashSet;

use crate::config::parse::{parse_tuple1, string_base, FromConfig};
use crate::config::ConfigParseError;
use crate::game::event_handler::EventHandler;
use crate::game::game_view::GameView;
use crate::map::direction::Direction;
use crate::map::map::NeighborMode;
use crate::map::map_view::MapView;
use crate::map::point::Point;
use crate::units::attributes::{ActionStatus, AttributeKey};
use crate::units::commands::UnitAction;
use crate::units::hero::{Hero, HeroType};
use crate::units::movement::{Path, TBallast};
use crate::units::unit::Unit;
use crate::units::unit_types::UnitType;


/*pub enum CustomActionDataType {
    Point,
    Direction,
    UnitType,
}*/

#[derive(Debug, Clone, PartialEq)]
pub enum CustomActionData<D: Direction> {
    Point(Point),
    Direction(D),
    UnitType(UnitType),
}

#[derive(Debug, Clone, PartialEq)]
pub enum CustomActionDataOptions<D: Direction> {
    Point(HashSet<Point>),
    Direction(Point, HashSet<D>),
    UnitShop(Vec<(Unit<D>, i32)>),
}

impl<D: Direction> CustomActionDataOptions<D> {
    fn contains(&self, data: &CustomActionData<D>) -> bool {
        match (self, data) {
            (Self::Point(options), CustomActionData::Point(option)) => options.contains(option),
            (Self::Direction(_visual_center, options), CustomActionData::Direction(option)) => options.contains(option),
            (Self::UnitShop(options), CustomActionData::UnitType(option)) => options.iter().any(|o| o.0.typ() == *option),
            _ => false
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum CustomActionTestResult<D: Direction> {
    Success,
    // ideally only returned in the first iteration.
    // but if the script returns an error, that's also a failure (and should be logged somewhere)
    Failure,
    Next(CustomActionDataOptions<D>),
    NextOrSuccess(CustomActionDataOptions<D>),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CustomAction {
    None, // not parsed, since this isn't valid for normal units. it's used as default for hero powers
    UnexhaustWithoutMoving,
    SummonCrystal(HeroType),
    ActivateUnits,
    BuyUnit,
    SwapUnitPositions,
}

impl FromConfig for CustomAction {
    fn from_conf(s: &str) -> Result<(Self, &str), ConfigParseError> {
        let (base, mut arguments) = string_base(s);
        Ok((match base {
            "UnexhaustWithoutMoving" => Self::UnexhaustWithoutMoving,
            "SummonCrystal" => {
                let (hero_type, remainder) = parse_tuple1(arguments)?;
                arguments = remainder;
                Self::SummonCrystal(hero_type)
            },
            "ActivateUnits" => Self::ActivateUnits,
            "BuyUnit" => Self::BuyUnit,
            "SwapUnitPositions" => Self::SwapUnitPositions,
            invalid => return Err(ConfigParseError::UnknownEnumMember(format!("CustomAction::{}", invalid))),
        }, arguments))
    }
}

impl CustomAction {
    pub fn next_condition<D: Direction>(
        &self,
        game: &impl GameView<D>,
        funds: i32,
        unit: &Unit<D>,
        path: &Path<D>,
        destination: Point,
        transporter: Option<(&Unit<D>, usize)>,
        heroes: &[(Unit<D>, Hero, Point, Option<usize>)],
        ballast: &[TBallast<D>],
        data_so_far: &[CustomActionData<D>],
    ) -> CustomActionTestResult<D> {
        let transporter: Option<(&Unit<D>, Point)> = transporter.map(|(u, _)| (u, path.start));
        match self {
            Self::None => CustomActionTestResult::Success,
            Self::UnexhaustWithoutMoving => {
                if path.len() == 0 {
                    CustomActionTestResult::Success
                } else {
                    CustomActionTestResult::Failure
                }
            }
            Self::SummonCrystal(_) => {
                if data_so_far.len() == 0 {
                    let options = game.get_neighbors(destination, NeighborMode::FollowPipes).into_iter()
                    .map(|op| op.point)
                    .filter(|p| {
                        game.get_unit(*p).is_none()
                    }).collect();
                    CustomActionTestResult::Next(CustomActionDataOptions::Point(options))
                } else {
                    CustomActionTestResult::Success
                }
            }
            Self::ActivateUnits => CustomActionTestResult::Success,
            Self::BuyUnit => {
                let build_inside = unit.has_attribute(AttributeKey::Transported);
                if data_so_far.len() == 0 {
                    if build_inside && unit.remaining_transport_capacity() == 0 {
                        return CustomActionTestResult::Failure;
                    }
                    let mut options = Vec::new();
                    for unit_type in unit.transportable_units() {
                        options.push(unit.unit_shop_option(game, destination, *unit_type, transporter, heroes, ballast));
                    }
                    CustomActionTestResult::Next(CustomActionDataOptions::UnitShop(options))
                } else if data_so_far.len() == 1 {
                    let unit = match data_so_far {
                        [CustomActionData::UnitType(unit_type)] => {
                            let (unit, cost) = unit.unit_shop_option(game, destination, *unit_type, transporter, heroes, ballast);
                            if cost > funds {
                                return CustomActionTestResult::Failure;
                            }
                            unit
                        }
                        _ => return CustomActionTestResult::Failure,
                    };
                    if !build_inside {
                        let options = game.get_neighbors(destination, NeighborMode::FollowPipes).into_iter()
                        .filter(|p| {
                            game.get_terrain(p.point).unwrap().movement_cost(unit.default_movement_type()).is_some()
                            && game.get_unit(p.point).is_none()
                        })
                        .map(|op| op.direction)
                        .collect();
                        CustomActionTestResult::Next(CustomActionDataOptions::Direction(destination, options))
                    } else {
                        CustomActionTestResult::Success
                    }
                } else {
                    CustomActionTestResult::Success
                }
            }
            Self::SwapUnitPositions => {
                if data_so_far.len() == 0 {
                    let options = game.get_neighbors(destination, NeighborMode::FollowPipes).into_iter()
                    .map(|op| op.point)
                    .filter(|p| {
                        *p != destination && game.get_unit(*p).is_some()
                    }).collect();
                    CustomActionTestResult::Next(CustomActionDataOptions::Point(options))
                } else if data_so_far.len() == 1 {
                    let first_point = match data_so_far[0] {
                        CustomActionData::Point(p) => p,
                        _ => return CustomActionTestResult::Failure,
                    };
                    let mut options = HashSet::new();
                    for layer in game.range_in_layers(destination, 5) {
                        for p in layer {
                            if p != first_point && p != destination && game.get_unit(p).is_some() {
                                options.insert(p);
                            }
                        }
                    }
                    CustomActionTestResult::Next(CustomActionDataOptions::Point(options))
                } else {
                    CustomActionTestResult::Success
                }
            }
        }
    }

    pub fn is_data_valid<D: Direction>(
        &self,
        game: &impl GameView<D>,
        unit: &Unit<D>,
        path: &Path<D>,
        destination: Point,
        transporter: Option<(&Unit<D>, usize)>,
        ballast: &[TBallast<D>],
        data: &[CustomActionData<D>],
    ) -> bool {
        let funds = game.get_owning_player(unit.get_owner_id()).unwrap().funds_after_path(game, path);
        let heroes = Hero::hero_influence_at(game, destination, unit.get_owner_id());
        for i in 0..data.len() {
            use CustomActionTestResult as R;
            match self.next_condition(game, funds, unit, path, destination, transporter, &heroes, ballast, &data[..i]) {
                R::Failure => return false,
                R::Success => return i == data.len(),
                R::Next(options) => {
                    if i >= data.len() || !options.contains(&data[i]) {
                        return false;
                    }
                }
                R::NextOrSuccess(options) => {
                    if i < data.len() && !options.contains(&data[i]) {
                        return false;
                    }
                }
            }
        }
        true
    }

    pub fn execute<D: Direction>(
        &self,
        handler: &mut EventHandler<D>,
        unit: &Unit<D>,
        path: &Path<D>,
        destination: Point,
        _transporter: Option<(&Unit<D>, usize)>,
        _heroes: &[(Unit<D>, Hero, Point, Option<usize>)],
        ballast: &[TBallast<D>],
        data: &[CustomActionData<D>],
    ) {
        match self {
            Self::None => (),
            Self::UnexhaustWithoutMoving => {
                handler.unit_status(destination, ActionStatus::Ready);
            }
            Self::SummonCrystal(hero_type) => {
                let crystal_pos = match data {
                    &[CustomActionData::Point(p)] => p,
                    _ => panic!("SummonCrystal Action Data is wrong: {:?}", data),
                };
                let builder = UnitType::HeroCrystal.instance(handler.environment())
                .set_owner_id(unit.get_owner_id())
                .set_hero(Hero::new(*hero_type, None));
                handler.unit_creation(crystal_pos, builder.build_with_defaults());
            }
            Self::ActivateUnits => {
                for p in handler.get_map().get_neighbors(destination, NeighborMode::FollowPipes) {
                    if let Some(u) = handler.get_map().get_unit(p.point) {
                        if u.get_owner_id() == unit.get_owner_id() && u.is_exhausted() {
                            handler.unit_status(p.point, ActionStatus::Ready);
                        }
                    }
                }
            }
            Self::BuyUnit => {
                match data {
                    &[CustomActionData::UnitType(unit_type)] => {
                        UnitAction::buy_transported_unit(handler, path.start, destination, unit_type, ballast, false);
                    },
                    &[CustomActionData::UnitType(unit_type), CustomActionData::Direction(dir)] => {
                        UnitAction::buy_unit(handler, path.start, destination, unit_type, dir, ballast, false);
                    },
                    _ => panic!("BuyUnit Action Data is wrong: {:?}", data),
                };
            }
            Self::SwapUnitPositions => {
                let (p1, p2) = match data {
                    &[CustomActionData::Point(p1), CustomActionData::Point(p2)] => (p1, p2),
                    _ => panic!("SwapUnitPositions Action Data is wrong: {:?}", data),
                };
                let unit1 = handler.get_map().get_unit(p1).cloned().unwrap();
                let unit2 = handler.get_map().get_unit(p2).cloned().unwrap();
                handler.unit_replace(p1, unit2);
                handler.unit_replace(p2, unit1);
            }
        }
    }
}
