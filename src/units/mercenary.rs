use crate::map::direction::Direction;
use crate::map::map::Map;

use super::*;





#[derive(Debug, PartialEq, Clone)]
pub struct Mercenary {
    pub typ: Mercenaries,
    pub unit: NormalUnit,
    pub charge: u8,
}
impl Mercenary {
    pub fn get_armor(&self) -> (ArmorType, f32) {
        let (armor_type, mut factor) = self.unit.typ.get_armor();
        factor *= 1.2;
        (armor_type, factor)
    }
    pub fn range(&self) -> u8 {
        match self.typ {
            Mercenaries::EarlGrey => 1,
        }
    }
    pub fn in_range<D: Direction>(&self, map: &Map<D>, position: &Point, target: &Point) -> bool {
        self.aura(map, position).contains(target)
    }
    pub fn aura<D: Direction>(&self, map: &Map<D>, position: &Point) -> HashSet<Point> {
        let mut result = HashSet::new();
        result.insert(position.clone());
        for layer in range_in_layers(map, position, self.range() as usize) {
            for (p, _, _) in layer {
                result.insert(p);
            }
        }
        result
    }
    pub fn attack_bonus<D: Direction>(&self, attacker: &dyn NormalUnitTrait<D>, _is_counter: bool) -> f32 {
        if attacker.get_owner() != &self.unit.owner {
            return 1.0;
        }
        match &self.typ {
            Mercenaries::EarlGrey => 1.3,
            _ => 1.1,
        }
    }
    pub fn defense_bonus<D: Direction>(&self, defender: &UnitType<D>, _is_counter: bool) -> f32 {
        if defender.get_owner() != Some(&self.unit.owner) {
            return 1.0;
        }
        1.1
    }
}

impl<D: Direction> NormalUnitTrait<D> for Mercenary {
    fn as_trait(&self) -> &dyn NormalUnitTrait<D> {
        self
    }
    fn get_hp(&self) -> u8 {
        self.unit.hp
    }
    fn get_weapons(&self) -> Vec<(WeaponType, f32)> {
        let u: &dyn NormalUnitTrait<D> = self.unit.as_trait();
        u.get_weapons().into_iter().map(|(weapon, atk)| {
            let mut factor = 1.2;
            match (&self.typ, &weapon) {
                (Mercenaries::EarlGrey, _) => {
                    factor += 0.3;
                }
            }
            (weapon, atk * factor)
        }).collect()
    }
    fn get_owner(&self) -> &Owner {
        &self.unit.owner
    }
    fn get_team(&self, game: &Game<D>) -> Option<Team> {
        self.unit.get_team(game)
    }
    fn get_movement(&self) -> (MovementType, u8) {
        let u: &dyn NormalUnitTrait<D> = self.unit.as_trait();
        u.get_movement()
    }
    fn has_stealth(&self) -> bool {
        let u: &dyn NormalUnitTrait<D> = self.unit.as_trait();
        u.has_stealth()
    }
    fn shortest_path_to(&self, game: &Game<D>, start: &Point, path_so_far: &Vec<Point>, goal: &Point) -> Option<Vec<Point>> {
        self.unit.shortest_path_to(game, start, path_so_far, goal)
    }
    fn options_after_path(&self, game: &Game<D>, start: &Point, path: &Vec<Point>) -> Vec<UnitAction<D>> {
        self.unit.options_after_path(game, start, path)
    }
    fn shortest_path_to_attack(&self, game: &Game<D>, start: &Point, path_so_far: &Vec<Point>, goal: &Point) -> Option<Vec<Point>> {
        self.unit.shortest_path_to_attack(game, start, path_so_far, goal)
    }
    fn get_attack_type(&self) -> AttackType {
        self.unit.typ.get_attack_type()
    }
    fn is_position_targetable(&self, game: &Game<D>, target: &Point) -> bool {
        self.unit.is_position_targetable(game, target)
    }
    fn can_attack_unit_type(&self, game: &Game<D>, target: &UnitType<D>) -> bool {
        self.unit.can_attack_unit_type(game, target)
    }
    fn attackable_positions(&self, map: &Map<D>, position: &Point, moved: bool) -> HashSet<Point> {
        self.unit.attackable_positions(map, position, moved)
    }
    fn attack_splash(&self, map: &Map<D>, from: &Point, to: &AttackInfo<D>) -> Result<Vec<Point>, CommandError> {
        self.unit.attack_splash(map, from, to)
    }
    fn make_attack_info(&self, map: &Map<D>, from: &Point, to: &Point) -> Option<AttackInfo<D>> {
        self.unit.make_attack_info(map, from, to)
    }
}


#[derive(Debug, PartialEq, Clone)]
pub enum Mercenaries {
    EarlGrey,
}
impl Mercenaries {
    pub fn max_charge(&self) -> u8 {
        match self {
            Self::EarlGrey => 10,
        }
    }
}
