//! Состояние RPG-слоя: статы персонажа, эффекты, предметы, враги, объявление хода.

use serde::{Deserialize, Serialize};

use crate::dice::{clamp_stat, Ability};

/// Шесть характеристик D&D 5e.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct CharacterStats {
    pub str: i32,
    pub dex: i32,
    pub con: i32,
    pub int: i32,
    pub wis: i32,
    pub cha: i32,
}

impl Default for CharacterStats {
    fn default() -> Self {
        Self { str: 10, dex: 10, con: 10, int: 10, wis: 10, cha: 10 }
    }
}

impl CharacterStats {
    pub fn get(&self, ability: Ability) -> i32 {
        match ability {
            Ability::Str => self.str,
            Ability::Dex => self.dex,
            Ability::Con => self.con,
            Ability::Int => self.int,
            Ability::Wis => self.wis,
            Ability::Cha => self.cha,
        }
    }

    pub fn set(&mut self, ability: Ability, value: i32) {
        match ability {
            Ability::Str => self.str = value,
            Ability::Dex => self.dex = value,
            Ability::Con => self.con = value,
            Ability::Int => self.int = value,
            Ability::Wis => self.wis = value,
            Ability::Cha => self.cha = value,
        }
    }
}

/// Модификаторы, которые даёт предмет или эффект. Отсутствующее поле = не влияет.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct Modifiers {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub str: Option<i32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub dex: Option<i32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub con: Option<i32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub int: Option<i32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub wis: Option<i32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cha: Option<i32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ac: Option<i32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max_hp: Option<i32>,
}

impl Modifiers {
    pub fn ability(&self, ability: Ability) -> Option<i32> {
        match ability {
            Ability::Str => self.str,
            Ability::Dex => self.dex,
            Ability::Con => self.con,
            Ability::Int => self.int,
            Ability::Wis => self.wis,
            Ability::Cha => self.cha,
        }
    }

    pub fn is_empty(&self) -> bool {
        *self == Modifiers::default()
    }

    /// Пары «ключ → значение» для журнальной строки, в порядке характеристик, затем КЗ и макс. HP.
    pub fn entries(&self) -> Vec<(&'static str, i32)> {
        let mut out = Vec::new();
        for ability in crate::dice::ABILITIES {
            if let Some(value) = self.ability(ability) {
                if value != 0 {
                    out.push((ability.key(), value));
                }
            }
        }
        if let Some(value) = self.ac {
            if value != 0 {
                out.push(("ac", value));
            }
        }
        if let Some(value) = self.max_hp {
            if value != 0 {
                out.push(("maxHp", value));
            }
        }
        out
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
#[derive(Default)]
pub enum EffectKind {
    #[default]
    Buff,
    Debuff,
}


/// Временный эффект — благословение, проклятие, яд. Его модификаторы складываются в
/// производные статы ровно как надетая экипировка; `turns` убывает раз в ход, на нуле
/// эффект снимается. Ход = один разрешённый игровой шаг.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Effect {
    pub id: String,
    pub name: String,
    #[serde(default)]
    pub kind: EffectKind,
    #[serde(default)]
    pub modifiers: Modifiers,
    pub turns: i32,
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub note: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct Hp {
    pub current: i32,
    pub max: i32,
}

impl Default for Hp {
    fn default() -> Self {
        Self { current: 20, max: 20 }
    }
}

/// Состояние персонажа. Хранится как JSON на строке персонажа рядом со свободным
/// текстом инвентаря и заклинаний, который остаётся чистым флейвором.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct CharacterRpg {
    pub stats: CharacterStats,
    pub hp: Hp,
    pub ac: i32,
    pub level: i32,
    pub xp: i32,
    pub conditions: Vec<String>,
    pub effects: Vec<Effect>,
    pub dead: bool,
}

impl Default for CharacterRpg {
    fn default() -> Self {
        Self {
            stats: CharacterStats::default(),
            hp: Hp::default(),
            ac: 10,
            level: 1,
            xp: 0,
            conditions: Vec::new(),
            effects: Vec::new(),
            dead: false,
        }
    }
}

impl CharacterRpg {
    /// Привести значения, пришедшие из базы или от модели, к допустимым: статы и уровень
    /// не ниже минимума, «мёртв» выводится ещё и из HP ≤ 0.
    pub fn sanitized(mut self) -> Self {
        for ability in crate::dice::ABILITIES {
            let value = clamp_stat(self.stats.get(ability), 1, 30);
            self.stats.set(ability, value);
        }
        self.hp.max = self.hp.max.max(1);
        self.ac = self.ac.max(0);
        self.level = self.level.max(1);
        self.xp = self.xp.max(0);
        self.effects.retain(|effect| !effect.name.trim().is_empty());
        self.dead = self.dead || self.hp.current <= 0;
        self
    }
}

/// Запись в журнале приключения. Журнал — одновременно пересказ для игрока и аудит того,
/// что движок разрешил из объявлений нарратора.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum EventKind {
    Roll,
    Hp,
    Death,
    Item,
    Combat,
    Effect,
    Quest,
    Level,
    Achievement,
    Note,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GameEvent {
    pub id: String,
    pub kind: EventKind,
    pub text: String,
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub data: Option<serde_json::Value>,
    pub created_at: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
#[derive(Default)]
pub enum ItemSlot {
    Weapon,
    Armor,
    Shield,
    Trinket,
    Consumable,
    #[default]
    Misc,
}


#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
#[derive(Default)]
pub enum ItemRarity {
    #[default]
    Common,
    Uncommon,
    Rare,
    Epic,
    Legendary,
}


impl ItemRarity {
    pub fn label_ru(self) -> &'static str {
        match self {
            ItemRarity::Common => "обычный",
            ItemRarity::Uncommon => "необычный",
            ItemRarity::Rare => "редкий",
            ItemRarity::Epic => "эпический",
            ItemRarity::Legendary => "легендарный",
        }
    }
}

/// Состояние задания.
///
/// `Offered` — модель предложила, игрок ещё не решил. Дальше он либо берёт его (`Active`),
/// либо отказывается (`Declined`). Взятое задание висит в контексте хода, пока не закроется.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
#[derive(Default)]
pub enum QuestStatus {
    #[default]
    Offered,
    Active,
    Done,
    Failed,
    Declined,
}


impl QuestStatus {
    /// Держим ли мы задание в контексте хода.
    pub fn is_open(self) -> bool {
        matches!(self, QuestStatus::Offered | QuestStatus::Active)
    }
}

/// Задание: кто дал, что нужно сделать и при каких условиях оно закроется.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Quest {
    pub id: String,
    pub title: String,
    /// Кто выдал. Без этого задание нельзя вернуть — и непонятно, к кому идти.
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub giver: Option<String>,
    pub summary: String,
    /// Условия, по которым видно, выполнено оно или нет.
    #[serde(default)]
    pub conditions: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub reward: Option<String>,
    /// Опыт за выполнение.
    #[serde(default)]
    pub xp: i32,
    #[serde(default)]
    pub status: QuestStatus,
    /// На каком ходу задание предложили. По нему держится редкость: следующее не раньше,
    /// чем через несколько ходов.
    #[serde(default)]
    pub turn: i64,
    pub created_at: String,
    pub updated_at: String,
}

/// Насколько заметно достижение. От этого зависит, как его показать: обычное отмечают
/// строкой, а легендарное — плашкой, мимо которой не пройдёшь.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum AchievementRarity {
    #[default]
    Common,
    Rare,
    Legendary,
}

/// Достижение: чем именно игрок отличился и когда.
///
/// Живёт отдельно от заданий: задание игрок берёт и выполняет, а достижение случается —
/// его нельзя «взять», о нём узнают постфактум.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Achievement {
    pub id: String,
    pub title: String,
    /// За что выдано — словами игрока, а не разбором механики.
    pub summary: String,
    /// Значок плашки. Одна картинка-эмодзи, подобранная под повод.
    #[serde(default)]
    pub icon: String,
    #[serde(default)]
    pub rarity: AchievementRarity,
    /// На каком ходу выдано: по нему держится редкость.
    #[serde(default)]
    pub turn: i64,
    /// В какой истории заработано. Сама история может быть давно удалена — награда живёт
    /// в профиле игрока и переживает её.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub chat_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub story: Option<String>,
    pub created_at: String,
}

/// Предложение задания от модели.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct OfferQuestDecl {
    pub title: String,
    pub giver: Option<String>,
    pub summary: Option<String>,
    pub conditions: Vec<String>,
    pub reward: Option<String>,
    pub xp: Option<i32>,
}

/// Закрытие задания: модель называет его заголовком, который сама же и дала.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct QuestOutcomeDecl {
    pub title: String,
    pub reason: Option<String>,
}

/// Начисление опыта — за бой, находку или что угодно ещё по решению модели.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct XpAwardDecl {
    pub character_id: Option<String>,
    pub amount: i32,
    pub reason: Option<String>,
}

/// Предмет инвентаря. `image_url` заполняет пайплайн картинок, когда дроп иллюстрируется,
/// после чего портрет переиспользуется как референс.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Item {
    pub id: String,
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub owner_id: Option<String>,
    pub name: String,
    #[serde(default)]
    pub slot: ItemSlot,
    #[serde(default)]
    pub rarity: ItemRarity,
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub description: Option<String>,
    /// Запись костей урона, например «1d8+1».
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub damage: Option<String>,
    #[serde(default)]
    pub modifiers: Modifiers,
    #[serde(default)]
    pub equipped: bool,
    pub qty: i32,
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub image_url: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub image_prompt_en: Option<String>,
    pub created_at: String,
}

/// Противник. Механически это тот же блок статов, что у персонажа, но живёт он в
/// состоянии чата как временный состав стычки, а не отдельной строкой персонажа.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Enemy {
    pub id: String,
    pub name: String,
    pub rpg: CharacterRpg,
}

// ── Объявление хода, которое приходит от структурированного прохода модели ──────────

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct RollDecl {
    pub ability: Option<Ability>,
    pub dc: i32,
    pub label: Option<String>,
    pub actor_id: Option<String>,
    pub kind: Option<String>,
    pub target_id: Option<String>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct SpawnDecl {
    pub name: String,
    pub hp: Option<i32>,
    pub ac: Option<i32>,
    pub level: Option<i32>,
    pub stats: Option<CharacterStats>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct AttackDecl {
    pub attacker_id: Option<String>,
    pub target_id: String,
    pub ability: Option<Ability>,
    pub damage: Option<String>,
    pub label: Option<String>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct HpDeltaDecl {
    pub character_id: Option<String>,
    pub amount: i32,
    pub reason: Option<String>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct GrantItemDecl {
    pub owner_id: Option<String>,
    pub name: String,
    pub slot: Option<ItemSlot>,
    pub rarity: Option<ItemRarity>,
    pub description: Option<String>,
    pub damage: Option<String>,
    pub modifiers: Option<Modifiers>,
    pub qty: Option<i32>,
    pub with_image: Option<bool>,
    pub image_prompt_en: Option<String>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct ApplyEffectDecl {
    pub character_id: Option<String>,
    pub name: String,
    pub kind: Option<EffectKind>,
    pub modifiers: Option<Modifiers>,
    pub turns: Option<i32>,
    pub note: Option<String>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct ClearEffectDecl {
    pub character_id: Option<String>,
    /// Имя эффекта; «*» снимает все.
    pub name: String,
}

/// Что нарратор объявил механически за этот ход. Движок решает исход сам.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct GameUpdate {
    pub rolls: Vec<RollDecl>,
    pub spawn_enemies: Vec<SpawnDecl>,
    pub attacks: Vec<AttackDecl>,
    pub hp_delta: Vec<HpDeltaDecl>,
    pub grant_items: Vec<GrantItemDecl>,
    pub apply_effects: Vec<ApplyEffectDecl>,
    pub clear_effects: Vec<ClearEffectDecl>,
    pub offer_quests: Vec<OfferQuestDecl>,
    pub complete_quests: Vec<QuestOutcomeDecl>,
    pub fail_quests: Vec<QuestOutcomeDecl>,
    pub award_xp: Vec<XpAwardDecl>,
    pub note: Option<String>,
}

/// Снимок состояния ДО хода, который висит на сообщении нарратора: по нему Retry и
/// Erase откатывают последствия вместо повторного применения.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RpgSnapshot {
    pub chars: std::collections::BTreeMap<String, CharacterRpg>,
    pub combatants: Vec<Enemy>,
    pub item_ids: Vec<String>,
    pub event_ids: Vec<String>,
}

/// Состояние RPG на уровне чата.
#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct RpgState {
    pub enabled: bool,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn stats_serialize_with_short_ability_keys() {
        let json = serde_json::to_string(&CharacterStats::default()).unwrap();
        assert_eq!(json, "{\"str\":10,\"dex\":10,\"con\":10,\"int\":10,\"wis\":10,\"cha\":10}");
    }

    #[test]
    fn modifiers_use_camel_case_max_hp_and_skip_empty() {
        let modifiers = Modifiers { ac: Some(2), max_hp: Some(4), ..Default::default() };
        let json = serde_json::to_string(&modifiers).unwrap();
        assert_eq!(json, "{\"ac\":2,\"maxHp\":4}");
    }

    #[test]
    fn sanitizing_clamps_garbage_and_derives_death_from_hp() {
        let raw = CharacterRpg {
            stats: CharacterStats { str: 99, dex: -4, ..Default::default() },
            hp: Hp { current: 0, max: 0 },
            ac: -7,
            level: 0,
            xp: -10,
            ..Default::default()
        };
        let clean = raw.sanitized();
        assert_eq!(clean.stats.str, 30);
        assert_eq!(clean.stats.dex, 1);
        assert_eq!(clean.hp.max, 1);
        assert_eq!(clean.ac, 0);
        assert_eq!(clean.level, 1);
        assert_eq!(clean.xp, 0);
        assert!(clean.dead, "HP на нуле означает смерть даже без явного флага");
    }

    #[test]
    fn stored_character_json_round_trips() {
        let rpg = CharacterRpg::default();
        let json = serde_json::to_string(&rpg).unwrap();
        let back: CharacterRpg = serde_json::from_str(&json).unwrap();
        assert_eq!(rpg, back);
        // Частичный JSON из старой базы дополняется значениями по умолчанию.
        let partial: CharacterRpg = serde_json::from_str("{\"ac\":14}").unwrap();
        assert_eq!(partial.ac, 14);
        assert_eq!(partial.hp.max, 20);
    }
}
