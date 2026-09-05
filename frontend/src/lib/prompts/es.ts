import type { PromptSet } from "./types";

// Spanish. Only the player-side directives live here: every model-facing prompt
// moved to the server (crates/du-prompts/assets) with the Rust port.
const es: PromptSet = {
  kickoff:
    "Empieza la historia ahora mismo. Escribe un fragmento introductorio: establece la escena, el personaje del jugador y la situación inmediata en segunda persona, terminando en un momento que invite a la primera acción del jugador. No le hagas preguntas de configuración al jugador; la historia ya ha comenzado.",

  continue:
    "Continúa la historia justo donde se interrumpió. El jugador no realiza ninguna acción en este turno — desarrolla la escena con naturalidad mediante la narración, el diálogo o los eventos, y luego haz una pausa en un momento que invite a su siguiente acción.",

  playerDirection: "Indicación del jugador para el inicio (construye la escena en torno a ella): ",

  checkResolved:
    "Acaba de resolverse una tirada: {outcome}. Describe la consecuencia CONCRETA de este resultado en la historia (con éxito, un desenlace favorable; con fallo, una complicación, una trampa o un revés) y, si hace falta, aplica daño o botín a través de la mecánica. Termina en un momento que invite a la siguiente acción del jugador. NO introduzcas una nueva tirada en este pasaje.",
};

export default es;
