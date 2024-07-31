use std::collections::HashSet;

use interfaces::ClientPerspective;
use zipper::*;

use crate::commander::Commander;
use crate::config::config::Config;
use crate::config::environment::Environment;
use crate::details::Detail;
use crate::game::game_view::GameView;
use crate::map::direction::Direction;
use crate::map::point::Point;
use crate::units::movement::Path;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Owner(pub i8);

impl SupportedZippable<&Config> for Owner {
    fn export(&self, zipper: &mut Zipper, support: &Config) {
        zipper.write_u8((self.0 + 1) as u8, bits_needed_for_max_value(support.max_player_count() as u32));
    }
    fn import(unzipper: &mut Unzipper, support: &Config) -> Result<Self, ZipperError> {
        Ok(Self(unzipper.read_u8(bits_needed_for_max_value(support.max_player_count() as u32))? as i8 - 1))
    }
}
impl SupportedZippable<&Environment> for Owner {
    fn export(&self, zipper: &mut Zipper, support: &Environment) {
        self.export(zipper, &*support.config)
    }
    fn import(unzipper: &mut Unzipper, support: &Environment) -> Result<Self, ZipperError> {
        Self::import(unzipper, &*support.config)
    }
}

impl From<i8> for Owner {
    fn from(value: i8) -> Self {
        Self(value)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Team(pub u8);

impl SupportedZippable<&Config> for Team {
    fn export(&self, zipper: &mut Zipper, support: &Config) {
        zipper.write_u8(self.0, bits_needed_for_max_value(support.max_player_count() as u32));
    }
    fn import(unzipper: &mut Unzipper, support: &Config) -> Result<Self, ZipperError> {
        Ok(Self(unzipper.read_u8(bits_needed_for_max_value(support.max_player_count() as u32))?))
    }
}
impl SupportedZippable<&Environment> for Team {
    fn export(&self, zipper: &mut Zipper, support: &Environment) {
        self.export(zipper, &*support.config)
    }
    fn import(unzipper: &mut Unzipper, support: &Environment) -> Result<Self, ZipperError> {
        Self::import(unzipper, &*support.config)
    }
}

impl From<u8> for Team {
    fn from(value: u8) -> Self {
        Self(value)
    }
}

pub type Income = I<-1000, 1000>;
pub type Funds = I<-1_000_000_000, 1_000_000_000>;

pub type Perspective = Option<Team>;

pub fn from_client_perspective(value: ClientPerspective) -> Perspective {
    match value {
        ClientPerspective::Neutral => None,
        ClientPerspective::Team(team) => Some(team.into()),
    }
}

pub fn to_client_perspective(value: &Perspective) -> ClientPerspective {
    match value {
        None => ClientPerspective::Neutral,
        Some(team) => ClientPerspective::Team(team.0),
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct Player {
    owner_id: u8,
    pub funds: Funds, // may not be consistent during fog
    pub dead: bool,
    pub commander: Commander,
}
impl Player {
    pub fn new(owner_id: u8, funds: i32, commander: Commander) -> Self {
        Self {
            owner_id: owner_id.try_into().expect(&format!("Owner id can be at most 15, got {}", owner_id)),
            funds: funds.try_into().expect("funds setting outside of allowed values"),
            dead: false,
            commander,
        }
    }

    pub fn get_owner_id(&self) -> i8 {
        self.owner_id as i8
    }

    pub fn get_team(&self) -> ClientPerspective {
        self.commander.environment().get_team(self.get_owner_id())
    }

    pub fn get_income(&self) -> i32 {
        self.commander.environment().get_income(self.get_owner_id())
    }
    
    pub fn funds_after_path<D: Direction>(&self, game: &impl GameView<D>, path: &Path<D>) -> i32 {
        let mut funds_after_path = *self.funds;
        let income = self.get_income();
        let path_points: HashSet<Point> = path.points(game).unwrap().into_iter().collect();
        for p in path_points {
            for detail in game.get_details(p) {
                match detail.fog_replacement(game.get_fog_at(ClientPerspective::Team(self.owner_id), p)) {
                    Some(Detail::Coins1) => funds_after_path += income / 2,
                    Some(Detail::Coins2) => funds_after_path += income,
                    Some(Detail::Coins3) => funds_after_path += income * 3 / 2,
                    _ => {}
                }
            }
        }
        funds_after_path
    }

    pub fn export(&self, zipper: &mut Zipper, hide_secrets: bool) {
        let environment = self.commander.environment();
        zipper.write_u8(self.owner_id, bits_needed_for_max_value(environment.config.max_player_count() as u32 - 1));
        zipper.write_bool(self.dead);
        self.commander.export(zipper, environment);
        if !hide_secrets {
            self.funds.zip(zipper);
        }
    }
    pub fn import(unzipper: &mut Unzipper, environment: &Environment, hide_secrets: bool) -> Result<Self, ZipperError> {
        let owner_id = unzipper.read_u8(bits_needed_for_max_value(environment.config.max_player_count() as u32 - 1))?;
        let dead = unzipper.read_bool()?;
        let commander = Commander::import(unzipper, environment)?;
        let funds = if hide_secrets {
            0.into()
        } else {
            Funds::unzip(unzipper)?
        };
        Ok(Self {
            owner_id,
            commander,
            funds,
            dead,
        })
    }
}
