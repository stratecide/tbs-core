use crate::map::direction::Direction;
use crate::map::map::Map;

use super::*;

use zipper::*;
use zipper::zipper_derive::*;



pub const MAX_CHARGE: u8 = 63;

#[derive(Debug, PartialEq, Eq, Clone, Zippable, Hash)]
#[zippable(bits=1)]
pub enum MaybeMercenary {
    None,
    Some {
        mercenary: Mercenaries,
        origin: Option<Point>
    },    
}
impl MaybeMercenary {
    pub fn and_then<T, F: FnOnce(&Mercenaries, &Option<Point>) -> Option<T>>(&self, f: F) -> Option<T> {
        match self {
            Self::None => None,
            Self::Some { mercenary, origin } => f(mercenary, origin),
        }
    }
    pub fn then<F: FnOnce(&mut Mercenaries, &mut Option<Point>)>(&mut self, f: F) {
        match self {
            Self::None => (),
            Self::Some { mercenary, origin } => f(mercenary, origin),
        }
    }
    pub fn get_origin(&self) -> Option<Point> {
        self.and_then(|_, origin| *origin)
    }
    pub fn own_movement_bonus(&self) -> MovementPoints {
        self.and_then(|m, _| Some(m.own_movement_bonus())).unwrap_or(MovementPoints::from(0.))
    }
    pub fn own_defense_bonus(&self) -> f32 {
        self.and_then(|m, _| Some(m.own_defense_bonus())).unwrap_or(0.)
    }
    pub fn own_attack_bonus(&self) -> f32 {
        self.and_then(|m, _| Some(m.own_attack_bonus())).unwrap_or(0.)
    }
    pub fn add_options_after_path<D: Direction>(&self, unit: &NormalUnit, game: &Game<D>, path: &Path<D>, available_funds: i32, options: &mut Vec<UnitAction<D>>) {
        let player = game.get_owning_player(unit.owner).unwrap();
        let destination = path.end(game.get_map()).unwrap();
        match self {
            Self::None => {
                if game.can_buy_merc_at(player, destination) {
                    for merc in game.available_mercs(player) {
                        if merc.price(game, unit).filter(|price| *price as i32 <= available_funds).is_some() {
                            options.push(UnitAction::BuyMercenary(merc));
                        }
                    }
                }
            }
            Self::Some { mercenary, .. } => {
                mercenary.add_options_after_path(unit, game, path, options);
            }
        }
    }
    pub fn power_active(&self) -> bool {
        self.and_then(|m, _| Some(m.power_active())).unwrap_or(false)
    }
}

#[derive(Debug, PartialEq, Eq, Clone, Zippable, Hash)]
#[zippable(bits = 6)]
pub enum Mercenaries {
    EarlGrey(U<10>, bool),
}
impl Mercenaries {
    pub fn name(&self) -> &'static str {
        match self {
            Self::EarlGrey(_, _) => "Earl Grey",
        }
    }
    // should NEVER be higher than MAX_CHARGE
    pub fn max_charge(&self) -> u8 {
        if self.power_active() {
            return 0;
        }
        match self {
            Self::EarlGrey(charge, _) => charge.max_value() as u8,
        }
    }
    pub fn charge(&self) -> u8 {
        match self {
            Self::EarlGrey(charge, _) => **charge as u8,
        }
    }
    pub fn add_charge(&mut self, add: i8) {
        let value = (self.charge() as i8 + add).min(self.max_charge() as i8).max(0) as u8;
        match self {
            Mercenaries::EarlGrey(charge, _) => *charge = value.try_into().unwrap(),
        }
    }
    pub fn power_active(&self) -> bool {
        match self {
            Mercenaries::EarlGrey(_, power_active) => *power_active,
        }
    }
    pub fn power_active_mut<'a>(&'a mut self) -> Option<&'a mut bool> {
        match self {
            Mercenaries::EarlGrey(_, power_active) => Some(power_active),
        }
    }
    pub fn can_use_simple_power<D: Direction>(&self, _game: &Game<D>, _pos: Point) -> bool {
        match self {
            Mercenaries::EarlGrey(charge, false) => self.charge() >= self.max_charge(),
            _ => false,
        }
    }
    pub fn add_options_after_path<D: Direction>(&self, _unit: &NormalUnit, _game: &Game<D>, path: &Path<D>, options: &mut Vec<UnitAction<D>>) {
        match self {
            Mercenaries::EarlGrey(charge, false) => {
                if path.steps.len() == 0 && **charge >= charge.max_value() {
                    options.insert(0, UnitAction::MercenaryPowerSimple);
                }
            }
            _ => {}
        }
    }

    pub fn own_defense_bonus(&self) -> f32 {
        0.2
    }

    pub fn defense_bonus<D: Direction>(&self, _defender: &UnitType<D>, _is_counter: bool) -> f32 {
        match &self {
            _ => 0.1,
        }
    }

    pub fn own_attack_bonus(&self) -> f32 {
        match self {
            Mercenaries::EarlGrey(_, false) => 0.5,
            Mercenaries::EarlGrey(_, true) => 0.8,
        }
    }

    pub fn attack_bonus(&self, _attacker: &NormalUnit, _is_counter: bool) -> f32 {
        match &self {
            Mercenaries::EarlGrey(_, false) => 0.3,
            Mercenaries::EarlGrey(_, true) => 0.5,
            _ => 0.1,
        }
    }

    pub fn own_movement_bonus(&self) -> MovementPoints {
        match self {
            Mercenaries::EarlGrey(_, true) => MovementPoints::from(1.),
            _ => MovementPoints::from(0.)
        }
    }

    pub fn aura_range(&self) -> u8 {
        match self {
            Mercenaries::EarlGrey(_, _) => 1,
        }
    }

    pub fn in_range<D: Direction>(&self, map: &Map<D>, position: Point, target: Point) -> bool {
        self.aura(map, position).contains(&target)
    }

    pub fn aura<D: Direction>(&self, map: &Map<D>, position: Point) -> HashSet<Point> {
        let mut result = HashSet::new();
        result.insert(position.clone());
        for layer in map.range_in_layers(position, self.aura_range() as usize) {
            for (p, _, _) in layer {
                result.insert(p);
            }
        }
        result
    }

    pub fn price<D: Direction>(&self, _game: &Game<D>, unit: &NormalUnit) -> Option<u16> {
        Some(unit.typ.value())
    }

    pub fn build_option(&self) -> MercenaryOption {
        match self {
            Mercenaries::EarlGrey(_, _) => MercenaryOption::EarlGrey,
        }
    }
}

#[derive(Debug, PartialEq, Eq, Clone, Zippable, Hash)]
#[zippable(bits = 6)]
pub enum MercenaryOption {
    EarlGrey,
}
impl MercenaryOption {
    pub fn mercenary(&self) -> Mercenaries {
        match self {
            MercenaryOption::EarlGrey => Mercenaries::EarlGrey(0.try_into().unwrap(), false),
        }
    }
    pub fn price<D: Direction>(&self, _game: &Game<D>, unit: &NormalUnit) -> Option<u16> {
        Some(unit.typ.value() / 2)
    }

}
