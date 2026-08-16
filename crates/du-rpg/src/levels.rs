//! Опыт и уровни героя.
//!
//! Поля `level` и `xp` в листе персонажа были с самого начала, но их никто не менял: в
//! игре стояла вечная «ур. 1» и не было способа вырасти. Здесь — единственное место, где
//! опыт превращается в уровень.
//!
//! Кривая нарочно пологая: история из десятка ходов должна дать почувствовать рост, а не
//! упереться в стену. На второй уровень нужно 100 опыта, на третий — ещё 200, дальше шаг
//! растёт на сотню за уровень.

/// Сколько опыта нужно НА СЛЕДУЮЩИЙ уровень, начиная с текущего.
pub fn xp_to_next(level: i32) -> i32 {
    100 * level.max(1)
}

/// Сколько всего опыта отделяет начало игры от данного уровня.
pub fn xp_for_level(level: i32) -> i32 {
    (1..level.max(1)).map(xp_to_next).sum()
}

/// Прибавка к запасу сил за уровень.
pub const HP_PER_LEVEL: i32 = 6;

/// Новый уровень по накопленному опыту.
pub fn level_for_xp(xp: i32) -> i32 {
    let mut level = 1;
    while xp >= xp_for_level(level + 1) {
        level += 1;
    }
    level
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_first_level_up_costs_a_hundred() {
        assert_eq!(xp_for_level(1), 0);
        assert_eq!(xp_for_level(2), 100);
        assert_eq!(level_for_xp(0), 1);
        assert_eq!(level_for_xp(99), 1);
        assert_eq!(level_for_xp(100), 2);
    }

    #[test]
    fn each_level_costs_more_than_the_one_before() {
        assert_eq!(xp_for_level(3), 300);
        assert_eq!(xp_for_level(4), 600);
        assert!(xp_to_next(3) > xp_to_next(2));
    }

    #[test]
    fn a_big_haul_can_skip_a_level() {
        // 350 опыта разом — это сразу третий, а не второй с остатком.
        assert_eq!(level_for_xp(350), 3);
    }
}
