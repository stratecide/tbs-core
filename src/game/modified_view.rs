use rustc_hash::{FxHashMap as HashMap, FxHashSet as HashSet};
use std::sync::Arc;

use interfaces::ClientPerspective;

use crate::config::environment::Environment;
use crate::tokens::token::Token;
use crate::map::map::NeighborMode;
use crate::map::pipe::PipeState;
use crate::map::point::Point;
use crate::map::wrapping_map::*;
use crate::handle::BorrowedHandle;
use crate::player::Player;
use crate::terrain::terrain::Terrain;
use crate::units::hero::{Hero, HeroInfluence};
use crate::units::movement::{Path, PermanentBallast};
use crate::units::unit::Unit;

use super::fog::{FogIntensity, FogSetting};
use super::game_view::GameView;
use super::Direction;
use super::rhai_board::SharedGameView;


trait ModifiedView<D: Direction> {
    fn get_inner_view(&self) -> &dyn GameView<D>;

    fn environment(&self) -> Environment {
        self.get_inner_view().environment()
    }
    fn all_points(&self) -> Vec<Point> {
        self.get_inner_view().all_points()
    }
    fn get_terrain(&self, p: Point) -> Option<Terrain<D>> {
        self.get_inner_view().get_terrain(p)
    }
    fn get_tokens(&self, p: Point) -> Vec<Token<D>> {
        self.get_inner_view().get_tokens(p)
    }
    fn get_unit(&self, p: Point) -> Option<Unit<D>> {
        self.get_inner_view().get_unit(p)
    }

    fn next_pipe_tile(&self, point: Point, direction: D) -> Option<(Point, Distortion<D>)> {
        self.get_inner_view().next_pipe_tile(point, direction)
    }

    fn get_neighbor(&self, p: Point, d: D) -> Option<(Point, Distortion<D>)> {
        self.get_inner_view().get_neighbor(p, d)
    }
    fn get_neighbors(&self, p: Point, mode: NeighborMode) -> Vec<OrientedPoint<D>> {
        self.get_inner_view().get_neighbors(p, mode)
    }
    fn width_search(&self, start: Point, f: Box<&mut dyn FnMut(Point) -> bool>) -> HashSet<Point> {
        self.get_inner_view().width_search(start, f)
    }
    fn range_in_layers(&self, center: Point, range: usize) -> Vec<HashSet<Point>> {
        self.get_inner_view().range_in_layers(center, range)
    }

    fn get_line(&self, start: Point, d: D, length: usize, mode: NeighborMode) -> Vec<OrientedPoint<D>> {
        self.get_inner_view().get_line(start, d, length, mode)
    }

    fn current_owner(&self) -> i8 {
        self.get_inner_view().current_owner()
    }
    fn get_owning_player(&self, owner: i8) -> Option<Player> {
        self.get_inner_view().get_owning_player(owner)
    }
    fn get_fog_setting(&self) -> FogSetting {
        self.get_inner_view().get_fog_setting()
    }
    fn get_fog_at(&self, team: ClientPerspective, position: Point) -> FogIntensity {
        self.get_inner_view().get_fog_at(team, position)
    }

    fn get_visible_unit(&self, team: ClientPerspective, p: Point) -> Option<Unit<D>> {
        self.get_inner_view().get_visible_unit(team, p)
    }
    fn additional_hero_influence_at(&self, point: Point, only_owner_id: i8) -> Option<Vec<HeroInfluence<D>>> {
        self.get_inner_view().additional_hero_influence_at(point, only_owner_id)
    }
    fn additional_hero_influence_map(&self, only_owner_id: Option<i8>) -> Option<HashMap<(Point, i8), Vec<HeroInfluence<D>>>> {
        self.get_inner_view().additional_hero_influence_map(only_owner_id)
    }
}

macro_rules! impl_game_view {
    ($name: ty) => {
        impl<D: Direction> GameView<D> for $name {
            fn environment(&self) -> Environment {
                ModifiedView::environment(self)
            }
            fn all_points(&self) -> Vec<Point> {
                ModifiedView::all_points(self)
            }
            fn get_pipes(&self, p: Point) -> Vec<PipeState<D>> {
                self.get_inner_view().get_pipes(p)
            }
            fn get_terrain(&self, p: Point) -> Option<Terrain<D>> {
                ModifiedView::get_terrain(self, p)
            }
            fn get_tokens(&self, p: Point) -> Vec<Token<D>> {
                ModifiedView::get_tokens(self, p)
            }
            fn get_unit(&self, p: Point) -> Option<Unit<D>> {
                ModifiedView::get_unit(self, p)
            }

            fn as_shared(&self) -> SharedGameView<D> {
                SharedGameView(Arc::new(self.clone()))
            }

            fn wrapping_logic(&self) -> BorrowedHandle<WrappingMap<D>> {
                self.get_inner_view().wrapping_logic()
            }

            fn next_pipe_tile(&self, point: Point, direction: D) -> Option<(Point, Distortion<D>)> {
                ModifiedView::next_pipe_tile(self, point, direction)
            }

            fn get_neighbor(&self, p: Point, d: D) -> Option<(Point, Distortion<D>)> {
                ModifiedView::get_neighbor(self, p, d)
            }
            fn get_neighbors(&self, p: Point, mode: NeighborMode) -> Vec<OrientedPoint<D>> {
                ModifiedView::get_neighbors(self, p, mode)
            }
            fn width_search(&self, start: Point, f: Box<&mut dyn FnMut(Point) -> bool>) -> HashSet<Point> {
                ModifiedView::width_search(self, start, f)
            }
            fn range_in_layers(&self, center: Point, range: usize) -> Vec<HashSet<Point>> {
                ModifiedView::range_in_layers(self, center, range)
            }

            fn get_line(&self, start: Point, d: D, length: usize, mode: NeighborMode) -> Vec<OrientedPoint<D>> {
                ModifiedView::get_line(self, start, d, length, mode)
            }

            fn current_owner(&self) -> i8 {
                ModifiedView::current_owner(self)
            }
            fn get_owning_player(&self, owner: i8) -> Option<Player> {
                ModifiedView::get_owning_player(self, owner)
            }
            fn get_team(&self, owner: i8) -> ClientPerspective {
                self.get_inner_view().get_team(owner)
            }
            fn get_fog_setting(&self) -> FogSetting {
                ModifiedView::get_fog_setting(self)
            }
            fn get_fog_at(&self, team: ClientPerspective, position: Point) -> FogIntensity {
                ModifiedView::get_fog_at(self, team, position)
            }

            fn get_visible_unit(&self, team: ClientPerspective, p: Point) -> Option<Unit<D>> {
                ModifiedView::get_visible_unit(self, team, p)
            }
            fn additional_hero_influence_at(&self, point: Point, only_owner_id: i8) -> Option<Vec<HeroInfluence<D>>> {
                ModifiedView::additional_hero_influence_at(self, point, only_owner_id)
            }
            fn additional_hero_influence_map(&self, only_owner_id: Option<i8>) -> Option<HashMap<(Point, i8), Vec<HeroInfluence<D>>>> {
                ModifiedView::additional_hero_influence_map(self, only_owner_id)
            }

            // prevent infinite loop in Config
            fn get_unit_config_limit(&self) -> Option<usize> {
                self.get_inner_view().get_unit_config_limit()
            }
            fn set_unit_config_limit(&self, limit: Option<usize>) {
                self.get_inner_view().set_unit_config_limit(limit)
            }
            fn get_terrain_config_limit(&self) -> Option<usize> {
                self.get_inner_view().get_terrain_config_limit()
            }
            fn set_terrain_config_limit(&self, limit: Option<usize>) {
                self.get_inner_view().set_terrain_config_limit(limit)
            }
        }
    };
}

#[derive(Clone)]
pub(crate) struct IgnoreUnits<D: Direction>(SharedGameView<D>);

impl<D: Direction> IgnoreUnits<D> {
    pub fn new(base: &impl GameView<D>) -> Self {
        Self (base.as_shared())
    }
}

impl<D: Direction> ModifiedView<D> for IgnoreUnits<D> {
    fn get_inner_view(&self) -> &dyn GameView<D> {
        &*self.0.0
    }

    fn get_unit(&self, _: Point) -> Option<Unit<D>> {
        None
    }
    fn get_visible_unit(&self, _: ClientPerspective, _: Point) -> Option<Unit<D>> {
        None
    }
}

impl_game_view!(IgnoreUnits<D>);

/**
 * allows removing, adding, moving of units
 * used in movement planning, finding attack-vectors
 */
#[derive(Clone)]
pub struct UnitMovementView<D: Direction> {
    base: SharedGameView<D>,
    units: HashMap<Point, Option<Unit<D>>>,
    players: HashMap<i8, Player>,
}

impl<D: Direction> UnitMovementView<D> {
    pub fn new(base: &impl GameView<D>) -> Self {
        Self {
            base: base.as_shared(),
            units: HashMap::default(),
            players: HashMap::default(),
        }
    }

    pub fn remove_unit(&mut self, pos: Point, unload_index: Option<usize>) -> Option<Unit<D>> {
        if let Some(unit) = ModifiedView::get_unit(self, pos) {
            if let Some(index) = unload_index {
                if unit.get_transported().len() > index {
                    let mut unit = unit.clone();
                    let u = unit.get_transported_mut().remove(index);
                    self.units.insert(pos, Some(unit));
                    Some(u)
                } else {
                    None
                }
            } else {
                let result = Some(unit.clone());
                self.units.insert(pos, None);
                result
            }
        } else {
            None
        }
    }

    pub fn put_unit(&mut self, pos: Point, unit: Unit<D>) {
        self.units.insert(pos, Some(unit));
    }

    pub fn unit_path_without_placing(&mut self, unload_index: Option<usize>, path: &Path<D>) -> Option<(Point, Unit<D>)> {
        if let Some(mut unit) = self.remove_unit(path.start, unload_index) {
            // TODO: update fog, funds after path, ...
            // would be better to somehow wrap EventHandler, i guess?
            let (end, _) = path.end(self).unwrap();
            unit.transformed_by_path(self, path);
            Some((end, unit))
        } else {
            None
        }
    }
}

impl<D: Direction> ModifiedView<D> for UnitMovementView<D> {
    fn get_inner_view(&self) -> &dyn GameView<D> {
        &**self.base
    }

    fn get_unit(&self, p: Point) -> Option<Unit<D>> {
        if let Some(entry) = self.units.get(&p) {
            entry.clone()
        } else {
            self.get_inner_view().get_unit(p)
        }
    }

    fn get_owning_player(&self, owner: i8) -> Option<Player> {
        self.players.get(&owner).cloned().or(self.get_inner_view().get_owning_player(owner))
    }

    fn get_visible_unit(&self, team: ClientPerspective, p: Point) -> Option<Unit<D>> {
        ModifiedView::get_unit(self, p)
        .and_then(|u| {
            // use base's fog instead of self.get_fog_at
            // maybe it should be possible to predict the fog (i.e. modify fog in this view)
            // but that shouldn't influence the output of this method because
            // while it can be predicted where fog will be lifted, it can't be predicted which units are revealed
            u.fog_replacement(self, p, self.get_inner_view().get_fog_at(team, p))
        })
    }
}

impl_game_view!(UnitMovementView<D>);

#[derive(Clone)]
pub(crate) struct MovingHeroView<D: Direction> {
    map: UnitMovementView<D>,
    hero_pos: Option<Point>,
    hero_unit: Unit<D>,
    transporter: Option<(Unit<D>, usize)>,
    round: usize,
}

impl<D: Direction> MovingHeroView<D> {
    pub fn new(map: &impl GameView<D>, unit: &Unit<D>, unit_origin: Option<(Point, Option<usize>)>) -> Self {
        let mut map = UnitMovementView::new(map);
        let mut transporter = None;
        if let Some((pos, unload_index)) = unit_origin {
            map.remove_unit(pos, unload_index);
            if let Some(unload_index) = unload_index {
                transporter = Some((ModifiedView::get_unit(&map, pos).unwrap().clone(), unload_index));
            }
        }
        Self {
            map,
            hero_pos: None,
            hero_unit: unit.clone(),
            transporter,
            round: 0,
        }
    }

    pub fn update_hero(&mut self, pos: Point, permanent: &PermanentBallast<D>, round: usize) {
        self.hero_pos = Some(pos);
        permanent.update_unit(&mut self.hero_unit);
        self.round = round;
    }

    pub fn get_hero(&self) -> &Unit<D> {
        &self.hero_unit
    }

    pub fn get_transporter(&self) -> Option<(&Unit<D>, usize)> {
        self.transporter.as_ref()
        .filter(|_| self.round == 0)
        .map(|(u, i)| (u, *i))
    }
}

impl<D: Direction> ModifiedView<D> for MovingHeroView<D> {
    fn get_inner_view(&self) -> &dyn GameView<D> {
        &self.map
    }

    fn additional_hero_influence_at(&self, point: Point, only_owner_id: i8) -> Option<Vec<HeroInfluence<D>>> {
        if !self.hero_unit.is_hero() || self.hero_unit.get_owner_id() != only_owner_id {
            return None;
        }
        let pos = self.hero_pos?;
        let unit = self.hero_unit.clone();
        if let Some(strength) = Hero::aura(self, &unit, pos, self.get_transporter()).get(&point) {
            let hero = unit.get_hero().unwrap().clone();
            Some(vec![(unit, hero, pos, self.get_transporter().map(|(_, unload_index)| unload_index), *strength as u8)])
        } else {
            None
        }
    }

    fn additional_hero_influence_map(&self, only_owner_id: Option<i8>) -> Option<HashMap<(Point, i8), Vec<HeroInfluence<D>>>> {
        if !self.hero_unit.is_hero() || only_owner_id.is_some() && Some(self.hero_unit.get_owner_id()) != only_owner_id {
            return None;
        }
        let pos = self.hero_pos?;
        let unit = self.hero_unit.clone();
        let mut result: HashMap<(Point, i8), Vec<HeroInfluence<D>>> = HashMap::default();
        let hero = unit.get_hero().unwrap();
        for (p, strength) in Hero::aura(self, &unit, pos, self.get_transporter()) {
            let key = (p, unit.get_owner_id());
            result.insert(key, vec![(unit.clone(), hero.clone(), p, self.get_transporter().map(|(_, unload_index)| unload_index), strength as u8)]);
        }
        Some(result)
    }
}

impl_game_view!(MovingHeroView<D>);

impl<D: Direction> ModifiedView<D> for Arc<dyn GameView<D>> {
    fn get_inner_view(&self) -> &dyn GameView<D> {
        &**self
    }
}

impl_game_view!(Arc<dyn GameView<D>>);
