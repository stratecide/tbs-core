use zipper::*;

use crate::config::parse::FromConfig;
use crate::map::direction::Direction;
use crate::config::environment::Environment;
use super::unit::UnitBuilder;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct UnitType(pub usize);

impl FromConfig for UnitType {
    fn from_conf<'a>(s: &'a str, loader: &mut crate::config::file_loader::FileLoader) -> Result<(Self, &'a str), crate::config::ConfigParseError> {
        let (base, s) = crate::config::parse::string_base(s);
        match loader.unit_types.iter().position(|name| name.as_str() == base) {
            Some(i) => Ok((Self(i), s)),
            None => Err(crate::config::ConfigParseError::MissingUnit(base.to_string()))
        }
    }
}

impl SupportedZippable<&Environment> for UnitType {
    fn export(&self, zipper: &mut Zipper, environment: &Environment) {
        let bits = bits_needed_for_max_value(environment.config.unit_count() as u32 - 1);
        zipper.write_u32(self.0 as u32, bits);
    }
    fn import(unzipper: &mut Unzipper, environment: &Environment) -> Result<Self, ZipperError> {
        let bits = bits_needed_for_max_value(environment.config.unit_count() as u32 - 1);
        let index = unzipper.read_u32(bits)? as usize;
        if index >= environment.config.unit_count() {
            return Err(ZipperError::EnumOutOfBounds(format!("UnitType index {}", index)))
        }
        Ok(Self(index))
    }
}

impl UnitType {
    pub fn base_cost(&self, environment: &Environment) -> i32 {
        environment.config.base_value(*self)
    }

    pub fn instance<D: Direction>(&self, environment: &Environment) -> UnitBuilder<D> {
        UnitBuilder::new(environment, *self)
    }
}

#[cfg(test)]
mod helper {
    use super::UnitType;

    macro_rules! ut {
        ($fun: ident, $id: expr) => {
            pub fn $fun() -> Self {
                Self($id)
            }
        };
    }

    impl UnitType {
        ut!(marine, 0);
        ut!(sniper, 1);
        ut!(bazooka, 2);
        ut!(magnet, 3);
        ut!(convoy, 5);
        ut!(artillery, 6);
        ut!(small_tank, 7);
        ut!(drone_boat, 13);
        ut!(destroyer, 17);
        ut!(war_ship, 18);
        ut!(transport_heli, 21);
        ut!(attack_heli, 22);
        ut!(light_drone, 26);
        ut!(factory, 27);
        ut!(hero_crystal, 28);
        ut!(pyramid, 29);
        ut!(life_crystal, 34);
        ut!(tentacle, 35);
        ut!(puffer_fish, 36);
        ut!(pawn, 37);
        ut!(rook, 38);
        ut!(bishop, 39);
        ut!(knight, 40);
        ut!(queen, 41);
        ut!(king, 42);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::config::Config;

    #[test]
    fn helpers_are_correct() {
        let config = Config::test_config();
        assert_eq!(config.unit_name(UnitType::marine()), "Marine");
        assert_eq!(config.unit_name(UnitType::sniper()), "Sniper");
        assert_eq!(config.unit_name(UnitType::bazooka()), "Bazooka");
        assert_eq!(config.unit_name(UnitType::magnet()), "Magnet");
        assert_eq!(config.unit_name(UnitType::convoy()), "Convoy");
        assert_eq!(config.unit_name(UnitType::small_tank()), "SmallTank");
        assert_eq!(config.unit_name(UnitType::drone_boat()), "DroneBoat");
        assert_eq!(config.unit_name(UnitType::destroyer()), "Destroyer");
        assert_eq!(config.unit_name(UnitType::war_ship()), "WarShip");
        assert_eq!(config.unit_name(UnitType::transport_heli()), "TransportHeli");
        assert_eq!(config.unit_name(UnitType::attack_heli()), "AttackHeli");
        assert_eq!(config.unit_name(UnitType::light_drone()), "LightDrone");
        assert_eq!(config.unit_name(UnitType::factory()), "Factory");
        assert_eq!(config.unit_name(UnitType::hero_crystal()), "HeroCrystal");
        assert_eq!(config.unit_name(UnitType::pyramid()), "Pyramid");
        assert_eq!(config.unit_name(UnitType::life_crystal()), "LifeCrystal");
        assert_eq!(config.unit_name(UnitType::tentacle()), "Tentacle");
        assert_eq!(config.unit_name(UnitType::puffer_fish()), "PufferFish");
        assert_eq!(config.unit_name(UnitType::pawn()), "Pawn");
        assert_eq!(config.unit_name(UnitType::rook()), "Rook");
        assert_eq!(config.unit_name(UnitType::bishop()), "Bishop");
        assert_eq!(config.unit_name(UnitType::knight()), "Knight");
        assert_eq!(config.unit_name(UnitType::queen()), "Queen");
        assert_eq!(config.unit_name(UnitType::king()), "King");
    }
}
