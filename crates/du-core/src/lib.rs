//! Типы Dungeon Ultimate — зеркало `src/lib/types.ts`.
//!
//! Контракт с фронтом остаётся тем же JSON, что отдавали роуты Next.js, поэтому все
//! структуры сериализуются в camelCase, а необязательные поля не пишутся при None.

pub mod settings;
pub mod story;

pub use settings::*;
pub use story::*;
