use std::collections::HashSet;
use std::fmt::Debug;
use zipper::*;

use crate::config::environment::Environment;
use crate::game::fog::FogIntensity;
use crate::map::direction::Direction;
use crate::game::game::Game;
use crate::map::map::Map;
use crate::map::point::Point;
use crate::script::custom_action::CustomActionTestResult;
use super::attributes::*;
use super::commands::UnitAction;
use super::movement::{Path, TBallast};
use super::unit::Unit;

crate::enum_with_custom! {
    #[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
    pub enum HeroType {
        None,
        EarlGrey,
        Crystal,
        CrystalObelisk,
    }
}

impl HeroType {
    pub fn max_charge(&self, environment: &Environment) -> u8 {
        environment.config.hero_charge(*self)
    }

    pub fn transport_capacity(&self, environment: &Environment) -> usize {
        environment.config.hero_transport_capacity(*self) as usize
    }

    pub fn price<D: Direction>(&self, environment: &Environment, unit: &Unit<D>) -> Option<i32> {
        environment.config.hero_price(*self, unit.typ())
    }
}

// TODO: implement Display
/**
 * Hero purposefully doesn't have Environment.
 * This way, it's easier to create a dummy unit without access to Environment/Config
 */
#[derive(Clone, PartialEq, Eq)]
pub struct Hero {
    typ: HeroType,
    power: usize,
    charge: u8,
    origin: Option<Point>,
}
attribute!(Hero, Hero);

impl Debug for Hero {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Hero{{typ: {:?}", self.typ)?;
        write!(f, ", charge: {}", self.charge)?;
        write!(f, ", power: {}", self.power)?;
        write!(f, "}}")
    }
}

impl Hero {
    pub fn new(typ: HeroType, origin: Option<Point>) -> Self {
        Self {
            typ,
            power: 0,
            charge: 0,
            origin,
        }
    }

    pub fn typ(&self) -> HeroType {
        self.typ
    }

    pub fn get_origin(&self) -> Option<Point> {
        self.origin
    }

    pub fn max_charge(&self, environment: &Environment) -> u8 {
        self.typ.max_charge(environment)
    }
    pub fn get_charge(&self) -> u8 {
        self.charge
    }
    pub fn set_charge(&mut self, environment: &Environment, charge: u8) {
        self.charge = charge.min(self.typ.max_charge(environment));
    }

    pub fn get_next_power(&self, environment: &Environment) -> usize {
        let power = match environment.config.hero_powers(self.typ).get(self.power) {
            Some(power) => power,
            None => return 0,
        };
        power.next_power as usize
    }

    pub fn get_active_power(&self) -> usize {
        self.power
    }
    pub fn set_active_power(&mut self, index: usize) {
        self.power = index;
    }

    pub fn can_activate_power(&self, environment: &Environment, index: usize) -> bool {
        if self.power == index {
            return false;
        }
        let power = match environment.config.hero_powers(self.typ).get(index) {
            Some(power) => power,
            None => return false,
        };
        power.usable_from_power.contains(&(self.power as u8))
        && power.required_charge <= self.charge
    }

    pub fn power_cost(&self, environment: &Environment, index: usize) -> u8 {
        let power = match environment.config.hero_powers(self.typ).get(index) {
            Some(power) => power,
            None => return 0,
        };
        power.required_charge
    }

    pub fn aura_range<D: Direction>(
        game: Option<&Game<D>>,
        map: &Map<D>,
        unit: &Unit<D>,
        unit_pos: Point,
        transporter: Option<(&Unit<D>, usize)>,
    ) -> Option<usize> {
        map.environment().config.hero_aura_range(game, map, unit, unit_pos, transporter).map(|r| r as usize)
    }

    pub fn in_range<D: Direction>(
        game: Option<&Game<D>>,
        map: &Map<D>,
        unit: &Unit<D>,
        unit_pos: Point,
        transporter: Option<(&Unit<D>, usize)>,
        target: Point,
    ) -> bool {
        Self::aura(game, map, unit, unit_pos, transporter).contains(&target)
    }

    pub fn aura<D: Direction>(
        game: Option<&Game<D>>,
        map: &Map<D>,
        unit: &Unit<D>,
        unit_pos: Point,
        transporter: Option<(&Unit<D>, usize)>,
    ) -> HashSet<Point> {
        let mut result = HashSet::new();
        let aura_range = match Self::aura_range(game, map, unit, unit_pos, transporter) {
            Some(aura_range) => aura_range,
            _ => return result
        };
        result.insert(unit_pos);
        for layer in map.range_in_layers(unit_pos, aura_range) {
            for p in layer {
                result.insert(p);
            }
        }
        result
    }

    pub fn hero_influence_at<D: Direction>(game: Option<&Game<D>>, map: &Map<D>, point: Point, owner_id: i8) -> Vec<(Unit<D>, Self, Point, Option<usize>)> {
        let mut result = vec![];
        for p in map.all_points() {
            if let Some(unit) = map.get_unit(p) {
                if !unit.is_hero() || owner_id >= 0 && unit.get_owner_id() != owner_id {
                    continue;
                }
                let hero = unit.get_hero();
                if Self::in_range(game, map, unit, p, None, point) {
                    result.push((unit.clone(), hero, p, None));
                }
                for (i, u) in unit.get_transported().iter().enumerate() {
                    if u.is_hero() {
                        let hero = u.get_hero();
                        if Self::in_range(game, map, u, p, Some((unit, i)), point) {
                            result.push((u.clone(), hero, p, Some(i)));
                        }
                    }
                }
            }
        }
        result
    }

    pub fn add_options_after_path<D: Direction>(
        &self,
        list: &mut Vec<UnitAction<D>>,
        unit: &Unit<D>,
        game: &Game<D>,
        funds: i32,
        path: &Path<D>,
        destination: Point,
        transporter: Option<&Unit<D>>,
        heroes: &[&(Unit<D>, Hero, Point, Option<usize>)],
        ballast: &[TBallast<D>],
        get_fog: &impl Fn(Point) -> FogIntensity
    ) {
        let data = Vec::new();
        for (i, power) in game.environment().config.hero_powers(self.typ).iter().enumerate() {
            if self.charge >= power.required_charge
            && power.usable_from_power.contains(&(self.power as u8))
            && power.script.next_condition(game, funds, unit, path, destination, transporter, heroes, ballast, &data, get_fog) != CustomActionTestResult::Failure {
                list.push(UnitAction::HeroPower(i, Vec::new()));
            }
        }
    }
}

impl SupportedZippable<&Environment> for Hero {
    fn export(&self, zipper: &mut Zipper, environment: &Environment) {
        let bits = bits_needed_for_max_value(environment.config.hero_count() as u32 - 1);
        zipper.write_u32(environment.config.hero_types().iter().position(|t| *t == self.typ).unwrap_or(0) as u32, bits);
        if self.typ == HeroType::None {
            return;
        }
        self.origin.export(zipper, environment);
        zipper.write_u8(self.power as u8, bits_needed_for_max_value(environment.config.hero_powers(self.typ).len() as u32 - 1));
        if self.typ.max_charge(&environment) > 0 {
            let bits = bits_needed_for_max_value(self.typ.max_charge(&environment) as u32);
            zipper.write_u8(self.charge, bits);
        }
    }

    fn import(unzipper: &mut Unzipper, environment: &Environment) -> Result<Self, ZipperError> {
        let bits = bits_needed_for_max_value(environment.config.hero_count() as u32 - 1);
        let typ = *environment.config.hero_types().get(unzipper.read_u32(bits)? as usize).ok_or(ZipperError::EnumOutOfBounds("HeroType".to_string()))?;
        let origin = if typ != HeroType::None {
            Option::<Point>::import(unzipper, environment)?
        } else {
            None
        };
        let mut result = Self::new(typ, origin);
        if typ != HeroType::None {
            result.power = unzipper.read_u8(bits_needed_for_max_value(environment.config.hero_powers(typ).len() as u32 - 1))? as usize;
            if typ.max_charge(environment) > 0 {
                let bits = bits_needed_for_max_value(typ.max_charge(environment) as u32);
                result.charge = typ.max_charge(environment).min(unzipper.read_u8(bits)?);
            }
        }
        Ok(result)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct HeroChargeChange(pub i8);

impl SupportedZippable<&Environment> for HeroChargeChange {
    fn export(&self, zipper: &mut Zipper, support: &Environment) {
        let max = support.config.max_hero_charge() as i8;
        zipper.write_u8((self.0 + max) as u8, bits_needed_for_max_value(max as u32 * 2));
    }
    fn import(unzipper: &mut Unzipper, support: &Environment) -> Result<Self, ZipperError> {
        let max = support.config.max_hero_charge() as i8;
        Ok(Self(unzipper.read_u8(bits_needed_for_max_value(max as u32 * 2))? as i8 - max))
    }
}

impl From<i8> for HeroChargeChange {
    fn from(value: i8) -> Self {
        Self(value)
    }
}


#[cfg(test)]
mod tests {

    use std::sync::Arc;

    use interfaces::game_interface::*;
    use interfaces::map_interface::*;
    use crate::config::config::Config;
    use crate::game::commands::Command;
    use crate::game::fog::*;
    use crate::map::direction::*;
    use crate::map::map::Map;
    use crate::map::point::Point;
    use crate::map::point::Position;
    use crate::map::point_map::PointMap;
    use crate::map::wrapping_map::WMBuilder;
    use crate::script::custom_action::CustomActionData;
    use crate::terrain::TerrainType;
    use crate::units::combat::AttackVector;
    use crate::units::commands::UnitAction;
    use crate::units::commands::UnitCommand;
    use crate::units::hero::ActionStatus;
    use crate::units::hero::Hero;
    use crate::units::hero::HeroType;
    use crate::units::movement::Path;
    use crate::units::movement::PathStep;
    use crate::units::unit_types::UnitType;

    #[test]
    fn buy_hero() {
        let config = Arc::new(Config::test_config());
        let map = PointMap::new(5, 5, false);
        let map = WMBuilder::<Direction4>::new(map);
        let mut map = Map::new(map.build(), &config);
        let map_env = map.environment().clone();
        map.set_terrain(Point::new(1, 1), TerrainType::Memorial.instance(&map_env).set_owner_id(0).build_with_defaults());
        map.set_unit(Point::new(1, 1), Some(UnitType::SmallTank.instance(&map_env).set_owner_id(0).build_with_defaults()));
        map.set_unit(Point::new(4, 4), Some(UnitType::SmallTank.instance(&map_env).set_owner_id(1).set_hero(Hero::new(HeroType::CrystalObelisk, None)).build_with_defaults()));

        let settings = map.settings().unwrap();
        let mut settings = settings.clone();
        settings.fog_mode = FogMode::Constant(FogSetting::None);
        settings.players[0].set_funds(999999);

        let (mut server, _) = map.clone().game_server(&settings, || 0.);
        let environment: crate::config::environment::Environment = server.environment().clone();
        assert_eq!(Hero::aura_range(Some(&server), server.get_map(), server.get_map().get_unit(Point::new(1, 1)).unwrap(), Point::new(1, 1), None), None);
        let path = Path::new(Point::new(1, 1));
        let options = server.get_map().get_unit(Point::new(1, 1)).unwrap().options_after_path(&server, &path, None, &[]);
        println!("options: {:?}", options);
        assert!(options.contains(&UnitAction::BuyHero(HeroType::Crystal)));
        server.handle_command(Command::UnitCommand(UnitCommand {
            unload_index: None,
            path,
            action: UnitAction::BuyHero(HeroType::Crystal),
        }), || 0.).unwrap();
        assert_eq!(server.get_map().get_unit(Point::new(1, 1)), Some(&UnitType::SmallTank.instance(&environment).set_owner_id(0).set_hero(Hero::new(HeroType::Crystal, Some(Point::new(1, 1)))).set_status(ActionStatus::Exhausted).build_with_defaults()));
        assert_eq!(Hero::aura_range(Some(&server), server.get_map(), server.get_map().get_unit(Point::new(1, 1)).unwrap(), Point::new(1, 1), None), Some(2));
    }


    #[test]
    fn crystal() {
        let config = Arc::new(Config::test_config());
        let map = PointMap::new(5, 5, false);
        let map = WMBuilder::<Direction4>::new(map);
        let mut map = Map::new(map.build(), &config);
        let map_env = map.environment().clone();
        let mut crystal = Hero::new(HeroType::Crystal, None);
        crystal.set_charge(&map_env, crystal.max_charge(&map_env));
        //map.set_unit(Point::new(0, 0), Some(UnitType::SmallTank.instance(&map_env).set_owner_id(0).set_hero(Hero::new(HeroType::CrystalObelisk, None)).build_with_defaults()));
        map.set_unit(Point::new(1, 1), Some(UnitType::SmallTank.instance(&map_env).set_owner_id(0).set_hero(crystal).set_hp(1).build_with_defaults()));
        map.set_unit(Point::new(2, 1), Some(UnitType::SmallTank.instance(&map_env).set_owner_id(0).build_with_defaults()));
        map.set_unit(Point::new(3, 1), Some(UnitType::SmallTank.instance(&map_env).set_owner_id(1).build_with_defaults()));

        map.set_unit(Point::new(4, 4), Some(UnitType::SmallTank.instance(&map_env).set_owner_id(0).set_hero(Hero::new(HeroType::CrystalObelisk, None)).build_with_defaults()));

        let settings = map.settings().unwrap();
        let mut settings = settings.clone();
        settings.fog_mode = FogMode::Constant(FogSetting::None);

        let (mut server, _) = map.clone().game_server(&settings, || 0.);
        let unchanged = server.clone();
        let environment: crate::config::environment::Environment = server.environment().clone();
        assert_eq!(Hero::aura_range(Some(&server), server.get_map(), server.get_map().get_unit(Point::new(1, 1)).unwrap(), Point::new(1, 1), None), Some(2));
        assert_eq!(Hero::aura_range(Some(&server), server.get_map(), server.get_map().get_unit(Point::new(4, 4)).unwrap(), Point::new(4, 4), None), Some(2));
        // use power
        let path = Path::new(Point::new(1, 1));
        let options = server.get_map().get_unit(Point::new(1, 1)).unwrap().options_after_path(&server, &path, None, &[]);
        println!("options: {:?}", options);
        assert!(options.contains(&UnitAction::HeroPower(1, Vec::new())));
        server.handle_command(Command::UnitCommand(UnitCommand {
            unload_index: None,
            path,
            action: UnitAction::HeroPower(1, vec![CustomActionData::Point(Point::new(0, 1))]),
        }), || 0.).unwrap();
        assert_eq!(server.get_map().get_unit(Point::new(0, 1)), Some(&UnitType::HeroCrystal.instance(&environment).set_owner_id(0).set_hero(Hero::new(HeroType::CrystalObelisk, None)).build_with_defaults()));
        assert_eq!(Hero::aura_range(Some(&server), server.get_map(), server.get_map().get_unit(Point::new(1, 1)).unwrap(), Point::new(1, 1), None), Some(3));
        assert_eq!(Hero::aura_range(Some(&server), server.get_map(), server.get_map().get_unit(Point::new(4, 4)).unwrap(), Point::new(4, 4), None), Some(3));
        server.handle_command(Command::UnitCommand(UnitCommand {
            unload_index: None,
            path: Path::new(Point::new(2, 1)),
            action: UnitAction::Attack(AttackVector::Direction(Direction4::D0)),
        }), || 0.).unwrap();
        let power_aura_damage = 100 - server.get_map().get_unit(Point::new(3, 1)).unwrap().get_hp();

        // don't use power
        let mut server = unchanged.clone();
        server.handle_command(Command::UnitCommand(UnitCommand {
            unload_index: None,
            path: Path::new(Point::new(2, 1)),
            action: UnitAction::Attack(AttackVector::Direction(Direction4::D0)),
        }), || 0.).unwrap();
        let aura_damage = 100 - server.get_map().get_unit(Point::new(3, 1)).unwrap().get_hp();
        server.handle_command(Command::EndTurn, || 0.).unwrap();
        server.handle_command(Command::EndTurn, || 0.).unwrap();
        assert_eq!(server.get_map().get_unit(Point::new(4, 4)).unwrap().get_hp(), 100);
        assert_eq!(Hero::hero_influence_at(Some(&server), server.get_map(), Point::new(0, 0), 0).len(), 1);
        assert_eq!(Hero::hero_influence_at(Some(&server), server.get_map(), Point::new(0, 0), 1).len(), 0);
        assert_eq!(Hero::hero_influence_at(Some(&server), server.get_map(), Point::new(0, 0), -1).len(), 1);

        assert!(aura_damage < power_aura_damage);

        // test crystal obelisk behavior when hero is missing
        map.set_unit(Point::new(1, 1), None);
        let (mut server, _) = map.clone().game_server(&settings, || 0.);
        assert_eq!(server.get_map().get_unit(Point::new(4, 4)).unwrap().get_hp(), 80);
        server.handle_command(Command::EndTurn, || 0.).unwrap();
        server.handle_command(Command::EndTurn, || 0.).unwrap();
        assert_eq!(server.get_map().get_unit(Point::new(4, 4)).unwrap().get_hp(), 60);
        server.handle_command(Command::UnitCommand(UnitCommand {
            unload_index: None,
            path: Path::new(Point::new(2, 1)),
            action: UnitAction::Attack(AttackVector::Direction(Direction4::D0)),
        }), || 0.).unwrap();
        let normal_damage = 100 - server.get_map().get_unit(Point::new(3, 1)).unwrap().get_hp();

        assert!(normal_damage < aura_damage);
    }

    #[test]
    fn earl_grey() {
        let config = Arc::new(Config::test_config());
        let map = PointMap::new(5, 5, false);
        let map = WMBuilder::<Direction4>::new(map);
        let mut map = Map::new(map.build(), &config);
        let map_env = map.environment().clone();
        let mut earl_grey = Hero::new(HeroType::EarlGrey, None);
        earl_grey.set_charge(&map_env, earl_grey.max_charge(&map_env));
        //map.set_unit(Point::new(0, 0), Some(UnitType::SmallTank.instance(&map_env).set_owner_id(0).set_hero(Hero::new(HeroType::CrystalObelisk, None)).build_with_defaults()));
        map.set_unit(Point::new(1, 1), Some(UnitType::HoverBike.instance(&map_env).set_owner_id(0).set_hero(earl_grey).set_hp(1).build_with_defaults()));
        map.set_unit(Point::new(2, 1), Some(UnitType::HoverBike.instance(&map_env).set_owner_id(0).build_with_defaults()));
        map.set_unit(Point::new(3, 1), Some(UnitType::HoverBike.instance(&map_env).set_owner_id(1).build_with_defaults()));

        map.set_unit(Point::new(4, 4), Some(UnitType::HoverBike.instance(&map_env).set_owner_id(0).build_with_defaults()));

        let settings = map.settings().unwrap();
        let mut settings = settings.clone();
        settings.fog_mode = FogMode::Constant(FogSetting::None);

        let (mut server, _) = map.clone().game_server(&settings, || 0.);
        let unchanged = server.clone();
        let environment: crate::config::environment::Environment = server.environment().clone();
        let influence1 = Hero::hero_influence_at(Some(&server), server.get_map(), Point::new(2, 1), 0);
        let influence1: Vec<_> = influence1.iter().collect();
        let influence2 = Hero::hero_influence_at(Some(&server), server.get_map(), Point::new(4, 4), 0);
        let influence2: Vec<_> = influence2.iter().collect();
        assert_eq!(
            server.get_map().get_unit(Point::new(2, 1)).unwrap().movement_points(Some(&server), server.get_map(), Point::new(2, 1), None, &influence1),
            server.get_map().get_unit(Point::new(4, 4)).unwrap().movement_points(Some(&server), server.get_map(), Point::new(4, 4), None, &influence2),
        );
        // hero power shouldn't be available if the hero moves
        let mut path = Path::new(Point::new(1, 1));
        path.steps.push(PathStep::Dir(Direction4::D90));
        let options = server.get_map().get_unit(Point::new(1, 1)).unwrap().options_after_path(&server, &path, None, &[]);
        println!("options: {:?}", options);
        assert!(!options.contains(&UnitAction::HeroPower(1, Vec::new())));
        // use power
        let path = Path::new(Point::new(1, 1));
        let options = server.get_map().get_unit(Point::new(1, 1)).unwrap().options_after_path(&server, &path, None, &[]);
        println!("options: {:?}", options);
        assert!(options.contains(&UnitAction::HeroPower(1, Vec::new())));
        server.handle_command(Command::UnitCommand(UnitCommand {
            unload_index: None,
            path,
            action: UnitAction::HeroPower(1, Vec::new()),
        }), || 0.).unwrap();
        assert_eq!(server.get_map().get_unit(Point::new(2, 1)).unwrap().get_status(), ActionStatus::Ready);
        let influence1 = Hero::hero_influence_at(Some(&server), server.get_map(), Point::new(2, 1), 0);
        let influence1: Vec<_> = influence1.iter().collect();
        let influence2 = Hero::hero_influence_at(Some(&server), server.get_map(), Point::new(4, 4), 0);
        let influence2: Vec<_> = influence2.iter().collect();
        assert!(
            server.get_map().get_unit(Point::new(2, 1)).unwrap().movement_points(Some(&server), server.get_map(), Point::new(2, 1), None, &influence1)
            >
            server.get_map().get_unit(Point::new(4, 4)).unwrap().movement_points(Some(&server), server.get_map(), Point::new(4, 4), None, &influence2)
        );
    }
}
