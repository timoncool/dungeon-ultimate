import type { PromptSet } from "./types";

// German. Only the player-side directives live here: every model-facing prompt
// moved to the server (crates/du-prompts/assets) with the Rust port.
const de: PromptSet = {
  kickoff:
    "Beginne die Geschichte jetzt sofort. Schreibe einen einleitenden Abschnitt: Etabliere die Szene, die Spielerfigur und die unmittelbare Situation in der zweiten Person und schließe mit einem Moment, der die erste Handlung des Spielers einlädt. Stelle dem Spieler keine Fragen zur Einrichtung; die Geschichte hat bereits begonnen.",

  continue:
    "Setze die Geschichte genau dort fort, wo sie unterbrochen wurde. Der Spieler führt in diesem Zug keine Handlung aus — entwickle die Szene natürlich durch Erzählung, Dialog oder Ereignisse weiter und halte dann bei einem Moment inne, der seine nächste Handlung einlädt.",

  playerDirection: "Vorgabe des Spielers für den Anfang (baue die Szene darum herum): ",
};

export default de;
