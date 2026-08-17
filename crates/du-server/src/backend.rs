//! Где считать локальные стадии — на видеокарте или на процессоре.
//!
//! Раньше у каждой стадии был свой невнятный тумблер: у рассказчика — «сколько слоёв на
//! карте», у картинок — «выгружать на процессор» (что на деле означает совсем другое:
//! держать веса в памяти, а считать всё равно на карте), у распознавания — переменная
//! окружения, а у озвучки не было ничего. Игрок не понимал, что где считается, и жаловался,
//! что «одно на процессоре, другое на карте».
//!
//! Здесь один общий выбор и по одному переопределению на стадию — как в Dub Studio:
//! `auto` смотрит, есть ли карта; `gpu` и `cpu` говорят прямо.

use crate::runtime::Runtime;

/// Стадия, которую можно посчитать где угодно.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Stage {
    Narrator,
    Image,
    Tts,
    Asr,
}

/// Куда в итоге пойдёт стадия.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Backend {
    Gpu,
    Cpu,
}

impl Backend {
    /// Имя, которое понимают движки картинок и озвучки.
    pub fn engine_name(self) -> &'static str {
        match self {
            Backend::Gpu => "cuda",
            Backend::Cpu => "cpu",
        }
    }

    pub fn is_cpu(self) -> bool {
        self == Backend::Cpu
    }
}

/// Явный выбор игрока для стадии; пусто — следовать общему.
fn stage_choice(runtime: &Runtime, stage: Stage) -> &str {
    match stage {
        Stage::Narrator => &runtime.narrator_backend,
        Stage::Image => &runtime.image_backend,
        Stage::Tts => &runtime.tts_backend,
        Stage::Asr => &runtime.asr_backend,
    }
}

impl Stage {
    /// Умеет ли стадия считаться на процессоре.
    ///
    /// Замеры на живом железе:
    ///
    /// * распознавание речи — секунды, процессор годится вполне;
    /// * рассказчик — 14 знаков в секунду, около полутора минут на обычный отрывок. Медленно,
    ///   но текст идёт потоком и первые слова появляются через девять секунд, поэтому играть
    ///   можно; в интерфейсе это подписано прямо;
    /// * кадр — за шесть минут не выдал ни одной картинки (на карте те же 13 секунд);
    /// * озвучка — голос отстаёт от чтения настолько, что слушать нечего.
    ///
    /// Последние два поэтому и не предлагаются: выбор, которым нельзя пользоваться, хуже,
    /// чем его отсутствие.
    pub fn supports_cpu(self) -> bool {
        matches!(self, Stage::Narrator | Stage::Asr)
    }
}

/// Разобрать значение настройки. Незнакомое слово — не выбор.
fn parse(choice: &str) -> Option<Backend> {
    match choice.trim().to_lowercase().as_str() {
        "cpu" => Some(Backend::Cpu),
        "gpu" | "cuda" => Some(Backend::Gpu),
        _ => None,
    }
}

/// Есть ли на машине видеокарта, на которой вообще можно считать.
pub fn gpu_present() -> bool {
    crate::hw::snapshot().total_vram > 0
}

/// Где считать эту стадию: сначала её собственный выбор, потом общий, потом по факту железа.
pub fn stage_backend(runtime: &Runtime, stage: Stage) -> Backend {
    // Кадр и озвучка считаются только видеокартой — даже если общий выбор говорит иначе.
    if !stage.supports_cpu() {
        return Backend::Gpu;
    }
    parse(stage_choice(runtime, stage))
        .or_else(|| parse(&runtime.local_backend))
        .unwrap_or(if gpu_present() { Backend::Gpu } else { Backend::Cpu })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn runtime() -> Runtime {
        Runtime { local_backend: "gpu".into(), ..Default::default() }
    }

    #[test]
    fn a_stage_follows_the_common_choice_until_it_says_otherwise() {
        let mut settings = runtime();
        assert_eq!(stage_backend(&settings, Stage::Asr), Backend::Gpu);

        settings.asr_backend = "cpu".into();
        assert_eq!(stage_backend(&settings, Stage::Asr), Backend::Cpu);
        // Остальные стадии чужой выбор не задевает.
        assert_eq!(stage_backend(&settings, Stage::Narrator), Backend::Gpu);
    }

    #[test]
    fn the_frame_and_the_voice_are_never_pushed_to_the_processor() {
        // Кадр, рассказчик и озвучка не уводятся с карты ДАЖЕ по прямому указанию: замеры
        // показали минуты вместо секунд.
        let settings = Runtime {
            local_backend: "cpu".into(),
            image_backend: "cpu".into(),
            tts_backend: "cpu".into(),
            narrator_backend: "cpu".into(),
            ..Default::default()
        };
        assert_eq!(stage_backend(&settings, Stage::Image), Backend::Gpu);
        assert_eq!(stage_backend(&settings, Stage::Tts), Backend::Gpu);
        // Рассказчик и распознавание слушаются: это медленно, но играть можно.
        assert_eq!(stage_backend(&settings, Stage::Narrator), Backend::Cpu);
        assert_eq!(stage_backend(&settings, Stage::Asr), Backend::Cpu);
    }

    #[test]
    fn an_unknown_word_is_not_a_choice() {
        let settings = Runtime { local_backend: "как-нибудь".into(), ..Default::default() };
        // Значение мусорное — решает железо, а не оно.
        let decided = stage_backend(&settings, Stage::Narrator);
        assert!(matches!(decided, Backend::Gpu | Backend::Cpu));
    }

    #[test]
    fn the_engine_names_are_the_ones_engines_understand() {
        assert_eq!(Backend::Gpu.engine_name(), "cuda");
        assert_eq!(Backend::Cpu.engine_name(), "cpu");
        assert!(Backend::Cpu.is_cpu() && !Backend::Gpu.is_cpu());
    }
}
