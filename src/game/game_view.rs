use std::collections::{HashMap, HashSet};

use interfaces::ClientPerspective;
use rhai::Scope;

use crate::config::environment::Environment;
use crate::details::Detail;
use crate::handle::BorrowedHandle;
use crate::map::map::NeighborMode;
use crate::map::point::Point;
use crate::map::wrapping_map::{Distortion, OrientedPoint, WrappingMap};
use crate::player::Player;
use crate::terrain::terrain::Terrain;
use crate::units::hero::{HeroInfluence, HeroType};
use crate::units::unit::Unit;

use super::fog::{FogIntensity, FogSetting};
use super::Direction;


pub trait GameView<D: Direction>: Send + Sync {
    fn environment(&self) -> Environment;
    fn all_points(&self) -> Vec<Point>;
    fn get_terrain(&self, p: Point) -> Option<Terrain>;
    fn get_details(&self, p: Point) -> Vec<Detail<D>>;
    fn get_unit(&self, p: Point) -> Option<Unit<D>>;

    fn add_self_to_scope(&self, scope: &mut Scope<'_>);

    fn wrapping_logic(&self) -> BorrowedHandle<WrappingMap<D>>;

    // TODO: remove a few of these methods from the trait and turn them into functions that take dyn Gameview as parameter

    /**
     * checks the pipe at dp.point for whether it can be entered by dp.direction and if true, returns the position of the next pipe tile
     * returns None if no pipe is at the given location, for example because the previous pipe tile was an exit
     */
    fn next_pipe_tile(&self, point: Point, direction: D) -> Option<(Point, Distortion<D>)>;

    /**
     * the returned Distortion has to be applied to 'd' in order to
     * keep moving in the same direction
     */
    fn get_neighbor(&self, p: Point, d: D) -> Option<(Point, Distortion<D>)>;
    fn get_neighbors(&self, p: Point, mode: NeighborMode) -> Vec<OrientedPoint<D>>;
    fn width_search(&self, start: Point, f: Box<&mut dyn FnMut(Point) -> bool>) -> HashSet<Point>;
    fn range_in_layers(&self, center: Point, range: usize) -> Vec<HashSet<Point>>;

    // the result includes start, the OrientedPoints point towards the next point
    // the result may be shorter than the requested length if not enough points could be found
    fn get_line(&self, start: Point, d: D, length: usize, mode: NeighborMode) -> Vec<OrientedPoint<D>>;

    fn get_owning_player(&self, owner: i8) -> Option<Player>;
    fn get_team(&self, owner: i8) -> ClientPerspective;
    fn is_foggy(&self) -> bool {
        self.get_fog_setting().intensity() != FogIntensity::TrueSight
    }
    fn get_fog_setting(&self) -> FogSetting;
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
    fn additional_hero_influence_at(&self, _point: Point, _only_owner_id: i8) -> Option<Vec<HeroInfluence<D>>> {
        None
    }
    fn additional_hero_influence_map(&self, _only_owner_id: i8) -> Option<HashMap<(Point, i8), Vec<HeroInfluence<D>>>> {
        None
    }

    // prevent infinite loop in Config
    fn get_unit_config_limit(&self) -> Option<usize>;
    fn set_unit_config_limit(&self, limit: Option<usize>);
    fn get_terrain_config_limit(&self) -> Option<usize>;
    fn set_terrain_config_limit(&self, limit: Option<usize>);
}

/*impl<'a, D: Direction, G: GameView<D>> GameView<D> for Box<&'a G> {
    fn get_owning_player(&self, owner: i8) -> Option<Player> {
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

    fn get_unit_config_limit(&self) -> Option<usize> {
        (**self).get_unit_config_limit()
    }
    fn set_unit_config_limit(&self, limit: Option<usize>) {
        (**self).set_unit_config_limit(limit);
    }
    fn get_terrain_config_limit(&self) -> Option<usize> {
        (**self).get_terrain_config_limit()
    }
    fn set_terrain_config_limit(&self, limit: Option<usize>) {
        (**self).set_terrain_config_limit(limit);
    }
}*/
