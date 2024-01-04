use serde::Deserialize;

crate::listable_enum! {
    #[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Deserialize)]
    pub enum CommanderType {
        None,
        Vampire,
        Zombie,
    }
}
