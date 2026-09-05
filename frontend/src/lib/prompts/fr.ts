import type { PromptSet } from "./types";

// French. Only the player-side directives live here: every model-facing prompt
// moved to the server (crates/du-prompts/assets) with the Rust port.
const fr: PromptSet = {
  kickoff:
    "Commence l'histoire maintenant. Écris un passage d'introduction : plante le décor, le personnage du joueur et la situation immédiate, à la deuxième personne, en terminant sur un moment qui invite la première action du joueur. Ne pose pas de questions de configuration au joueur ; l'histoire a déjà commencé.",

  continue:
    "Poursuis l'histoire exactement là où elle s'est interrompue. Le joueur n'accomplit aucune action à ce tour — développe la scène naturellement par la narration, le dialogue ou les événements, puis fais une pause sur un moment qui invite sa prochaine action.",

  playerDirection: "Indication du joueur pour le début (construis la scène autour) : ",

  checkResolved:
    "Un jet vient d'être résolu : {outcome}. Décris la conséquence CONCRÈTE de ce résultat dans l'histoire — en cas de réussite, une issue favorable ; en cas d'échec, une complication, un piège ou un revers — et, si nécessaire, applique dégâts ou butin via la mécanique. Termine sur un moment qui appelle la prochaine action du joueur. N'introduis PAS de nouveau jet dans ce passage.",
};

export default fr;
