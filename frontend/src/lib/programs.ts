// Названия программ timoncool по id, под которым они регистрируются (studio-hub-client CONTRACT.md), для «взят из ...».
export const PROGRAM_NAMES: Record<string, string> = {
  yue2: "YuE2 Studio",
  minimax: "MiniMax Music3 Studio",
  ace: "ACE-Step Studio",
  dub: "Dub Studio",
  dungeon: "Dungeon Ultimate",
  bulka: "Bulka",
  videosos: "VideoSOS",
};

export const programName = (id: string): string => PROGRAM_NAMES[id] ?? id;
