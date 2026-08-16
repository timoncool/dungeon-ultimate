//! Разрешение хода. Нарратор объявил механику — здесь она превращается в броски,
//! урон, смерть, лут и записи журнала. Всё случайное берётся из криптостойкого
//! генератора, все значения зажимаются, решение о смерти принимается тут.

use indexmap::IndexMap;

use crate::journal::{render, JournalLabels};
use crate::dice::{clamp_stat, roll_check, roll_die, roll_notation, Ability, Crit, ABILITIES};
use crate::types::{
    ApplyEffectDecl, CharacterRpg, Effect, EffectKind, Enemy, EventKind, GameEvent, GameUpdate,
    Item, Modifiers,
};

/// Сколько активных эффектов держим на персонаже, чтобы список не рос без предела.
const EFFECT_CAP: usize = 8;
/// Шанс случайного события за разрешённый ход, в процентах.
const RANDOM_EVENT_CHANCE: i32 = 15;

/// Участник хода: герой, спутник или враг — механически они одинаковы.
#[derive(Debug, Clone)]
pub struct Actor {
    pub name: String,
    pub rpg: CharacterRpg,
}

/// Состав хода. Порядок вставки важен: первый участник — это герой, к нему падают
/// объявления без явного адресата.
pub type ActorMap = IndexMap<String, Actor>;

#[derive(Debug, Default)]
pub struct ApplyResult {
    pub events: Vec<GameEvent>,
    /// Идентификаторы тех, чьё состояние изменилось и требует сохранения.
    pub changed: Vec<String>,
    pub items: Vec<Item>,
    pub spawned_enemies: Vec<Enemy>,
}

#[derive(Debug, Default, Clone)]
pub struct ApplyOptions {
    pub hero_id: Option<String>,
    pub random_events: bool,
    /// Подписи журнала на языке игры. По умолчанию русские — как было.
    pub journal: JournalLabels,
}

fn now_iso() -> String {
    chrono::Utc::now().format("%Y-%m-%dT%H:%M:%S%.3fZ").to_string()
}

fn new_id() -> String {
    uuid::Uuid::new_v4().to_string()
}

fn make_event(kind: EventKind, text: String, data: Option<serde_json::Value>) -> GameEvent {
    GameEvent { id: new_id(), kind, text, data, created_at: now_iso() }
}

fn fmt_mod(value: i32) -> String {
    if value >= 0 {
        format!("+{value}")
    } else {
        value.to_string()
    }
}

fn effect_summary(name: &str, modifiers: &Modifiers, turns: i32, labels: &JournalLabels) -> String {
    let mods = modifiers
        .entries()
        .into_iter()
        .map(|(key, value)| format!("{} {}", labels.stat(key), fmt_mod(value)))
        .collect::<Vec<_>>()
        .join(", ");
    let mods = if mods.is_empty() { String::new() } else { format!(" ({mods})") };
    render(
        &labels.effect_summary,
        &[
            ("name", name),
            ("mods", &mods),
            ("turns", &turns.to_string()),
            ("turnWord", labels.turns.pick(turns)),
        ],
    )
}

/// Добавить или обновить эффект: одноимённый заменяется, срок отсчитывается заново.
fn merge_effect(rpg: &mut CharacterRpg, effect: Effect) {
    let key = effect.name.trim().to_lowercase();
    rpg.effects.retain(|existing| existing.name.trim().to_lowercase() != key);
    rpg.effects.push(effect);
    if rpg.effects.len() > EFFECT_CAP {
        let overflow = rpg.effects.len() - EFFECT_CAP;
        rpg.effects.drain(0..overflow);
    }
}

/// Списать один ход со всех эффектов и убрать истёкшие. Возвращает их названия.
fn tick_effects(rpg: &mut CharacterRpg) -> Vec<String> {
    let mut expired = Vec::new();
    let mut kept = Vec::new();
    for mut effect in std::mem::take(&mut rpg.effects) {
        effect.turns -= 1;
        if effect.turns <= 0 {
            expired.push(effect.name.clone());
        } else {
            kept.push(effect);
        }
    }
    rpg.effects = kept;
    expired
}

/// Подобранные благословения и проклятия для случайного события.
const RANDOM_EVENTS: &[(&str, EffectKind, Modifiers, i32, &str)] = &[
    ("Благословение силы", EffectKind::Buff, Modifiers { str: Some(2), dex: None, con: None, int: None, wis: None, cha: None, ac: None, max_hp: None }, 3, "Тело наливается мощью."),
    ("Кошачья ловкость", EffectKind::Buff, Modifiers { str: None, dex: Some(2), con: None, int: None, wis: None, cha: None, ac: None, max_hp: None }, 3, "Движения становятся текучими."),
    ("Каменная кожа", EffectKind::Buff, Modifiers { str: None, dex: None, con: None, int: None, wis: None, cha: None, ac: Some(2), max_hp: None }, 2, "Кожа твердеет, словно камень."),
    ("Прилив жизни", EffectKind::Buff, Modifiers { str: None, dex: None, con: None, int: None, wis: None, cha: None, ac: None, max_hp: Some(4) }, 3, "Запас сил прибывает."),
    ("Ясность ума", EffectKind::Buff, Modifiers { str: None, dex: None, con: None, int: Some(2), wis: Some(1), cha: None, ac: None, max_hp: None }, 3, "Мысли становятся острее."),
    ("Воодушевление", EffectKind::Buff, Modifiers { str: None, dex: None, con: None, int: None, wis: None, cha: Some(2), ac: None, max_hp: None }, 3, "Слова звучат убедительнее."),
    ("Проклятье слабости", EffectKind::Debuff, Modifiers { str: Some(-2), dex: None, con: None, int: None, wis: None, cha: None, ac: None, max_hp: None }, 3, "Мышцы наливаются свинцом."),
    ("Дрожь в руках", EffectKind::Debuff, Modifiers { str: None, dex: Some(-2), con: None, int: None, wis: None, cha: None, ac: None, max_hp: None }, 2, "Пальцы не слушаются."),
    ("Лихорадка", EffectKind::Debuff, Modifiers { str: Some(-1), dex: None, con: Some(-1), int: None, wis: None, cha: None, ac: None, max_hp: None }, 3, "Жар туманит тело."),
    ("Сглаз", EffectKind::Debuff, Modifiers { str: None, dex: None, con: None, int: None, wis: None, cha: None, ac: Some(-2), max_hp: None }, 2, "Удача отворачивается."),
    ("Смятение", EffectKind::Debuff, Modifiers { str: None, dex: None, con: None, int: Some(-2), wis: None, cha: None, ac: None, max_hp: None }, 2, "Мысли путаются."),
];

fn roll_random_event(rpg: &mut CharacterRpg, labels: &JournalLabels) -> Option<Effect> {
    if roll_die(100) > RANDOM_EVENT_CHANCE {
        return None;
    }
    let index = (roll_die(RANDOM_EVENTS.len() as i64) - 1) as usize;
    let (name, kind, modifiers, turns, note) = *RANDOM_EVENTS.get(index)?;
    // Названия и присказки берём на языке игры; механика события — из встроенного перечня.
    let (name, note) = match labels.random_events.get(index) {
        Some(text) => (text.name.as_str(), text.note.as_str()),
        None => (name, note),
    };
    let effect = Effect {
        id: new_id(),
        name: name.to_string(),
        kind,
        modifiers,
        turns,
        note: Some(note.to_string()),
    };
    merge_effect(rpg, effect.clone());
    Some(effect)
}

/// Похоже ли на машинный ключ вместо текста: `scene_mariana_bania_hint` и подобное.
///
/// Модель иногда возвращает в заметку идентификатор — по-видимому, из своей внутренней
/// разметки. Игроку такое показывать нельзя: это не событие истории, а мусор.
fn looks_like_identifier(note: &str) -> bool {
    let note = note.trim();
    if note.is_empty() || note.contains(' ') {
        return false;
    }
    // Одно слово без пробелов: ключом его выдаёт подчёркивание или дефис вместе с латиницей.
    note.chars().any(|c| c == '_' || c == '-')
        && note.chars().all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-' || c == '.')
}

/// Похоже ли на размышление модели, а не на событие истории.
fn looks_like_deliberation(note: &str) -> bool {
    let low = note.to_lowercase();
    const MARKS: [&str; 10] = [
        "нужно объявить", "объявить проверку", "исход:", "учитываем", "сцена требует",
        "используя 'ты'", "имена персонажей не", "модификатор", "против внимательности",
        "провал — ",
    ];
    MARKS.iter().filter(|mark| low.contains(*mark)).count() >= 2
}

/// Собрать эффект из объявления модели, отбросив мусор.
fn effect_from_decl(decl: &ApplyEffectDecl) -> Option<Effect> {
    let name = decl.name.trim();
    if name.is_empty() {
        return None;
    }
    let turns = match decl.turns {
        Some(value) if value > 0 => value,
        _ => 3,
    };
    Some(Effect {
        id: new_id(),
        name: name.to_string(),
        kind: decl.kind.unwrap_or(EffectKind::Buff),
        modifiers: decl.modifiers.unwrap_or_default(),
        turns,
        note: decl.note.as_deref().map(str::trim).filter(|s| !s.is_empty()).map(str::to_string),
    })
}

struct Resolver<'a> {
    actors: &'a mut ActorMap,
    hero_id: Option<String>,
}

impl<'a> Resolver<'a> {
    fn first_actor_id(&self) -> Option<String> {
        self.actors.keys().next().cloned()
    }

    /// Кому адресовать объявление без явного (или с неизвестным) идентификатором:
    /// сначала указанному, затем герою, затем первому участнику.
    fn resolve_actor_id(&self, preferred: Option<&str>) -> Option<String> {
        if let Some(id) = preferred {
            if self.actors.contains_key(id) {
                return Some(id.to_string());
            }
        }
        if let Some(hero) = self.hero_id.as_deref() {
            if self.actors.contains_key(hero) {
                return Some(hero.to_string());
            }
        }
        self.first_actor_id()
    }
}

pub fn apply_game_update(
    update: &GameUpdate,
    actors: &mut ActorMap,
    opts: &ApplyOptions,
) -> ApplyResult {
    let labels = &opts.journal;
    let mut result = ApplyResult::default();
    let mut changed: IndexMap<String, ()> = IndexMap::new();

    // Эффекты списываются ПЕРВЫМИ, чтобы наложенный этим же ходом сохранил полный срок.
    let ids: Vec<String> = actors.keys().cloned().collect();
    for id in &ids {
        let expired = actors.get_mut(id).map(|actor| tick_effects(&mut actor.rpg)).unwrap_or_default();
        for name in expired {
            changed.insert(id.clone(), ());
            result.events.push(make_event(
                EventKind::Effect,
                render(&labels.effect_faded, &[("name", &name)]),
                None,
            ));
        }
    }

    // Враги выходят на поле первыми, чтобы атаки ЭТОГО же хода могли их выбрать по id.
    for spawn in &update.spawn_enemies {
        let mut rpg = CharacterRpg::default();
        if let Some(hp) = spawn.hp {
            rpg.hp.max = hp.max(1);
            rpg.hp.current = rpg.hp.max;
        }
        if let Some(ac) = spawn.ac {
            rpg.ac = clamp_stat(ac, 1, 40);
        }
        if let Some(level) = spawn.level {
            rpg.level = level.max(1);
        }
        if let Some(stats) = spawn.stats {
            for ability in ABILITIES {
                rpg.stats.set(ability, clamp_stat(stats.get(ability), 1, 30));
            }
        }
        let name = if spawn.name.trim().is_empty() {
            labels.enemy.clone()
        } else {
            spawn.name.trim().to_string()
        };
        let enemy = Enemy { id: new_id(), name: name.clone(), rpg: rpg.clone() };
        actors.insert(enemy.id.clone(), Actor { name: name.clone(), rpg });
        changed.insert(enemy.id.clone(), ());
        result.events.push(make_event(
            EventKind::Combat,
            render(
                &labels.enemy_enters,
                &[
                    ("name", &name),
                    ("hp", &enemy.rpg.hp.max.to_string()),
                    ("ac", &enemy.rpg.ac.to_string()),
                ],
            ),
            serde_json::to_value(&enemy).ok().map(|enemy| serde_json::json!({ "enemy": enemy })),
        ));
        result.spawned_enemies.push(enemy);
    }

    // Атаки: d20 + модификатор атакующего против КЗ цели, при попадании бросается урон.
    for attack in &update.attacks {
        let first = actors.keys().next().cloned();
        // ПРИСУТСТВУЮЩИЙ, но неизвестный идентификатор атакующего пропускается — иначе
        // удар врага молча превратился бы в удар героя. По умолчанию бьёт первый участник.
        let attacker_id = match attack.attacker_id.as_deref() {
            Some(id) if actors.contains_key(id) => Some(id.to_string()),
            Some(_) => None,
            None => first,
        };
        let Some(attacker_id) = attacker_id else { continue };
        // Защита от само-удара: неверно разрешённая атака не должна прилетать атакующему.
        if attacker_id == attack.target_id {
            continue;
        }
        let Some(attacker) = actors.get(&attacker_id) else { continue };
        let Some(target) = actors.get(&attack.target_id) else { continue };
        if attacker.rpg.dead || target.rpg.dead {
            continue;
        }

        let ability = attack.ability.unwrap_or(Ability::Str);
        let score = attacker.rpg.stats.get(ability);
        let ac = target.rpg.ac;
        let attacker_name = attacker.name.clone();
        let target_name = target.name.clone();

        let check = roll_check(score, ac, 0);
        let verdict = if check.success {
            if check.crit == Some(Crit::Success) {
                &labels.verdict.crit_hit
            } else {
                &labels.verdict.hit
            }
        } else {
            &labels.verdict.miss
        };
        let label = attack.label.clone().unwrap_or_else(|| format!("{attacker_name} → {target_name}"));
        result.events.push(make_event(
            EventKind::Roll,
            render(
                &labels.attack,
                &[
                    ("label", &label),
                    ("d20", &check.d20.to_string()),
                    ("mod", &fmt_mod(check.modifier)),
                    ("total", &check.total.to_string()),
                    ("ac", &ac.to_string()),
                    ("verdict", verdict),
                ],
            ),
            Some(serde_json::json!({ "roll": { "kind": "attack", "dc": ac }, "result": check })),
        ));
        if !check.success {
            continue;
        }

        let notation = attack.damage.as_deref().unwrap_or("1d6");
        let rolled = roll_notation(notation);
        // Крит по правилам 5e удваивает КОСТИ, а не плоский модификатор:
        // 1d8+3 → 2d8+3, а не (1d8+3)×2.
        let dice_sum: i32 = rolled.rolls.iter().sum();
        let flat = rolled.total - dice_sum; // со знаком, поэтому «1d8-1» тоже верно
        let damage = if check.crit == Some(Crit::Success) { dice_sum * 2 + flat } else { rolled.total }.max(1);

        let Some(target) = actors.get_mut(&attack.target_id) else { continue };
        target.rpg.hp.current = clamp_stat(target.rpg.hp.current - damage, -999, target.rpg.hp.max);
        let hp_line = render(
            &labels.damage,
            &[
                ("name", &target_name),
                ("damage", &damage.to_string()),
                ("notation", &notation),
                (
                    "crit",
                    if check.crit == Some(Crit::Success) { &labels.crit_doubled } else { "" },
                ),
                ("hp", &target.rpg.hp.current.max(0).to_string()),
                ("max", &target.rpg.hp.max.to_string()),
            ],
        );
        let died = target.rpg.hp.current <= 0 && !target.rpg.dead;
        if died {
            target.rpg.dead = true;
        }
        changed.insert(attack.target_id.clone(), ());
        result.events.push(make_event(EventKind::Hp, hp_line, Some(serde_json::json!({ "damage": damage }))));
        if died {
            result.events.push(make_event(
                EventKind::Death,
                render(&labels.defeated, &[("name", &target_name)]),
                Some(serde_json::json!({ "characterId": attack.target_id })),
            ));
        }
    }

    // Проверки характеристик.
    for roll in &update.rolls {
        let actor_id = match roll.actor_id.as_deref() {
            Some(id) if actors.contains_key(id) => Some(id.to_string()),
            _ => actors.keys().next().cloned(),
        };
        let ability = roll.ability.unwrap_or(Ability::Str);
        let (score, name) = match actor_id.as_deref().and_then(|id| actors.get(id)) {
            Some(actor) => (actor.rpg.stats.get(ability), actor.name.clone()),
            None => (10, labels.player.clone()),
        };
        let check = roll_check(score, roll.dc, 0);
        let label = roll.label.clone().unwrap_or_else(|| ability.label_ru().to_string());
        let verdict = match check.crit {
            Some(Crit::Success) => &labels.verdict.crit_success,
            Some(Crit::Fail) => &labels.verdict.crit_fail,
            None if check.success => &labels.verdict.success,
            None => &labels.verdict.failure,
        };
        result.events.push(make_event(
            EventKind::Roll,
            render(
                &labels.check,
                &[
                    ("name", &name),
                    ("label", &label),
                    ("d20", &check.d20.to_string()),
                    ("mod", &fmt_mod(check.modifier)),
                    ("total", &check.total.to_string()),
                    ("dc", &check.dc.to_string()),
                    ("verdict", verdict),
                ],
            ),
            Some(serde_json::json!({ "roll": roll, "result": check })),
        ));
    }

    // Изменения HP.
    for delta in &update.hp_delta {
        let resolver = Resolver { actors, hero_id: opts.hero_id.clone() };
        let Some(target_id) = resolver.resolve_actor_id(delta.character_id.as_deref()) else { continue };
        let Some(actor) = actors.get_mut(&target_id) else { continue };
        actor.rpg.hp.current = clamp_stat(actor.rpg.hp.current + delta.amount, -999, actor.rpg.hp.max);
        let name = actor.name.clone();
        let sign = if delta.amount >= 0 { "+" } else { "" };
        let heart = if delta.amount >= 0 { "💚" } else { "💔" };
        let reason = delta.reason.as_deref().map(|r| format!(" ({r})")).unwrap_or_default();
        let line = render(
            &labels.hp_change,
            &[
                ("heart", heart),
                ("name", &name),
                ("sign", sign),
                ("amount", &delta.amount.to_string()),
                ("reason", &reason),
                ("hp", &actor.rpg.hp.current.max(0).to_string()),
                ("max", &actor.rpg.hp.max.to_string()),
            ],
        );
        // Лечение, поднявшее павшего выше нуля, возвращает его в строй — иначе персонаж
        // выглядел бы живым на полоске HP, но навсегда оставался помеченным мёртвым.
        let revived = actor.rpg.dead && actor.rpg.hp.current > 0;
        let died = !actor.rpg.dead && actor.rpg.hp.current <= 0;
        if revived {
            actor.rpg.dead = false;
        }
        if died {
            actor.rpg.dead = true;
        }
        changed.insert(target_id.clone(), ());
        result.events.push(make_event(EventKind::Hp, line, Some(serde_json::json!({ "delta": delta }))));
        if revived {
            result.events.push(make_event(
                EventKind::Hp,
                render(&labels.revived, &[("name", &name)]),
                Some(serde_json::json!({ "characterId": target_id })),
            ));
        } else if died {
            result.events.push(make_event(
                EventKind::Death,
                render(&labels.died, &[("name", &name)]),
                Some(serde_json::json!({ "characterId": target_id })),
            ));
        }
    }

    // Лут.
    for grant in &update.grant_items {
        let owner_id = match grant.owner_id.as_deref() {
            Some(id) if actors.contains_key(id) => Some(id.to_string()),
            _ => actors.keys().next().cloned(),
        };
        let item = Item {
            id: new_id(),
            owner_id,
            name: grant.name.clone(),
            slot: grant.slot.unwrap_or_default(),
            rarity: grant.rarity.unwrap_or_default(),
            description: grant.description.clone(),
            damage: grant.damage.clone(),
            modifiers: grant.modifiers.unwrap_or_default(),
            equipped: false,
            qty: grant.qty.filter(|qty| *qty > 0).unwrap_or(1),
            image_url: None,
            image_prompt_en: grant
                .image_prompt_en
                .as_deref()
                .map(str::trim)
                .filter(|s| !s.is_empty())
                .map(str::to_string),
            created_at: now_iso(),
        };
        let qty_suffix = if item.qty > 1 { format!(", ×{}", item.qty) } else { String::new() };
        let text = render(
            &labels.item_gained,
            &[
                ("name", &item.name),
                ("rarity", labels.rarity(item.rarity)),
                ("qty", &qty_suffix),
            ],
        );
        // with_image говорит клиенту сгенерировать портрет дропа; он же станет референсом.
        let data = serde_json::json!({
            "item": serde_json::to_value(&item).unwrap_or(serde_json::Value::Null),
            "withImage": grant.with_image == Some(true),
        });
        result.events.push(make_event(EventKind::Item, text, Some(data)));
        result.items.push(item);
    }

    // Сначала снимаем объявленные эффекты, потом накладываем новые.
    for clear in &update.clear_effects {
        let resolver = Resolver { actors, hero_id: opts.hero_id.clone() };
        let Some(id) = resolver.resolve_actor_id(clear.character_id.as_deref()) else { continue };
        let Some(actor) = actors.get_mut(&id) else { continue };
        let before = actor.rpg.effects.len();
        if clear.name.trim() == "*" {
            actor.rpg.effects.clear();
        } else {
            let key = clear.name.trim().to_lowercase();
            actor.rpg.effects.retain(|effect| effect.name.trim().to_lowercase() != key);
        }
        if actor.rpg.effects.len() != before {
            changed.insert(id, ());
        }
    }
    for decl in &update.apply_effects {
        let resolver = Resolver { actors, hero_id: opts.hero_id.clone() };
        let Some(id) = resolver.resolve_actor_id(decl.character_id.as_deref()) else { continue };
        let Some(effect) = effect_from_decl(decl) else { continue };
        let Some(actor) = actors.get_mut(&id) else { continue };
        let icon = if effect.kind == EffectKind::Debuff { "🔻" } else { "✨" };
        let line = effect_summary(&effect.name, &effect.modifiers, effect.turns, labels);
        merge_effect(&mut actor.rpg, effect);
        changed.insert(id, ());
        result.events.push(make_event(EventKind::Effect, format!("{icon} {line}"), None));
    }

    // Случайное благословение или проклятие может настигнуть героя за ход.
    if opts.random_events {
        let id = match opts.hero_id.as_deref() {
            Some(hero) if actors.contains_key(hero) => Some(hero.to_string()),
            _ => actors.keys().next().cloned(),
        };
        if let Some(id) = id {
            if let Some(actor) = actors.get_mut(&id) {
                if let Some(effect) = roll_random_event(&mut actor.rpg, labels) {
                    changed.insert(id, ());
                    let icon = if effect.kind == EffectKind::Debuff { "🌑" } else { "🌟" };
                    let note = effect.note.clone().unwrap_or_default();
                    result.events.push(make_event(
                        EventKind::Effect,
                        render(
                            &labels.random_event,
                            &[
                                ("icon", icon),
                                (
                                    "summary",
                                    &effect_summary(
                                        &effect.name,
                                        &effect.modifiers,
                                        effect.turns,
                                        labels,
                                    ),
                                ),
                                ("note", &note),
                            ],
                        ),
                        None,
                    ));
                }
            }
        }
    }

    if let Some(note) = update.note.as_deref().map(str::trim).filter(|note| !note.is_empty()) {
        // Модель иногда пишет сюда собственные размышления: «нужно объявить проверку»,
        // «исход: успех». Такое игроку показывать нельзя — во-первых, это не событие
        // истории, во-вторых, исход решает движок, а не пересказ.
        if !looks_like_deliberation(note) && !looks_like_identifier(note) {
            result.events.push(make_event(EventKind::Note, note.to_string(), None));
        }
    }

    result.changed = changed.into_keys().collect();
    result
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::{AttackDecl, HpDeltaDecl, Hp, SpawnDecl};

    fn actor(name: &str, hp: i32, ac: i32) -> Actor {
        let mut rpg = CharacterRpg::default();
        rpg.hp = Hp { current: hp, max: hp };
        rpg.ac = ac;
        Actor { name: name.into(), rpg }
    }

    fn cast() -> ActorMap {
        let mut actors = ActorMap::new();
        actors.insert("hero".into(), actor("Герой", 30, 15));
        actors.insert("foe".into(), actor("Гоблин", 12, 10));
        actors
    }

    #[test]
    fn spawned_enemy_can_be_attacked_in_the_same_turn() {
        let mut actors = ActorMap::new();
        actors.insert("hero".into(), actor("Герой", 30, 15));
        let update = GameUpdate {
            spawn_enemies: vec![SpawnDecl { name: "Скелет".into(), hp: Some(9), ac: Some(1), ..Default::default() }],
            ..Default::default()
        };
        let result = apply_game_update(&update, &mut actors, &ApplyOptions::default());
        assert_eq!(result.spawned_enemies.len(), 1);
        assert_eq!(actors.len(), 2, "заспавненный враг обязан попасть в состав хода");
    }

    #[test]
    fn an_unknown_but_present_attacker_id_is_skipped_not_redirected_to_the_hero() {
        let mut actors = cast();
        let update = GameUpdate {
            attacks: vec![AttackDecl {
                attacker_id: Some("призрак-которого-нет".into()),
                target_id: "hero".into(),
                damage: Some("1d6".into()),
                ..Default::default()
            }],
            ..Default::default()
        };
        let result = apply_game_update(&update, &mut actors, &ApplyOptions::default());
        assert!(result.events.is_empty(), "неизвестный атакующий не должен бить вовсе");
        assert_eq!(actors["hero"].rpg.hp.current, 30);
    }

    #[test]
    fn an_attack_never_lands_on_its_own_attacker() {
        let mut actors = cast();
        let update = GameUpdate {
            attacks: vec![AttackDecl {
                attacker_id: Some("hero".into()),
                target_id: "hero".into(),
                ..Default::default()
            }],
            ..Default::default()
        };
        apply_game_update(&update, &mut actors, &ApplyOptions::default());
        assert_eq!(actors["hero"].rpg.hp.current, 30);
    }

    #[test]
    fn a_dead_target_takes_no_further_hits() {
        let mut actors = cast();
        actors["foe"].rpg.dead = true;
        let update = GameUpdate {
            attacks: vec![AttackDecl { attacker_id: Some("hero".into()), target_id: "foe".into(), ..Default::default() }],
            ..Default::default()
        };
        let result = apply_game_update(&update, &mut actors, &ApplyOptions::default());
        assert!(result.events.is_empty());
    }

    #[test]
    fn hp_delta_without_an_id_falls_back_to_the_hero() {
        let mut actors = cast();
        let update = GameUpdate {
            hp_delta: vec![HpDeltaDecl { character_id: Some("ID".into()), amount: -5, reason: Some("падение".into()) }],
            ..Default::default()
        };
        let opts = ApplyOptions { hero_id: Some("hero".into()), ..Default::default() };
        apply_game_update(&update, &mut actors, &opts);
        assert_eq!(actors["hero"].rpg.hp.current, 25, "заглушка вместо id обязана уйти герою");
    }

    #[test]
    fn dropping_to_zero_kills_and_healing_back_revives() {
        let mut actors = cast();
        let kill = GameUpdate {
            hp_delta: vec![HpDeltaDecl { character_id: Some("hero".into()), amount: -40, reason: None }],
            ..Default::default()
        };
        let result = apply_game_update(&kill, &mut actors, &ApplyOptions::default());
        assert!(actors["hero"].rpg.dead);
        assert!(result.events.iter().any(|event| event.kind == EventKind::Death));

        let heal = GameUpdate {
            hp_delta: vec![HpDeltaDecl { character_id: Some("hero".into()), amount: 12, reason: None }],
            ..Default::default()
        };
        let result = apply_game_update(&heal, &mut actors, &ApplyOptions::default());
        assert!(!actors["hero"].rpg.dead, "лечение выше нуля обязано поднимать павшего");
        assert!(result.events.iter().any(|event| event.text.contains("приходит в себя")));
    }

    #[test]
    fn effects_tick_down_and_a_fresh_one_keeps_its_full_duration() {
        let mut actors = cast();
        actors["hero"].rpg.effects = vec![Effect {
            id: "старый".into(),
            name: "Каменная кожа".into(),
            kind: EffectKind::Buff,
            modifiers: Modifiers { ac: Some(2), ..Default::default() },
            turns: 1,
            note: None,
        }];
        let update = GameUpdate {
            apply_effects: vec![ApplyEffectDecl {
                character_id: Some("hero".into()),
                name: "Ясность ума".into(),
                turns: Some(3),
                ..Default::default()
            }],
            ..Default::default()
        };
        let result = apply_game_update(&update, &mut actors, &ApplyOptions::default());
        assert!(result.events.iter().any(|event| event.text.contains("развеялся")));
        let effects = &actors["hero"].rpg.effects;
        assert_eq!(effects.len(), 1);
        assert_eq!(effects[0].name, "Ясность ума");
        assert_eq!(effects[0].turns, 3, "наложенный этим же ходом эффект не должен терять ход");
    }

    #[test]
    fn a_same_named_effect_refreshes_instead_of_stacking() {
        let mut actors = cast();
        let decl = ApplyEffectDecl {
            character_id: Some("hero".into()),
            name: "Щит".into(),
            turns: Some(2),
            ..Default::default()
        };
        let update = GameUpdate { apply_effects: vec![decl.clone(), decl], ..Default::default() };
        apply_game_update(&update, &mut actors, &ApplyOptions::default());
        assert_eq!(actors["hero"].rpg.effects.len(), 1);
    }

    #[test]
    fn the_active_effect_list_is_capped() {
        let mut actors = cast();
        let apply_effects = (0..12)
            .map(|i| ApplyEffectDecl {
                character_id: Some("hero".into()),
                name: format!("Эффект {i}"),
                turns: Some(5),
                ..Default::default()
            })
            .collect();
        let update = GameUpdate { apply_effects, ..Default::default() };
        apply_game_update(&update, &mut actors, &ApplyOptions::default());
        assert_eq!(actors["hero"].rpg.effects.len(), EFFECT_CAP);
        assert_eq!(actors["hero"].rpg.effects.last().unwrap().name, "Эффект 11");
    }

    #[test]
    fn clearing_by_star_removes_every_effect() {
        let mut actors = cast();
        actors["hero"].rpg.effects = vec![Effect {
            id: "1".into(),
            name: "Яд".into(),
            kind: EffectKind::Debuff,
            modifiers: Modifiers::default(),
            turns: 5,
            note: None,
        }];
        let update = GameUpdate {
            clear_effects: vec![crate::types::ClearEffectDecl { character_id: Some("hero".into()), name: "*".into() }],
            ..Default::default()
        };
        apply_game_update(&update, &mut actors, &ApplyOptions::default());
        assert!(actors["hero"].rpg.effects.is_empty());
    }

    #[test]
    fn granted_loot_is_unequipped_and_defaults_to_one() {
        let mut actors = cast();
        let update = GameUpdate {
            grant_items: vec![crate::types::GrantItemDecl {
                name: "Ржавый меч".into(),
                ..Default::default()
            }],
            ..Default::default()
        };
        let result = apply_game_update(&update, &mut actors, &ApplyOptions::default());
        assert_eq!(result.items.len(), 1);
        assert_eq!(result.items[0].qty, 1);
        assert!(!result.items[0].equipped, "дроп не должен надеваться сам");
        assert_eq!(result.items[0].owner_id.as_deref(), Some("hero"));
    }

    #[test]
    fn a_note_becomes_a_journal_entry() {
        let mut actors = cast();
        let update = GameUpdate { note: Some("  Ветер стих.  ".into()), ..Default::default() };
        let result = apply_game_update(&update, &mut actors, &ApplyOptions::default());
        let note = result.events.iter().find(|event| event.kind == EventKind::Note).unwrap();
        assert_eq!(note.text, "Ветер стих.");
    }

    #[test]
    fn plural_forms_of_turns_follow_russian_grammar() {
        let turns = JournalLabels::default().turns;
        assert_eq!(turns.pick(1), "ход");
        assert_eq!(turns.pick(2), "хода");
        assert_eq!(turns.pick(5), "ходов");
        assert_eq!(turns.pick(11), "ходов");
        assert_eq!(turns.pick(21), "ход");
        assert_eq!(turns.pick(112), "ходов");
    }
}

#[cfg(test)]
mod note_tests {
    use super::looks_like_deliberation;

    #[test]
    fn a_models_plan_is_not_shown_to_the_player() {
        assert!(looks_like_deliberation(
            "Сцена требует проверки скрытности. Нужно объявить проверку Скрытности против              Внимательности. Исход: успех — остаётся незамеченным."
        ));
    }

    #[test]
    fn a_machine_key_never_reaches_the_journal() {
        use super::looks_like_identifier;
        assert!(looks_like_identifier("scene_mariana_bania_hint"));
        assert!(looks_like_identifier("hp-delta.note"));
        assert!(!looks_like_identifier("Дождь усиливается"));
        assert!(!looks_like_identifier("Трактирщик кивает"));
    }

    #[test]
    fn a_real_story_note_survives() {
        assert!(!looks_like_deliberation("Дождь усиливается, следы на глине быстро размывает."));
        assert!(!looks_like_deliberation("Трактирщик запомнил твоё лицо."));
    }
}
