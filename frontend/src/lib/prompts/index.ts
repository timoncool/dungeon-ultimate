import type { Language } from "@/lib/types";
import type { PromptSet } from "./types";
import ru from "./ru";
import en from "./en";
import es from "./es";
import fr from "./fr";
import de from "./de";
import zh from "./zh";
import ja from "./ja";

export type { PromptSet } from "./types";

// Every model-facing prompt, keyed by language. The narrator, RPG rules, the
// "surprise me" generators, the quick-action chips and the kickoff/continue
// directives are all pulled from here in the player's chosen language.
export const PROMPTS: Record<Language, PromptSet> = { ru, en, es, fr, de, zh, ja };

export function promptsFor(language: Language): PromptSet {
  return PROMPTS[language] ?? PROMPTS.ru;
}
