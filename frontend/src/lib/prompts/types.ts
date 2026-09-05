// Directives sent as the player's turn. Everything else the model reads (narrator,
// RPG rules, suggestion generators) lives on the server in crates/du-prompts —
// the old client copy was a dead duplicate that had already drifted from it.
export type PromptSet = {
  // Opening turn of a fresh story.
  kickoff: string;
  // "Continue without me" turn.
  continue: string;
  // Prefix for the player's own hint about how the story should open.
  playerDirection: string;
};
