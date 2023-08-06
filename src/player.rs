use interfaces::game_interface::ClientPerspective;
use zipper::*;
use crate::commanders::*;

pub type Owner = U<15>;
pub type Team = U<15>;
pub type Income = I<-1000, 1000>;
pub type Funds = I<-1_000_000_000, 1_000_000_000>;

pub type Perspective = Option<Team>;

pub fn from_client_perspective(value: ClientPerspective) -> Perspective {
    match value {
        ClientPerspective::Neutral => None,
        ClientPerspective::Team(team) => Some(Team::try_from(team).unwrap()),
    }
}

pub fn to_client_perspective(value: &Perspective) -> ClientPerspective {
    match value {
        None => ClientPerspective::Neutral,
        Some(team) => ClientPerspective::Team(**team as u8),
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct Player {
    pub color_id: u8,
    pub owner_id: Owner,
    pub team: Team,
    pub income: Income,
    pub funds: Funds, // may not be consistent during fog
    pub dead: bool,
    pub commander: Commander,
}
impl Player {
    pub fn new(id: u8, team: Team, income: i16, funds: i32, commander: Commander) -> Self {
        Self {
            color_id: id,
            owner_id: id.try_into().expect(&format!("Owner id can be at most 15, got {}", id)),
            team,
            income: income.try_into().expect("income setting outside of allowed values"),
            funds: funds.try_into().expect("funds setting outside of allowed values"),
            dead: false,
            commander,
        }
    }
    pub fn export(&self, zipper: &mut Zipper, hide_secrets: bool) {
        zipper.write_u8(self.color_id, 4);
        self.owner_id.export(zipper);
        self.team.export(zipper);
        zipper.write_bool(self.dead);
        self.commander.export(zipper);
        self.income.export(zipper);
        if !hide_secrets {
            self.funds.export(zipper);
        }
    }
    pub fn import(unzipper: &mut Unzipper, hidden: bool) -> Result<Self, ZipperError> {
        let color_id = unzipper.read_u8(4)?;
        let owner_id = Owner::import(unzipper)?;
        let team = Team::import(unzipper)?;
        let dead = unzipper.read_bool()?;
        let commander = Commander::import(unzipper)?;
        let income = Income::import(unzipper)?;
        let funds = if hidden {
            0.into()
        } else {
            Funds::import(unzipper)?
        };
        Ok(Self {
            color_id,
            owner_id,
            team,
            commander,
            income,
            funds,
            dead,
        })
    }
}

impl Player {
}

