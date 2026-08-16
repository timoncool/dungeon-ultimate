//! Сборка промпта кадра и чистка служебных утечек из прозы.

/// Указание, которое дописывается перед промптом, когда движок ЭВОЛЮЦИОНИРУЕТ предыдущий
/// кадр локации, а не рисует её заново. Надёжный рычаг при редактировании — назвать то, что
/// меняться НЕ должно: пропущенное как раз и уплывает. Что изменилось, скажет сам промпт.
const EDIT_CONTINUITY: &str = "Continue the EXACT same scene as the reference image: keep the same location, layout, composition, camera angle, lighting, and color palette. Change only what the following description adds or alters.";

/// Дописать перед промптом стилевой замок чата, чтобы весь арт держал один вид. Идемпотентно.
pub fn apply_style_prefix(prompt: &str, style_prefix: &str) -> String {
    let prefix = style_prefix.trim();
    let prompt = prompt.trim();
    if prefix.is_empty() {
        return prompt.to_string();
    }
    if prompt.to_lowercase().starts_with(&prefix.to_lowercase()) {
        return prompt.to_string();
    }
    // Если замок уже кончается знаком препинания, второй не нужен.
    let separator = if prefix.ends_with(['.', '!', '?', ',', ':', ';']) { " " } else { ". " };
    format!("{prefix}{separator}{prompt}")
}

/// Финализировать промпт СЦЕНЫ: стилевой замок плюс жёсткий запрет текста в кадре.
/// Сцены собираются из прозы, полной имён собственных, и без этого запрета модель охотно
/// выгравирует в кадре нечитаемые надписи.
pub fn finalize_scene_prompt(prompt: &str, style_prefix: &str) -> String {
    let styled = apply_style_prefix(prompt, style_prefix);
    let styled = styled.trim_end();
    format!("{styled}. No text, no letters, no words, no captions, no watermark, no UI.")
}

/// Дописать указание непрерывности перед промптом правки. Идемпотентно, поэтому повтор
/// кадра не наращивает его вторым слоем.
pub fn apply_edit_continuity(prompt: &str) -> String {
    let prompt = prompt.trim();
    if prompt.to_lowercase().starts_with("continue the exact same scene") {
        return prompt.to_string();
    }
    format!("{EDIT_CONTINUITY} {prompt}")
}

/// Вырезать из прозы служебные артефакты.
///
/// Кадр создаёт отдельный проход, но локальная модель нет-нет да и впишет инструкцию прямо
/// в текст: выдуманный маркер `[IMAGE_GEN_PROMPT]`, вызов `generate_image[...]`, объект
/// `{"action":"generate_image",…}` или блок в тройных кавычках. Всё это всегда идёт в конце
/// пассажа, поэтому режется вместе с хвостом.
pub fn strip_image_artifacts(text: &str) -> String {
    let mut result = text.to_string();

    // Маркер с ПОДЧЁРКИВАНИЯМИ и всё после него. Форма с подчёркиваниями обязательна, иначе
    // естественная надпись «[IMAGE PROMPT]» в прозе обрезала бы живой текст.
    for marker in ["[IMAGE_GEN_PROMPT]", "[IMAGE_GENERATION_PROMPT]"] {
        if let Some(index) = find_ignore_case(&result, marker) {
            result.truncate(index);
        }
    }
    // Вызов функции, написанный отдельной строкой.
    if let Some(index) = find_call_start(&result) {
        result.truncate(index);
    }
    // Голый JSON-объект вызова.
    if let Some(index) = find_ignore_case(&result, "{\"action\": \"generate_image\"")
        .or_else(|| find_ignore_case(&result, "{\"action\":\"generate_image\""))
    {
        result.truncate(index);
    }
    result = strip_closed_fences(&result);
    // Незакрытый хвостовой блок с полем prompt — режем только по этому признаку, чтобы
    // одиночная обратная кавычка в прозе не съела остаток истории.
    if let Some(index) = result.find("```") {
        if result[index..].contains("\"prompt\"") {
            result.truncate(index);
        }
    }
    result.trim().to_string()
}

/// Начало вызова `generate_image[` или `generate_image{` в начале строки.
fn find_call_start(text: &str) -> Option<usize> {
    let lower = text.to_lowercase();
    let mut offset = 0usize;
    for line in text.split_inclusive('\n') {
        let trimmed_start = offset + (line.len() - line.trim_start().len());
        let lower_line = &lower[offset..offset + line.len()];
        let candidate = lower_line.trim_start();
        let candidate = candidate.strip_prefix("call:").map(str::trim_start).unwrap_or(candidate);
        if let Some(rest) = candidate.strip_prefix("generate_image") {
            if rest.trim_start().starts_with(['[', '{']) {
                return Some(trimmed_start);
            }
        }
        offset += line.len();
    }
    None
}

/// Убрать ЗАКРЫТЫЕ блоки ```lang\n…\n```. Требуем оба ограничителя, чтобы случайная
/// обратная кавычка не удалила остаток истории.
fn strip_closed_fences(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut rest = text;
    while let Some(open) = rest.find("```") {
        let after_open = &rest[open + 3..];
        let Some(newline) = after_open.find('\n') else { break };
        // Между ``` и переводом строки допустим только ярлык языка.
        if !after_open[..newline].chars().all(|c| c.is_ascii_alphanumeric()) {
            break;
        }
        let body = &after_open[newline + 1..];
        let Some(close) = body.find("```") else { break };
        out.push_str(&rest[..open]);
        rest = &body[close + 3..];
    }
    out.push_str(rest);
    out
}

fn find_ignore_case(haystack: &str, needle: &str) -> Option<usize> {
    haystack.to_lowercase().find(&needle.to_lowercase())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_style_prefix_leads_and_never_doubles() {
        assert_eq!(apply_style_prefix("a knight", "grim ink wash"), "grim ink wash. a knight");
        assert_eq!(apply_style_prefix("a knight", "grim ink wash."), "grim ink wash. a knight");
        let once = apply_style_prefix("a knight", "grim ink wash");
        assert_eq!(apply_style_prefix(&once, "grim ink wash"), once, "повтор не должен наращивать замок");
        assert_eq!(apply_style_prefix("a knight", "  "), "a knight");
    }

    #[test]
    fn a_scene_prompt_always_forbids_text_in_frame() {
        let prompt = finalize_scene_prompt("a knight", "");
        assert!(prompt.ends_with("No text, no letters, no words, no captions, no watermark, no UI."));
        assert!(prompt.starts_with("a knight"));
    }

    #[test]
    fn the_edit_directive_is_idempotent() {
        let once = apply_edit_continuity("goblins arrive");
        assert!(once.starts_with("Continue the EXACT same scene"));
        assert_eq!(apply_edit_continuity(&once), once);
    }

    #[test]
    fn an_invented_image_marker_and_its_tail_are_cut() {
        let text = "Дверь скрипнула.\n[IMAGE_GEN_PROMPT] a dark door, cinematic";
        assert_eq!(strip_image_artifacts(text), "Дверь скрипнула.");
    }

    #[test]
    fn a_natural_bracketed_phrase_is_not_mistaken_for_the_marker() {
        // Табличка в прозе не должна обрезать пассаж: маркер узнаётся по подчёркиваниям.
        let text = "На стене висела табличка [IMAGE PROMPT], выцветшая от времени.";
        assert_eq!(strip_image_artifacts(text), text);
    }

    #[test]
    fn a_leaked_function_call_is_cut() {
        let text = "Он шагнул вперёд.\ngenerate_image{\"prompt\": \"a man\"}";
        assert_eq!(strip_image_artifacts(text), "Он шагнул вперёд.");
        let text = "Он шагнул вперёд.\ncall: generate_image[\"a man\"]";
        assert_eq!(strip_image_artifacts(text), "Он шагнул вперёд.");
    }

    #[test]
    fn a_closed_code_fence_is_removed_but_the_story_survives() {
        let text = "Начало.\n```json\n{\"a\":1}\n```\nПродолжение.";
        assert_eq!(strip_image_artifacts(text), "Начало.\n\nПродолжение.");
    }

    #[test]
    fn a_stray_backtick_run_never_eats_the_rest_of_the_story() {
        let text = "Он сказал ``` и замолчал. История продолжается ещё долго.";
        assert_eq!(strip_image_artifacts(text), text);
    }

    #[test]
    fn an_unterminated_json_payload_with_a_prompt_field_is_cut() {
        let text = "Финал сцены.\n```json\n{\"prompt\": \"a dark door";
        assert_eq!(strip_image_artifacts(text), "Финал сцены.");
    }

    #[test]
    fn a_bare_tool_object_is_cut() {
        let text = "Тишина.\n{\"action\":\"generate_image\",\"prompt\":\"x\"}";
        assert_eq!(strip_image_artifacts(text), "Тишина.");
    }
}
