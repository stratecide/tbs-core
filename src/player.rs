use zipper::*;
use crate::commanders::*;

pub type Owner = U8<16>;
pub const OWNER_0: Owner = Owner::new(0);
pub const OWNER_1: Owner = Owner::new(1);
pub const OWNER_2: Owner = Owner::new(2);
pub const OWNER_3: Owner = Owner::new(3);
pub const OWNER_4: Owner = Owner::new(4);
pub const OWNER_5: Owner = Owner::new(5);
pub const OWNER_6: Owner = Owner::new(6);
pub const OWNER_7: Owner = Owner::new(7);
pub const OWNER_8: Owner = Owner::new(8);
pub const OWNER_9: Owner = Owner::new(9);
pub const OWNER_10: Owner = Owner::new(10);
pub const OWNER_11: Owner = Owner::new(11);
pub const OWNER_12: Owner = Owner::new(12);
pub const OWNER_13: Owner = Owner::new(13);
pub const OWNER_14: Owner = Owner::new(14);
pub const OWNER_15: Owner = Owner::new(15);

pub type Team = U8<16>;
pub type Income = I16<-1000, 1000>;
pub type Funds = I32<-1_000_000_000, 1_000_000_000>;

pub type Perspective = Option<Team>;

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
            Funds::new(0)
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

