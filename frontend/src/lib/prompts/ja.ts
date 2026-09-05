import type { PromptSet } from "./types";

// Japanese. Only the player-side directives live here: every model-facing prompt
// moved to the server (crates/du-prompts/assets) with the Rust port.
const ja: PromptSet = {
  kickoff:
    "今すぐ物語を始めること。導入の一節を書く:場面、プレイヤーのキャラクター、そして差し迫った状況を二人称で設定し、プレイヤーの最初の行動を誘う瞬間で締めること。設定について質問しないこと。物語はすでに始まっている。",

  continue:
    "中断したまさにその地点から物語を続けること。この手番でプレイヤーは行動しない——語り・会話・出来事を通じて場面を自然に展開させ、それからプレイヤーの次の行動を誘う瞬間で間を置くこと。",

  playerDirection: "プレイヤーによる冒頭の指示（これを軸に場面を組み立てること）：",
};

export default ja;
