//! Сборка промптов для хода истории.
//!
//! Здесь живут чистые функции: как ужать историю под контекст, как подтолкнуть модель не
//! повторять зачины, как собрать промпт кадра и как вычистить из прозы служебные блоки,
//! которые локальная модель иногда протаскивает в текст.

pub mod history;
pub mod sets;
pub mod image;

pub use history::{build_anti_repetition, pack_story_history, AntiRepetition, PackedHistory};
pub use sets::{prompts_for, PromptSet};
pub use image::{
    apply_edit_continuity, apply_style_prefix, finalize_scene_prompt, strip_image_artifacts,
};
