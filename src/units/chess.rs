use std::collections::HashSet;
use std::fmt;

use crate::game::game::Game;
use crate::map::direction::Direction;
use crate::map::map::NeighborMode;
use crate::map::point::Point;
use crate::map::point_map;
use crate::map::wrapping_map::OrientedPoint;
use crate::player::*;
use crate::terrain::Terrain;

use super::*;

use zipper::*;
use zipper::zipper_derive::*;


#[derive(Debug, Zippable)]
#[zippable(bits = 6)]
pub enum ChessCommand<D: Direction> {
    Pawn(LVec::<PathStep::<D>, 2>, PawnUpgrade),
    Rook(D, U16::<{point_map::MAX_AREA as u16}>, bool),
    Bishop(D, U16::<{point_map::MAX_AREA as u16}>),
    Knight(PathStep::<D>),
    King(PathStep::<D>),
}
impl<D: Direction> ChessCommand<D> {
    pub fn convert(self, start: Point, unit: &ChessUnit<D>, handler: &mut EventHandler<D>) -> Result<(), CommandError> {
        //println!("ChessCommand {:?}", self);
        let team = match handler.get_game().get_team(Some(unit.owner)) {
            ClientPerspective::Neutral => panic!("Game with chess piece that doesn't belong to a team"),
            ClientPerspective::Team(team) => team,
        };
        let mut validated_path = None;
        match (&self, &unit.typ) {
            (Self::Pawn(path, _), ChessUnits::Pawn(_, _, _)) => {
                unit.all_possible_paths(handler.get_game(), start, true, |_, steps| {
                    if path[..] == steps[..] {
                        validated_path = Some(steps.clone());
                        PathSearchFeedback::Found
                    } else {
                        // could reject some, but not worth the effort
                        PathSearchFeedback::Continue
                    }
                });
            }
            (Self::Rook(dir, distance, _), ChessUnits::Rook(_) | ChessUnits::Queen) => {
                unit.all_possible_paths(handler.get_game(), start, true, |_, steps| {
                    match steps[0] {
                        PathStep::Dir(d) if d == *dir => {
                            if steps.len() == **distance as usize {
                                validated_path = Some(steps.clone());
                                PathSearchFeedback::Found
                            } else {
                                PathSearchFeedback::Continue
                            }
                        }
                        _ => PathSearchFeedback::Rejected,
                    }
                });
            }
            (Self::Bishop(dir, distance), ChessUnits::Bishop | ChessUnits::Queen) => {
                unit.all_possible_paths(handler.get_game(), start, true, |_, steps| {
                    match steps[0] {
                        PathStep::Diagonal(d) if d == *dir => {
                            if steps.len() == **distance as usize {
                                validated_path = Some(steps.clone());
                                PathSearchFeedback::Found
                            } else {
                                PathSearchFeedback::Continue
                            }
                        }
                        _ => PathSearchFeedback::Rejected,
                    }
                });
            }
            (Self::Knight(step), ChessUnits::Knight) | (Self::King(step), ChessUnits::King(_)) => {
                unit.all_possible_paths(handler.get_game(), start, true, |_, steps| {
                    if vec![step.clone()] == *steps {
                        validated_path = Some(steps.clone());
                        PathSearchFeedback::Found
                    } else {
                        PathSearchFeedback::Rejected
                    }
                });
            }
            _ => return Err(CommandError::UnitTypeWrong)
        };
        if validated_path.is_none() {
            return Err(CommandError::InvalidPath);
        }
        let validated_path = validated_path.unwrap();
        let mut path = vec![];
        unit.all_possible_paths(handler.get_game(), start, false, |_, steps| {
            if steps.len() > validated_path.len() || steps[..] != validated_path[..steps.len()] {
                PathSearchFeedback::Rejected
            } else {
                path = steps.clone();
                if path == validated_path {
                    PathSearchFeedback::Found
                } else {
                    PathSearchFeedback::Continue
                }
            }
        });
        let path = Path {
            start,
            steps: path.try_into().unwrap(),
        };

        let end = path.end(handler.get_map())?;
        let mut recalculate_fog = false;
        if let Some(other) = handler.get_map().get_unit(end) {
            recalculate_fog = true;
            handler.add_event(Event::UnitDeath(end, other.clone()));
        }
        handler.add_event(Event::UnitPath(Some(None), path.clone(), Some(false), UnitType::Chess::<D>(unit.clone())));
        let perspective = ClientPerspective::Team(team);
        if handler.get_game().is_foggy() {
            let vision_changes: Vec<(Point, U8<2>)> = unit.get_vision(handler.get_game(), end).into_iter()
            .filter_map(|(p, vision)| {
                fog_change_index(handler.get_game().get_vision(perspective, p), Some(vision))
                .and_then(|vi| Some((p, vi)))
            }).collect();
            if vision_changes.len() > 0 {
                handler.add_event(Event::PureFogChange(Some(team.try_into().unwrap()), vision_changes.try_into().unwrap()));
            }
        }
        super::on_path_details(handler, &path, &UnitType::Chess::<D>(unit.clone()));
        match unit.typ {
            ChessUnits::Pawn(d, moved_this_game, en_passant) => {
                match path.steps[0] {
                    PathStep::Diagonal(_) => {
                        // en passant
                        for n in handler.get_map().get_neighbors(end, NeighborMode::FollowPipes) {
                            if let Some(UnitType::Chess(unit)) = handler.get_map().get_unit(n.point) {
                                match unit.typ {
                                    ChessUnits::Pawn(d, _, true) => {
                                        if n.direction == d && handler.get_game().get_team(Some(unit.owner)) != perspective {
                                            handler.add_event(Event::UnitDeath(n.point, UnitType::Chess(unit.clone())));
                                        }
                                    }
                                    _ => {}
                                }
                            }
                        }
                    }
                    _ => {}
                }
                if ChessUnit::pawn_upgrades_after_path(handler.get_map(), &path, d) {
                    // upgrade if "end of map" reached, i.e. no terrain ahead that the pawn can enter
                    match self {
                        ChessCommand::Pawn(_, upgrade) => {
                            handler.add_event(Event::UnitReplacement(end, UnitType::Chess(unit.clone()), UnitType::Chess(ChessUnit {
                                typ: upgrade.to_chess_typ(),
                                hp: unit.hp,
                                owner: unit.owner,
                                exhausted: false, // will be exhausted after
                            })))
                        }
                        _ => return Err(CommandError::UnitTypeWrong)
                    }
                } else {
                    if (path.steps.len() > 1) != en_passant {
                        handler.add_event(Event::EnPassantOpportunity(end));
                    }
                    let new_dir = ChessUnit::pawn_dir_after_path(handler.get_map(), &path, d.clone());
                    if d != new_dir {
                        handler.add_event(Event::UnitDirection(end, new_dir, d));
                    }
                    if !moved_this_game {
                        handler.add_event(Event::UnitMovedThisGame(end));
                    }
                }
            }
            ChessUnits::Rook(moved_this_game) => {
                if !moved_this_game {
                    handler.add_event(Event::UnitMovedThisGame(end));
                    match self {
                        Self::Rook(dir, path_len, true) => {
                            if *path_len as usize == path.steps.len() {
                                if let Some(dp) = ChessUnit::find_king_for_castling(handler.get_game(), start, dir, *path_len as usize, unit.owner) {
                                    let mut king_path = Path::new(dp.point);
                                    king_path.steps.push(PathStep::Jump(dp.direction.opposite_direction())).unwrap();
                                    let king = handler.get_map().get_unit(dp.point).unwrap().clone();
                                    handler.add_event(Event::UnitPath(Some(None), king_path.clone(), Some(false), king.clone()));
                                    if handler.get_game().is_foggy() {
                                        let vision_changes: Vec<(Point, U8<2>)> = king.get_vision(handler.get_game(), king_path.end(handler.get_map()).unwrap()).into_iter()
                                            .filter_map(|(p, vision)| {
                                                fog_change_index(handler.get_game().get_vision(perspective, p), Some(vision))
                                                    .and_then(|vi| Some((p, vi)))
                                            }).collect();
                                        if vision_changes.len() > 0 {
                                            handler.add_event(Event::PureFogChange(Some(team.try_into().unwrap()), vision_changes.try_into().unwrap()));
                                        }
                                    }
                                    super::on_path_details(handler, &path, &king);
                                }
                            }
                        }
                        _ => {}
                    }
                }
            }
            ChessUnits::King(moved_this_game) => {
                if !moved_this_game {
                    handler.add_event(Event::UnitMovedThisGame(end));
                }
            }
            _ => {}
        }
        handler.add_event(Event::UnitExhaust(end));
        if recalculate_fog {
            handler.recalculate_fog(true);
        }
        Ok(())
    }
    pub fn exhaust_all_on_board(handler: &mut EventHandler<D>, pos: Point) {
        if handler.get_map().get_terrain(pos) != Some(&Terrain::ChessTile) {
            return;
        }
        let mut to_exhaust = HashSet::new();
        handler.get_map().width_search(pos, |p| {
            if let Some(unit) = handler.get_map().get_unit(p) {
                if !unit.is_exhausted() && unit.get_owner() == Some(handler.get_game().current_player().owner_id) {
                    to_exhaust.insert(p);
                }
            }
            handler.get_map().get_terrain(p) == Some(&Terrain::ChessTile)
        });
        for p in handler.get_map().all_points().into_iter().filter(|p| to_exhaust.contains(p)) {
            handler.add_event(Event::UnitExhaust(p));
        }
    }
}

#[derive(Debug, PartialEq, Clone, Zippable)]
pub struct ChessUnit<D: Direction> {
    pub typ: ChessUnits::<D>,
    pub owner: Owner,
    pub hp: Hp,
    pub exhausted: bool,
}
impl<D: Direction> ChessUnit<D> {
    pub fn new_instance(from: ChessUnits<D>, owner: Owner) -> ChessUnit<D> {
        ChessUnit {
            typ: from,
            owner,
            hp: 100.try_into().unwrap(),
            exhausted: false,
        }
    }
    fn can_move_through(game: &Game<D>, p: Point, team: Team, ignore_unseen: bool) -> bool {
        game.get_map().get_terrain(p).and_then(|t| t.movement_cost(MovementType::Chess)).is_some() &&
        (game.get_map().get_unit(p).is_none() || ignore_unseen && !game.has_vision_at(ClientPerspective::Team(*team), p))
    }
    fn can_stop_on(game: &Game<D>, p: Point, team: Team) -> bool {
        if game.get_map().get_terrain(p).and_then(|t| t.movement_cost(MovementType::Chess)).is_none() {
            return false;
        }
        if let Some(unit) = game.get_map().get_unit(p) {
            unit.killable_by_chess(team, game)
        } else {
            true
        }
    }
    fn possible_rook_paths<F>(game: &Game<D>, start: Point, team: ClientPerspective, max_cost: MovementPoints, ignore_unseen: bool, mut callback: F) -> bool
    where F: FnMut(Point, &Vec<PathStep<D>>) -> PathSearchFeedback {
        for d in D::list() {
            let mut found = false;
            straight_search(game, start, d, Some(max_cost), team, ignore_unseen, |p, steps| {
                match callback(p, steps) {
                    PathSearchFeedback::Continue => false,
                    PathSearchFeedback::Found => {
                        found = true;
                        true
                    },
                    PathSearchFeedback::Rejected => true,
                }
            });
            if found {
                return true;
            }
        }
        false
    }
    fn possible_bishop_paths<F>(game: &Game<D>, start: Point, team: ClientPerspective, max_cost: MovementPoints, ignore_unseen: bool, mut callback: F) -> bool
    where F: FnMut(Point, &Vec<PathStep<D>>) -> PathSearchFeedback {
        for d in D::list() {
            let mut found = false;
            diagonal_search(game, start, d, Some(max_cost), team, ignore_unseen, |p, steps| {
                match callback(p, steps) {
                    PathSearchFeedback::Continue => false,
                    PathSearchFeedback::Found => {
                        found = true;
                        true
                    },
                    PathSearchFeedback::Rejected => true,
                }
            });
            if found {
                return true;
            }
        }
        false
    }
    fn all_possible_paths<F>(&self, game: &Game<D>, start: Point, ignore_unseen: bool, mut callback: F)
    where F: FnMut(Point, &Vec<PathStep<D>>) -> PathSearchFeedback {
        let team = match game.get_team(Some(self.owner)) {
            ClientPerspective::Neutral => panic!("Game with chess piece that doesn't belong to a team"),
            ClientPerspective::Team(team) => team.try_into().unwrap(),
        };
        match self.typ {
            ChessUnits::Pawn(dir, moved_this_game, _) => {
                let mut directions = vec![];
                if game.get_map().get_terrain(start) == Some(&Terrain::ChessTile) {
                    directions.push(dir);
                } else {
                    directions = D::list();
                }
                for d in directions.clone() {
                    if let Some(dp) = game.get_map().get_neighbor(start, d) {
                        if Self::can_move_through(game, dp.point, team, ignore_unseen) {
                            // move forward 1
                            let mut steps = vec![PathStep::Dir(d)];

                            match callback(dp.point, &steps) {
                                PathSearchFeedback::Continue => {
                                    if let Some(dp) = game.get_map().get_neighbor(dp.point, dp.direction) {
                                        if !moved_this_game && Self::can_move_through(game, dp.point, team, ignore_unseen) {
                                            // move forward 2
                                            steps.push(PathStep::Dir(dp.direction));
                                            match callback(dp.point, &steps) {
                                                PathSearchFeedback::Found => return,
                                                _ => {}
                                            }
                                        }
                                    }
                                },
                                PathSearchFeedback::Found => return,
                                PathSearchFeedback::Rejected => continue,
                            }
                        }
                    }
                }
                //let directions: HashSet<Box<D>> = directions.into_iter().flat_map(|d| vec![d.clone(), Box::new(d.rotate_by(&D::list()[1].mirror_vertically()))]).collect();
                //for d in directions {
                for dp in pawn_attackable_positions(game, start, dir) {
                    //if let Some(dp) = get_diagonal_neighbor(game.get_map(), start, &d) {
                        let mut en_passant = false;
                        for n in game.get_map().get_neighbors(dp.point, NeighborMode::FollowPipes) {
                            if let Some(UnitType::Chess(unit)) = game.get_map().get_unit(n.point) {
                                match unit.typ {
                                    ChessUnits::Pawn(d, _, true) => {
                                        en_passant = en_passant || n.direction == d && game.get_team(Some(unit.owner)) != ClientPerspective::Team(*team);
                                    }
                                    _ => {}
                                }
                            }
                        }
                        if Self::can_stop_on(game, dp.point, team) &&
                        (en_passant || game.get_map().get_unit(dp.point).is_some() && game.has_vision_at(ClientPerspective::Team(*team), dp.point)) {
                            // kill unit diagonally
                            let steps = vec![PathStep::Diagonal(dp.direction)];
                            match callback(dp.point, &steps) {
                                PathSearchFeedback::Found => return,
                                _ => {}
                            }
                        }
                    //}
                }
            }
            ChessUnits::Rook(_) => {
                Self::possible_rook_paths(game, start, ClientPerspective::Team(*team), self.typ.get_movement(), ignore_unseen, callback);
            }
            ChessUnits::Bishop => {
                Self::possible_bishop_paths(game, start, ClientPerspective::Team(*team), self.typ.get_movement(), ignore_unseen, callback);
            }
            ChessUnits::Knight => {
                for d in D::list() {
                    for turn_left in vec![true, false] {
                        if let Some(dp) = get_knight_neighbor(game.get_map(), start, d, turn_left) {
                            if Self::can_stop_on(game, dp.point, team) {
                                let steps = vec![PathStep::Knight(d, turn_left)];
                                match callback(dp.point, &steps) {
                                    PathSearchFeedback::Found => return,
                                    _ => {}
                                }
                            }
                        }
                    }
                }
            }
            ChessUnits::Queen => {
                if !Self::possible_rook_paths(game, start, ClientPerspective::Team(*team), self.typ.get_movement(), ignore_unseen, &mut callback) {
                    Self::possible_bishop_paths(game, start, ClientPerspective::Team(*team), self.typ.get_movement(), ignore_unseen, callback);
                }
            }
            ChessUnits::King(_) => {
                for d in D::list() {
                    if let Some(dp) = game.get_map().get_neighbor(start, d) {
                        if Self::can_stop_on(game, dp.point, team) {
                            let steps = vec![PathStep::Dir(d)];
                            match callback(dp.point, &steps) {
                                PathSearchFeedback::Found => return,
                                _ => {}
                            }
                        }
                    }
                }
                for d in D::list() {
                    if let Some(dp) = get_diagonal_neighbor(game.get_map(), start, d) {
                        if Self::can_stop_on(game, dp.point, team) {
                                    
                            let steps = vec![PathStep::Diagonal(d)];

                            match callback(dp.point, &steps) {
                                PathSearchFeedback::Found => return,
                                _ => {}
                            }
                        }
                    }
                }
            }
        }
    }
    pub fn movable_positions(&self, game: &Game<D>, path_so_far: &Path<D>) -> HashSet<Point> {
        let mut result = HashSet::new();
        self.all_possible_paths(game, path_so_far.start, false, |p, steps| {
            if steps.len() > path_so_far.steps.len() && steps[..path_so_far.steps.len()] == path_so_far.steps[..] {
                result.insert(p);
            }
            PathSearchFeedback::Continue
        });
        result
    }
    pub fn attackable_positions(&self, game: &Game<D>, position: Point, moved: bool) -> HashSet<Point> {
        if moved {
            return HashSet::new();
        }
        match self.typ {
            ChessUnits::Pawn(d, _, _) => {
                pawn_attackable_positions(game, position, d).into_iter().map(|dp| dp.point).collect()
            }
            _ => self.movable_positions(game, &Path::new(position)),
        }
    }
    pub fn shortest_path_to(&self, game: &Game<D>, path_so_far: &Path<D>, goal: Point) -> Option<Path<D>> {
        let mut result = None;
        self.all_possible_paths(game, path_so_far.start, false, |p, steps| {
            if p == goal && steps.len() >= path_so_far.steps.len() && steps[..path_so_far.steps.len()] == path_so_far.steps[..] {
                result = Some(Path {
                    start: path_so_far.start,
                    steps: steps.clone().try_into().unwrap(),
                });
                PathSearchFeedback::Found
            } else if steps.len() > path_so_far.steps.len() || steps[..] == path_so_far.steps[..steps.len()] {
                PathSearchFeedback::Continue
            } else {
                PathSearchFeedback::Rejected
            }
        });
        result
    }
    pub fn shortest_path_to_attack(&self, game: &Game<D>, path_so_far: &Path<D>, goal: Point) -> Option<Path<D>> {
        match self.typ {
            ChessUnits::Pawn(d, _, _) => {
                for dp in pawn_attackable_positions(game, path_so_far.start, d) {
                    if dp.point == goal {
                        return Some(Path {
                            start: path_so_far.start,
                            steps: vec![PathStep::Diagonal(dp.direction)].try_into().unwrap(),
                        });
                    }
                }
                None
            }
            _ => self.shortest_path_to(game, path_so_far, goal)
        }
    }
    pub fn options_after_path(&self, game: &Game<D>, path: &Path<D>) -> Vec<UnitAction<D>> {
        let mut result = vec![];
        if path.steps.len() > 0 {
            match self.typ {
                ChessUnits::Rook(false) => {
                    if Self::find_king_for_castling(game, path.start, path.steps[0].dir().unwrap(), path.steps.len(), self.owner).is_some() {
                        result.push(UnitAction::Castle);
                    }
                    result.push(UnitAction::Wait);
                }
                ChessUnits::Pawn(d, _, _) => {
                    if Self::pawn_upgrades_after_path(game.get_map(), path, d.clone()) {
                        result.push(UnitAction::PawnUpgrade(PawnUpgrade::Queen));
                        result.push(UnitAction::PawnUpgrade(PawnUpgrade::Rook));
                        result.push(UnitAction::PawnUpgrade(PawnUpgrade::Knight));
                        result.push(UnitAction::PawnUpgrade(PawnUpgrade::Bishop));
                    } else {
                        result.push(UnitAction::Wait);
                    }
                }
                _ => {
                    result.push(UnitAction::Wait);
                }
            }
        }
        result
    }
    pub fn threatens(&self, _game: &Game<D>, target: &UnitType<D>) -> bool {
        match target {
            UnitType::Structure(_) => false,
            _ => true,
        }
    }
    fn find_king_for_castling(game: &Game<D>, start: Point, dir: D, path_len: usize, owner: Owner) -> Option<OrientedPoint<D>> {
        let mut rook_end = OrientedPoint::new(start, false, dir);
        for _ in 0..path_len {
            rook_end = game.get_map().get_neighbor(rook_end.point, rook_end.direction).unwrap();
        }
        if let Some(dp) = game.get_map().get_neighbor(rook_end.point, rook_end.direction) {
            if let Some(UnitType::Chess(unit)) = game.get_map().get_unit(dp.point) {
                let king = game.get_map().get_unit(dp.point).unwrap();
                let team = match game.get_team(Some(owner)) {
                    ClientPerspective::Neutral => panic!("Game with chess piece that doesn't belong to a team"),
                    ClientPerspective::Team(team) => team.try_into().unwrap(),
                };
                if unit.typ == ChessUnits::King(false) && unit.owner == owner
                    && game.find_visible_threats(dp.point, &king, team).is_empty()
                    && game.find_visible_threats(rook_end.point, &king, team).is_empty()
                    && game.find_visible_threats(game.get_map().get_neighbor(rook_end.point, rook_end.direction.opposite_direction()).unwrap().point, &king, team).is_empty() {
                    return Some(dp);
                }
            }
        }
        None
    }
    fn pawn_dir_after_path(map: &Map<D>, path: &Path<D>, old_dir: D) -> D {
        if let Some(d) = path.steps.last().unwrap().dir() {
            let mut p = path.start;
            for step in path.steps.iter().take(path.steps.len() - 1) {
                p = step.progress(map, p).unwrap();
            }
            map.get_neighbor(p, d).unwrap().direction
        } else {
            old_dir
        }
    }
    fn pawn_upgrades_after_path(map: &Map<D>, path: &Path<D>, old_dir: D) -> bool {
        let end = path.end(map).unwrap();
        let new_dir = Self::pawn_dir_after_path(map, path, old_dir);
        map.get_terrain(end) == Some(&Terrain::ChessTile)
        && map.get_neighbor(end, new_dir).is_none()
        && get_diagonal_neighbor(map, end, new_dir).is_none()
        && get_diagonal_neighbor(map, end, new_dir.rotate_clockwise()).is_none()
    }
    fn true_vision_range(&self, _game: &Game<D>, _pos: Point) -> usize {
        1
    }

    pub fn vision_range(&self, _game: &Game<D>, _pos: Point) -> usize {
        match self.typ {
            ChessUnits::Pawn(_, _, _) => 2,
            ChessUnits::Rook(_) => 8,
            ChessUnits::Bishop => 8,
            ChessUnits::Knight => 3,
            ChessUnits::Queen => 6,
            ChessUnits::King(_) => 2,
        }
    }

    fn add_path_to_vision(&self, game: &Game<D>, start: Point, path: &[PathStep<D>], end: Point, vision: &mut HashMap<Point, Vision>) {
        if path.len() <= self.true_vision_range(game, start) {
            vision.insert(end, Vision::TrueSight);
        } else {
            vision.insert(end, Vision::Normal);
        }
    }

    pub fn get_vision(&self, game: &Game<D>, pos: Point) -> HashMap<Point, Vision> {
        let mut result = HashMap::new();
        result.insert(pos, Vision::TrueSight);
        /*for p in game.get_map().get_neighbors(pos, NeighborMode::FollowPipes) {
            result.insert(p.point);
        }*/
        match self.typ {
            ChessUnits::Rook(_) => {
                for d in D::list() {
                    straight_search(game, pos, d, None, ClientPerspective::Neutral, true, |p, path| {
                        if path.len() > self.vision_range(game, pos) {
                            true
                        } else {
                            self.add_path_to_vision(game, pos, path, p, &mut result);
                            false
                        }
                    });
                }
            }
            ChessUnits::Bishop => {
                for d in D::list() {
                    diagonal_search(game, pos, d, None, ClientPerspective::Neutral, true, |p, path| {
                        if path.len() > self.vision_range(game, pos) {
                            true
                        } else {
                            self.add_path_to_vision(game, pos, path, p, &mut result);
                            false
                        }
                    });
                }
            }
            ChessUnits::Pawn(dir, moved_this_game, _) => {
                let mut directions = vec![];
                if game.get_map().get_terrain(pos) == Some(&Terrain::ChessTile) {
                    directions.push(dir);
                } else {
                    directions = D::list();
                }
                for d in directions.clone() {
                    if let Some(dp) = game.get_map().get_neighbor(pos, d) {
                        result.insert(dp.point, Vision::TrueSight);
                        if !moved_this_game {
                            if let Some(dp) = game.get_map().get_neighbor(dp.point, dp.direction) {
                                result.insert(dp.point, Vision::Normal);
                            }
                        }
                    }
                }
            }
            ChessUnits::Knight => {
                for d in D::list() {
                    for turn_left in vec![true, false] {
                        if let Some(dp) = get_knight_neighbor(game.get_map(), pos, d, turn_left) {
                            result.insert(dp.point, Vision::TrueSight);
                        }
                    }
                }
            }
            ChessUnits::Queen => {
                for d in D::list() {
                    straight_search(game, pos, d, None, ClientPerspective::Neutral, true, |p, path| {
                        if path.len() > self.vision_range(game, pos) {
                            true
                        } else {
                            self.add_path_to_vision(game, pos, path, p, &mut result);
                            false
                        }
                    });
                    diagonal_search(game, pos, d, None, ClientPerspective::Neutral, true, |p, path| {
                        if path.len() > self.vision_range(game, pos) {
                            true
                        } else {
                            self.add_path_to_vision(game, pos, path, p, &mut result);
                            false
                        }
                    });
                }
            }
            ChessUnits::King(_) => {
                for p in game.get_map().get_neighbors(pos, NeighborMode::FollowPipes) {
                    result.insert(p.point, Vision::TrueSight);
                }
                for d in D::list() {
                    if let Some(dp) = get_diagonal_neighbor(game.get_map(), pos, d) {
                        result.insert(dp.point, Vision::TrueSight);
                    }
                }
            }
        }
        result
    }
}

pub fn check_chess_unit_can_act<D: Direction>(game: &Game<D>, at: Point) -> Result<(), CommandError> {
    if !game.has_vision_at(ClientPerspective::Team(*game.current_player().team), at) {
        return Err(CommandError::NoVision);
    }
    let unit = match game.get_map().get_unit(at).ok_or(CommandError::MissingUnit)? {
        UnitType::Chess(unit) => unit,
        _ => return Err(CommandError::UnitTypeWrong),
    };
    if game.current_player().owner_id != unit.owner {
        return Err(CommandError::NotYourUnit);
    }
    if unit.exhausted {
        return Err(CommandError::UnitCannotMove);
    }
    Ok(())
}

fn pawn_attackable_positions<D: Direction>(game: &Game<D>, pos: Point, d:D) -> HashSet<OrientedPoint<D>> {
    let mut result = HashSet::new();
    for d in vec![d.clone(), d.rotate_clockwise()] {
        if let Some(dp) = get_diagonal_neighbor(game.get_map(), pos, d) {
            result.insert(OrientedPoint::new(dp.point, dp.mirrored, d.clone()));
        }
    }
    result
}

#[derive(Debug, PartialEq, Clone, Zippable)]
#[zippable(bits = 4)]
pub enum PawnUpgrade {
    Rook,
    Bishop,
    Knight,
    Queen,
}
impl PawnUpgrade {
    fn to_chess_typ<D: Direction>(&self) -> ChessUnits<D> {
        match self {
            Self::Rook => ChessUnits::Rook(true),
            Self::Bishop => ChessUnits::Bishop,
            Self::Knight => ChessUnits::Knight,
            Self::Queen => ChessUnits::Queen,
        }
    }
}

impl fmt::Display for PawnUpgrade {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            Self::Rook => write!(f, "Rook"),
            Self::Bishop => write!(f, "Bishop"),
            Self::Knight => write!(f, "Knight"),
            Self::Queen => write!(f, "Queen"),
        }
    }
}

#[derive(Debug, PartialEq, Clone, Zippable)]
#[zippable(bits = 4)]
pub enum ChessUnits<D: Direction> {
    Pawn(D, bool, bool),
    Rook(bool),
    Bishop,
    Knight,
    Queen,
    King(bool),
}
impl<D: Direction> ChessUnits<D> {
    pub fn name(&self) -> &'static str {
        match self {
            Self::Pawn(_, _, _) => "Pawn",
            Self::Rook(_) => "Rook",
            Self::Bishop => "Bishop",
            Self::Knight => "Knight",
            Self::Queen => "Queen",
            Self::King(_) => "King",
        }
    }
    pub fn get_movement(&self) -> MovementPoints {
        match self {
            Self::Pawn(_, _, _) => MovementPoints::from(0.),
            Self::Rook(_) =>MovementPoints::from(8.),
            Self::Bishop =>MovementPoints::from(8.),
            Self::Knight => MovementPoints::from(0.),
            Self::Queen =>MovementPoints::from(8.),
            Self::King(_) => MovementPoints::from(0.),
        }
    }
    pub fn get_armor(&self) -> (ArmorType, f32) {
        match self {
            Self::Pawn(_, _, _) => (ArmorType::Infantry, 1.5),
            Self::Rook(_) => (ArmorType::Light, 1.5),
            Self::Bishop => (ArmorType::Light, 1.5),
            Self::Knight => (ArmorType::Heli, 1.5),
            Self::Queen => (ArmorType::Heavy, 2.0),
            Self::King(_) => (ArmorType::Heavy, 2.5),
        }
    }
    pub fn value(&self) -> u16 {
        match self {
            Self::Pawn(_, _, _) => 100,
            Self::Rook(_) => 500,
            Self::Bishop => 300,
            Self::Knight => 300,
            Self::Queen => 1200,
            Self::King(_) => 800,
        }
    }
    pub fn flip_moved_this_game(&mut self) {
        match self {
            Self::Pawn(_, m, _) => *m = !*m,
            Self::Rook(m) => *m = !*m,
            Self::King(m) => *m = !*m,
            _ => {}
        }
    }
}

pub fn get_diagonal_neighbor<D: Direction>(map: &Map<D>, p: Point, dir: D) -> Option<OrientedPoint<D>> {
    if let Some(dp) = map.wrapping_logic().get_neighbor(p, dir).and_then(|dp| map.wrapping_logic().get_neighbor(dp.point, dp.direction.rotate_counter_clockwise())) {
        Some(OrientedPoint::new(dp.point, dp.mirrored, dp.direction.rotate_clockwise()))
    } else if let Some(dp) = map.wrapping_logic().get_neighbor(p, dir.rotate_counter_clockwise()).and_then(|dp| map.wrapping_logic().get_neighbor(dp.point, dp.direction.rotate_clockwise())) {
        Some(dp)
    } else {
        None
    }
}

pub fn get_knight_neighbor<D: Direction>(map: &Map<D>, p: Point, dir: D, turn_left: bool) -> Option<OrientedPoint<D>> {
    let rotation = if turn_left {
        D::angle_0()
    } else {
        D::angle_0().rotate_clockwise()
    };
    if let Some(dp) = map.wrapping_logic().get_neighbor(p, dir).and_then(|dp| get_diagonal_neighbor(map, dp.point, dp.direction.rotate_by(rotation))) {
        Some(OrientedPoint::new(dp.point, dp.mirrored, dp.direction.rotate_by(rotation.mirror_vertically())))
    } else if let Some(dp) = get_diagonal_neighbor(map, p, dir.rotate_by(rotation)).and_then(|dp| map.wrapping_logic().get_neighbor(dp.point, dp.direction.rotate_by(rotation.mirror_vertically()))) {
        Some(dp)
    } else {
        None
    }
}

// callback returns true if the search can be aborted
// if team is None, units will be ignored
fn straight_search<D, F>(game: &Game<D>, start: Point, direction: D, max_cost: Option<MovementPoints>, team: ClientPerspective, ignore_unseen: bool, mut callback: F)
where D: Direction, F: FnMut(Point, &Vec<PathStep<D>>) -> bool {
    let mut cost = MovementPoints::from(0.);
    let mut blocked_positions = HashMap::new();
    blocked_positions.insert(start, direction);
    let mut steps = vec![];
    let mut dp = OrientedPoint::new(start, false, direction);
    loop {
        steps.push(PathStep::Dir(dp.direction));
        if let Some(next_dp) = game.get_map().get_neighbor(dp.point, dp.direction) {
            if blocked_positions.get(&next_dp.point).and_then(|d| Some(*d == next_dp.direction || d.opposite_direction() == next_dp.direction)).unwrap_or(false) {
                break;
            }
            if let Some(c) = game.get_map().get_terrain(next_dp.point).and_then(|t| t.movement_cost(MovementType::Chess)) {
                if let Some(max_cost) = max_cost {
                    if cost + c > max_cost {
                        break;
                    }
                }
                if let ClientPerspective::Team(team) = team {
                    if let Some(unit) = game.get_map().get_unit(next_dp.point) {
                        if !ignore_unseen || game.has_vision_at(ClientPerspective::Team(team), next_dp.point) {
                            if unit.killable_by_chess(team.try_into().unwrap(), game) {
                                callback(next_dp.point, &steps);
                            }
                            break;
                        }
                    }
                }
                cost += c;
                dp = next_dp;
                if callback(dp.point, &steps) {
                    break;
                }
                blocked_positions.insert(dp.point, dp.direction);
            } else {
                break;
            }
        } else {
            break;
        }
    }

}

// callback returns true if the search can be aborted
// if team is None, units will be ignored
fn diagonal_search<D, F>(game: &Game<D>, start: Point, direction: D, max_cost: Option<MovementPoints>, team: ClientPerspective, ignore_unseen: bool, mut callback: F)
where D: Direction, F: FnMut(Point, &Vec<PathStep<D>>) -> bool {
    let mut cost = MovementPoints::from(0.);
    let mut blocked_positions = HashMap::new();
    blocked_positions.insert(start, direction);
    let mut steps = vec![];
    let mut dp = OrientedPoint::new(start, false, direction);
    loop {
        steps.push(PathStep::Diagonal(dp.direction));
        if let Some(next_dp) = get_diagonal_neighbor(game.get_map(), dp.point, dp.direction) {
            if blocked_positions.get(&next_dp.point).and_then(|d| Some(*d == next_dp.direction || d.opposite_direction() == next_dp.direction)).unwrap_or(false) {
                break;
            }
            if let Some(c) = game.get_map().get_terrain(next_dp.point).and_then(|t| t.movement_cost(MovementType::Chess)) {
                if let Some(max_cost) = max_cost {
                    if cost + c > max_cost {
                        break;
                    }
                }
                if let ClientPerspective::Team(team) = team {
                    if let Some(unit) = game.get_map().get_unit(next_dp.point) {
                        if !ignore_unseen || game.has_vision_at(ClientPerspective::Team(team), next_dp.point) {
                            if unit.killable_by_chess(team.try_into().unwrap(), game) {
                                callback(next_dp.point, &steps);
                            }
                            break;
                        }
                    }
                }
                cost += c;
                dp = next_dp;
                if callback(dp.point, &steps) {
                    break;
                }
                blocked_positions.insert(dp.point, dp.direction);
            }
        } else {
            break;
        }
    }

}

