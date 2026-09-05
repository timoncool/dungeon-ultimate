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

// Player-side directives keyed by language. Model-facing prompts live on the server.
export const PROMPTS: Record<Language, PromptSet> = { ru, en, es, fr, de, zh, ja };

export function promptsFor(language: Language): PromptSet {
  return PROMPTS[language] ?? PROMPTS.ru;
}
