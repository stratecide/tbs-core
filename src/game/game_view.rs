use std::collections::{HashMap, HashSet};

use interfaces::game_interface::ClientPerspective;

use crate::map::map_view::MapView;
use crate::map::point::Point;
use crate::player::Player;
use crate::units::hero::{Hero, HeroType};
use crate::units::unit::Unit;

use super::fog::FogIntensity;
use super::Direction;


pub trait GameView<D: Direction>: MapView<D> {
    fn get_owning_player(&self, owner: i8) -> Option<&Player>;
    fn is_foggy(&self) -> bool {
        self.fog_intensity() != FogIntensity::TrueSight
    }
    fn fog_intensity(&self) -> FogIntensity;
    fn get_fog_at(&self, team: ClientPerspective, position: Point) -> FogIntensity;
    //fn get_fog(&self, position: Point) -> FogIntensity;

    fn available_heroes(&self, player: &Player) -> Vec<HeroType> {
        let mut used = HashSet::new();
        used.insert(HeroType::None);
        for p in self.all_points() {
            if let Some(unit) = self.get_unit(p) {
                if unit.get_owner_id() == player.get_owner_id() {
                    used.insert(unit.get_hero().typ());
                }
            }
        }
        self.environment().config.hero_types()
        .into_iter()
        .filter(|m| !used.contains(m))
        .cloned()
        .collect()
    }

    fn get_visible_unit(&self, team: ClientPerspective, p: Point) -> Option<Unit<D>>;
    fn additional_hero_influence_at(&self, _point: Point, _only_owner_id: i8) -> Option<Vec<(Unit<D>, Hero, Point, Option<usize>)>> {
        None
    }
    fn additional_hero_influence_map(&self, _only_owner_id: i8) -> Option<HashMap<(Point, i8), Vec<(Unit<D>, Hero, Point, Option<usize>)>>> {
        None
    }
}

impl<'a, D: Direction, G: GameView<D>> GameView<D> for Box<&'a G> {
    fn get_owning_player(&self, owner: i8) -> Option<&Player> {
        (**self).get_owning_player(owner)
    }

    fn fog_intensity(&self) -> FogIntensity {
        (**self).fog_intensity()
    }

    fn get_fog_at(&self, team: ClientPerspective, position: Point) -> FogIntensity {
        (**self).get_fog_at(team, position)
    }

    fn get_visible_unit(&self, team: ClientPerspective, p: Point) -> Option<Unit<D>> {
        (**self).get_visible_unit(team, p)
    }
}
