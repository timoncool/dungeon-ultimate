import type { PromptSet } from "./types";

// English. Only the player-side directives live here: every model-facing prompt
// moved to the server (crates/du-prompts/assets) with the Rust port.
const en: PromptSet = {
  kickoff:
    "Begin the story right now. Write the opening passage: establish the scene, the player's character and the immediate situation in the second person, ending on a moment that invites the player's first action. Don't ask the player setup questions; the story has already begun.",

  continue:
    "Continue the story exactly where it left off. The player takes no action this turn — develop the scene naturally through narration, dialogue or events, then pause on a moment that invites their next action.",

  playerDirection: "The player's direction for the opening (build the scene around it): ",
};

export default en;
