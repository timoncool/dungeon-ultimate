//! Детерминированный игровой движок Dungeon Ultimate.
//!
//! Модель ПРЕДЛАГАЕТ (проверка и её сложность, изменение HP, дроп), а движок бросает,
//! зажимает и решает исход здесь — поэтому нарратор не может выдумать себе ни итог
//! броска, ни выживание.

pub mod apply;
mod journal;
pub mod derive;
pub mod dice;
pub mod types;

pub use journal::{render, JournalLabels, RandomEventText};
pub use apply::{apply_game_update, Actor, ActorMap, ApplyOptions, ApplyResult};
pub use derive::{derive_for_owner, derive_rpg, DerivedRpg};
pub use dice::{
    ability_mod, clamp_stat, roll_check, roll_die, roll_notation, Ability, CheckResult, Crit,
    ABILITIES,
};
pub use types::*;
