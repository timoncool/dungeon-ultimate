import type { ResponseLength } from "@/lib/types";

// Every model-facing instruction string, per language. The narrator, the RPG
// rules, the "surprise me" generators, the quick-action chips and the kickoff/
// continue directives all come from here in the player's chosen language, so the
// model is instructed IN that language instead of in Russian with a "write in X"
// patch. Image-generation instructions stay English (the FLUX worker is English).
export type PromptSet = {
  // The default narrator system prompt (used when the chat has no custom one).
  narrator: string;
  // Folded in only when the story should genuinely conclude.
  ending: string;
  // The optional cynical in-world companion voice.
  companion: string;
  // Shown when image generation is off.
  imageDisabled: string;
  // Per response-length hint.
  responseLength: Record<ResponseLength, string>;
  // Anti-repetition nudge pieces (assembled in code from the recent passages).
  // motifsPrefix is followed in code by the comma-joined motif list.
  antiRepetition: {
    header: string;
    recentOpenings: string;
    motifsPrefix: string;
    varyOpening: string;
  };
  // Section labels for the assembled system context (pure strings; the code adds
  // the colons + values).
  labels: {
    world: string;
    worldFallback: string;
    style: string;
    styleFallback: string;
    storySoFar: string;
    savedCharacters: string;
    noCharacters: string;
    charId: string;
    charName: string;
    charDetails: string;
    charInventory: string;
    charSkills: string;
    charSpells: string;
    portraitAvailable: string;
    portraitUnavailable: string;
    attachments: string;
  };
  // RPG (D&D mode): the strict rules block + the live state-block labels.
  rpg: {
    rules: string;
    inventory: string; // "ИНВЕНТАРЬ"
    foes: string; // "ПРОТИВНИКИ (...)"
    hp: string; // "HP" / "ОЗ"
    ac: string; // "КЗ" / "AC"
    level: string; // "ур." / "lvl"
    dead: string; // "МЁРТВ"
    equipped: string; // "надет"
    damage: string; // "урон"
    conditions: string; // "Состояния"
    effects: string; // "Эффекты"
  };
  // "Surprise me" field generator.
  suggest: {
    system: string;
    fields: { world: string; style: string; character: string; opening: string };
  };
  // Quick-action chips.
  actions: { system: string };
  // Directives sent as the user turn for a model-written opening / continuation.
  kickoff: string;
  continue: string;
};
