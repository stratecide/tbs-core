pub mod commander_type;


use commander_type::CommanderType;
use zipper::*;

use crate::config::environment::Environment;
use crate::script::player::PlayerScript;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Commander {
    typ: CommanderType,
    charge: u32,
    power: usize,
    environment: Environment,
}

impl SupportedZippable<&Environment> for Commander {
    fn export(&self, zipper: &mut Zipper, support: &Environment) {
        self.typ.export(zipper, &support.config);
        zipper.write_u32(self.charge, bits_needed_for_max_value(support.config.max_commander_charge()));
        zipper.write_u8(self.power as u8, bits_needed_for_max_value(support.config.commander_powers(self.typ).len() as u32 - 1));
    }
    fn import(unzipper: &mut Unzipper, support: &Environment) -> Result<Self, ZipperError> {
        let typ = CommanderType::import(unzipper, &support.config)?;
        let charge = unzipper.read_u32(bits_needed_for_max_value(support.config.max_commander_charge()))?;
        let power = unzipper.read_u8(bits_needed_for_max_value(support.config.commander_powers(typ).len() as u32 - 1))? as usize;
        Ok(Self {
            typ,
            charge,
            power,
            environment: support.clone(),
        })
    }
}

impl Commander {
    pub fn new(environment: &Environment, typ: CommanderType) -> Self {
        Self {
            typ,
            charge: 0,
            power: 0,
            environment: environment.clone(),
        }
    }

    pub fn environment(&self) -> &Environment {
        &self.environment
    }

    pub fn typ(&self) -> CommanderType {
        self.typ
    }

    pub fn name(&self) -> &str {
        self.environment.config.commander_name(self.typ)
    }

    pub fn get_charge(&self) -> u32 {
        self.charge
    }
    pub fn get_max_charge(&self) -> u32 {
        self.environment.config.commander_max_charge(self.typ)
    }
    pub fn add_charge(&mut self, delta: i32) {
        self.charge = (self.charge as i32 + delta).max(0) as u32;
    }
    pub fn can_gain_charge(&self) -> bool {
        self.environment.config.commander_can_gain_charge(self.typ, self.power)
    }

    pub fn power_count(&self) -> usize {
        self.environment.config.commander_powers(self.typ).len()
    }

    pub fn power_name(&self, index: usize) -> &str {
        self.environment.config.commander_powers(self.typ)
        .get(index).and_then(|p| Some(p.name.as_str()))
        .unwrap_or("")
    }

    pub fn get_next_power(&self) -> usize {
        let power = match self.environment.config.commander_powers(self.typ).get(self.power) {
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

    pub fn can_activate_power(&self, index: usize, automatic: bool) -> bool {
        if self.power == index {
            return false;
        }
        let power = match self.environment.config.commander_powers(self.typ).get(index) {
            Some(power) => power,
            None => return false,
        };
        power.required_charge <= self.charge
        && if automatic {
            index == self.get_next_power()
        } else {
            power.usable_from_power.contains(&(self.power as u8))
        }
    }

    pub fn power_cost(&self, index: usize) -> u32 {
        let power = match self.environment.config.commander_powers(self.typ).get(index) {
            Some(power) => power,
            None => return 0,
        };
        power.required_charge
    }

    pub fn power_activation_effects(&self, index: usize) -> Vec<PlayerScript> {
        let power = match self.environment.config.commander_powers(self.typ).get(index) {
            Some(power) => power,
            None => return Vec::new(),
        };
        power.effects.clone()
    }

}


#[cfg(test)]
mod tests {

    use std::sync::Arc;

    use interfaces::game_interface::*;
    use interfaces::map_interface::*;
    use crate::commander::commander_type::CommanderType;
    use crate::config::config::Config;
    use crate::details::Detail;
    use crate::game::commands::Command;
    use crate::game::fog::*;
    use crate::map::direction::*;
    use crate::map::map::Map;
    use crate::map::map_view::MapView;
    use crate::map::point::Point;
    use crate::map::point::Position;
    use crate::map::point_map::PointMap;
    use crate::map::wrapping_map::WMBuilder;
    use crate::terrain::TerrainType;
    use crate::units::combat::AttackVector;
    use crate::units::commands::UnitAction;
    use crate::units::commands::UnitCommand;
    use crate::units::movement::Path;
    use crate::units::unit_types::UnitType;

    #[test]
    fn zombie() {
        let config = Arc::new(Config::test_config());
        let map = PointMap::new(5, 5, false);
        let map = WMBuilder::<Direction6>::new(map);
        let mut map = Map::new(map.build(), &config);
        let environment = map.environment().clone();
        map.set_unit(Point::new(1, 1), Some(UnitType::SmallTank.instance(&environment).set_owner_id(0).build_with_defaults()));
        map.set_unit(Point::new(2, 1), Some(UnitType::SmallTank.instance(&environment).set_owner_id(1).set_hp(1).build_with_defaults()));

        map.set_unit(Point::new(4, 4), Some(UnitType::SmallTank.instance(&environment).set_owner_id(1).set_hp(1).build_with_defaults()));

        map.set_details(Point::new(0, 4), vec![Detail::Skull(0.into(), UnitType::SmallTank)]);

        let settings = map.settings().unwrap();

        let mut settings = settings.clone();
        for player in &settings.players {
            assert!(player.get_commander_options().contains(&CommanderType::Zombie));
        }
        settings.fog_mode = FogMode::Constant(FogSetting::None);
        settings.players[0].set_commander(CommanderType::Zombie);
        let (mut server, _) = map.clone().game_server(&settings, || 0.);
        server.players.get_mut(0).unwrap().commander.charge = server.players.get_mut(0).unwrap().commander.get_max_charge();
        let unchanged = server.clone();
        let environment = server.environment().clone();
        // small power
        server.handle_command(Command::CommanderPowerSimple(1), || 0.).unwrap();
        assert_eq!(server.get_map().get_details(Point::new(0, 4)), Vec::new());
        assert_eq!(server.get_map().get_unit(Point::new(0, 4)), Some(&UnitType::SmallTank.instance(&environment).set_owner_id(0).set_hp(50).set_zombified(true).build_with_defaults()));
        server.handle_command(Command::UnitCommand(UnitCommand {
            unload_index: None,
            path: Path::new(Point::new(1, 1)),
            action: UnitAction::Attack(AttackVector::Direction(Direction6::D0)),
        }), || 0.).unwrap();
        assert_eq!(server.get_map().get_details(Point::new(2, 1)), vec![Detail::Skull(0.into(), UnitType::SmallTank)]);
        assert_eq!(server.get_map().get_unit(Point::new(2, 1)), None);
        // big power
        let mut server = unchanged.clone();
        server.handle_command(Command::CommanderPowerSimple(2), || 0.).unwrap();
        assert_eq!(server.get_map().get_details(Point::new(0, 4)), Vec::new());
        assert_eq!(server.get_map().get_unit(Point::new(0, 4)), Some(&UnitType::SmallTank.instance(&environment).set_owner_id(0).set_hp(50).set_zombified(true).build_with_defaults()));
        server.handle_command(Command::UnitCommand(UnitCommand {
            unload_index: None,
            path: Path::new(Point::new(1, 1)),
            action: UnitAction::Attack(AttackVector::Direction(Direction6::D0)),
        }), || 0.).unwrap();
        assert_eq!(server.get_map().get_details(Point::new(2, 1)), Vec::new());
        assert_eq!(server.get_map().get_unit(Point::new(2, 1)), Some(&UnitType::SmallTank.instance(&environment).set_owner_id(0).set_hp(50).set_zombified(true).build_with_defaults()));
    }

    #[test]
    fn simo() {
        let config = Arc::new(Config::test_config());
        let map = PointMap::new(6, 5, false);
        let map = WMBuilder::<Direction4>::new(map);
        let mut map = Map::new(map.build(), &config);
        let map_env = map.environment().clone();
        let arty_pos = Point::new(0, 1);
        map.set_unit(arty_pos, Some(UnitType::Artillery.instance(&map_env).set_owner_id(0).set_hp(50).build_with_defaults()));

        let target_close = Point::new(3, 1);
        map.set_unit(target_close, Some(UnitType::SmallTank.instance(&map_env).set_owner_id(1).build_with_defaults()));
        let target_far = Point::new(4, 1);
        map.set_unit(target_far, Some(UnitType::SmallTank.instance(&map_env).set_owner_id(1).build_with_defaults()));
        let target_farthest = Point::new(5, 1);
        map.set_unit(target_farthest, Some(UnitType::SmallTank.instance(&map_env).set_owner_id(1).build_with_defaults()));

        let settings = map.settings().unwrap();

        let mut settings = settings.clone();
        for player in &settings.players {
            assert!(player.get_commander_options().contains(&CommanderType::Simo));
        }
        settings.fog_mode = FogMode::Constant(FogSetting::None);
        settings.players[0].set_commander(CommanderType::Simo);
        let (mut server, _) = map.clone().game_server(&settings, || 0.);
        server.players.get_mut(0).unwrap().commander.charge = server.players.get_mut(0).unwrap().commander.get_max_charge();
        let unchanged = server.clone();

        // before chaos/order
        let arty = server.get_unit(arty_pos).unwrap();
        assert!(arty.shortest_path_to_attack(&server, &Path::new(arty_pos), None, target_close).is_some());
        assert!(arty.shortest_path_to_attack(&server, &Path::new(arty_pos), None, target_far).is_none());
        server.handle_command(Command::UnitCommand(UnitCommand {
            unload_index: None,
            path: Path::new(arty_pos),
            action: UnitAction::Attack(AttackVector::Point(target_close)),
        }), || 0.).unwrap();
        assert!(server.get_unit(target_close).unwrap().get_hp() < 100);
        assert_eq!(server.get_unit(target_far).unwrap().get_hp(), 100);

        // embrace chaos
        let mut server = unchanged.clone();
        server.handle_command(Command::CommanderPowerSimple(1), || 0.).unwrap();
        server.handle_command(Command::CommanderPowerSimple(4), || 0.).err().unwrap();
        server.handle_command(Command::CommanderPowerSimple(5), || 0.).err().unwrap();
        let arty = server.get_unit(arty_pos).unwrap();
        assert!(arty.shortest_path_to_attack(&server, &Path::new(arty_pos), None, target_close).is_some());
        assert!(arty.shortest_path_to_attack(&server, &Path::new(arty_pos), None, target_far).is_none());
        server.handle_command(Command::UnitCommand(UnitCommand {
            unload_index: None,
            path: Path::new(arty_pos),
            action: UnitAction::Attack(AttackVector::Point(target_close)),
        }), || 0.).unwrap();
        let hp_close = server.get_unit(target_close).unwrap().get_hp();
        let hp_far = server.get_unit(target_far).unwrap().get_hp();
        assert!(hp_far < 100);
        assert!(hp_close < hp_far);

        // chaos power
        let mut server = unchanged.clone();
        server.handle_command(Command::CommanderPowerSimple(1), || 0.).unwrap();
        server.handle_command(Command::CommanderPowerSimple(3), || 0.).unwrap();
        //let arty = server.get_unit(arty_pos).unwrap();
        server.handle_command(Command::UnitCommand(UnitCommand {
            unload_index: None,
            path: Path::new(arty_pos),
            action: UnitAction::Attack(AttackVector::Point(target_far)),
        }), || 0.).err().expect("range shouldn't be increased");
        server.handle_command(Command::UnitCommand(UnitCommand {
            unload_index: None,
            path: Path::new(arty_pos),
            action: UnitAction::Attack(AttackVector::Point(target_close)),
        }), || 0.).unwrap();
        assert!(server.get_unit(target_close).unwrap().get_hp() < hp_close);
        assert!(server.get_unit(target_far).unwrap().get_hp() < hp_far);
        assert_eq!(server.get_unit(target_farthest).unwrap().get_hp(), 100);

        // order power (small)
        let mut server = unchanged.clone();
        server.handle_command(Command::CommanderPowerSimple(2), || 0.).unwrap();
        server.handle_command(Command::CommanderPowerSimple(3), || 0.).err().unwrap();
        server.handle_command(Command::CommanderPowerSimple(4), || 0.).unwrap();
        let arty = server.get_unit(arty_pos).unwrap();
        assert!(arty.shortest_path_to_attack(&server, &Path::new(arty_pos), None, target_far).is_some());
        assert!(arty.shortest_path_to_attack(&server, &Path::new(arty_pos), None, target_farthest).is_none());
        server.handle_command(Command::UnitCommand(UnitCommand {
            unload_index: None,
            path: Path::new(arty_pos),
            action: UnitAction::Attack(AttackVector::Point(target_far)),
        }), || 0.).unwrap();
        assert_eq!(server.get_unit(target_close).unwrap().get_hp(), 100);
        assert!(server.get_unit(target_far).unwrap().get_hp() < 100);

        // order power (big)
        let mut server = unchanged.clone();
        server.handle_command(Command::CommanderPowerSimple(2), || 0.).unwrap();
        server.handle_command(Command::CommanderPowerSimple(5), || 0.).unwrap();
        let arty = server.get_unit(arty_pos).unwrap();
        assert!(arty.shortest_path_to_attack(&server, &Path::new(arty_pos), None, target_close).is_some());
        assert!(arty.shortest_path_to_attack(&server, &Path::new(arty_pos), None, target_far).is_some());
        assert!(arty.shortest_path_to_attack(&server, &Path::new(arty_pos), None, target_farthest).is_some());
        server.handle_command(Command::UnitCommand(UnitCommand {
            unload_index: None,
            path: Path::new(arty_pos),
            action: UnitAction::Attack(AttackVector::Point(target_farthest)),
        }), || 0.).unwrap();
        assert_eq!(server.get_unit(target_far).unwrap().get_hp(), 100);
        assert!(server.get_unit(target_farthest).unwrap().get_hp() < 100);
    }

    #[test]
    fn vlad() {
        let config = Arc::new(Config::test_config());
        let map = PointMap::new(6, 5, false);
        let map = WMBuilder::<Direction4>::new(map);
        let mut map = Map::new(map.build(), &config);
        let map_env = map.environment().clone();
        let arty_pos = Point::new(0, 1);
        map.set_unit(arty_pos, Some(UnitType::Artillery.instance(&map_env).set_owner_id(0).set_hp(50).build_with_defaults()));

        let target_close = Point::new(3, 1);
        map.set_unit(target_close, Some(UnitType::SmallTank.instance(&map_env).set_owner_id(1).set_hp(50).build_with_defaults()));
        map.set_terrain(target_close, TerrainType::Flame.instance(&map_env).build_with_defaults());
        let target_far = Point::new(5, 4);
        map.set_unit(target_far, Some(UnitType::SmallTank.instance(&map_env).set_owner_id(1).set_hp(50).build_with_defaults()));

        let mut settings = map.settings().unwrap();
        for player in &settings.players {
            assert!(player.get_commander_options().contains(&CommanderType::Vlad));
        }
        settings.fog_mode = FogMode::Constant(FogSetting::None);
        settings.players[0].set_commander(CommanderType::Vlad);
        let (mut server, _) = map.clone().game_server(&settings, || 0.);

        // d2d daylight
        server.handle_command(Command::UnitCommand(UnitCommand {
            unload_index: None,
            path: Path::new(arty_pos),
            action: UnitAction::Attack(AttackVector::Point(target_close)),
        }), || 0.).unwrap();
        assert_eq!(server.get_unit(arty_pos).unwrap().get_hp(), 50);

        // d2d night
        settings.fog_mode = FogMode::Constant(FogSetting::Sharp(0));
        let (mut server, _) = map.clone().game_server(&settings, || 0.);
        server.players.get_mut(0).unwrap().commander.charge = server.players.get_mut(0).unwrap().commander.get_max_charge();
        let unchanged = server.clone();
        server.handle_command(Command::UnitCommand(UnitCommand {
            unload_index: None,
            path: Path::new(arty_pos),
            action: UnitAction::Attack(AttackVector::Point(target_close)),
        }), || 0.).unwrap();
        assert!(server.get_unit(arty_pos).unwrap().get_hp() > 50);

        // small power
        let mut server = unchanged.clone();
        server.handle_command(Command::CommanderPowerSimple(1), || 0.).unwrap();
        assert_eq!(server.get_unit(arty_pos).unwrap().get_hp(), 50);
        assert!(server.get_unit(target_close).unwrap().get_hp() < 50);
        assert_eq!(server.get_unit(target_far).unwrap().get_hp(), 50);

        // big power
        let mut server = unchanged.clone();
        server.handle_command(Command::CommanderPowerSimple(2), || 0.).unwrap();
        assert!(server.get_unit(arty_pos).unwrap().get_hp() > 50);
        assert!(server.get_unit(target_close).unwrap().get_hp() < 50);
        assert_eq!(server.get_unit(target_far).unwrap().get_hp(), 50);
    }
}
