import type { PromptSet } from "./types";

// Spanish. Only the player-side directives live here: every model-facing prompt
// moved to the server (crates/du-prompts/assets) with the Rust port.
const es: PromptSet = {
  kickoff:
    "Empieza la historia ahora mismo. Escribe un fragmento introductorio: establece la escena, el personaje del jugador y la situación inmediata en segunda persona, terminando en un momento que invite a la primera acción del jugador. No le hagas preguntas de configuración al jugador; la historia ya ha comenzado.",

  continue:
    "Continúa la historia justo donde se interrumpió. El jugador no realiza ninguna acción en este turno — desarrolla la escena con naturalidad mediante la narración, el diálogo o los eventos, y luego haz una pausa en un momento que invite a su siguiente acción.",

  playerDirection: "Indicación del jugador para el inicio (construye la escena en torno a ella): ",
};

export default es;
