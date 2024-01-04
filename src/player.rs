use interfaces::game_interface::{ClientPerspective, PlayerData};
use zipper::*;

use crate::commander::Commander;

pub type Owner = i8;
pub type Team = u8;
pub type Income = I<-1000, 1000>;
pub type Funds = I<-1_000_000_000, 1_000_000_000>;

pub type Perspective = Option<Team>;

pub fn from_client_perspective(value: ClientPerspective) -> Perspective {
    match value {
        ClientPerspective::Neutral => None,
        ClientPerspective::Team(team) => Some(team),
    }
}

pub fn to_client_perspective(value: &Perspective) -> ClientPerspective {
    match value {
        None => ClientPerspective::Neutral,
        Some(team) => ClientPerspective::Team(*team),
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

    pub fn export(&self, zipper: &mut Zipper, hide_secrets: bool) {
        self.owner_id.export(zipper);
        zipper.write_bool(self.dead);
        self.commander.export(zipper);
        if !hide_secrets {
            self.funds.export(zipper);
        }
    }
    pub fn import(unzipper: &mut Unzipper, hidden: bool) -> Result<Self, ZipperError> {
        let owner_id = Owner::import(unzipper)?;
        let dead = unzipper.read_bool()?;
        let commander = Commander::import(unzipper)?;
        let funds = if hidden {
            0.into()
        } else {
            Funds::import(unzipper)?
        };
        Ok(Self {
            owner_id,
            commander,
            funds,
            dead,
        })
    }
}
