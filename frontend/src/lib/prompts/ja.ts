import type { PromptSet } from "./types";

// Japanese. Only the player-side directives live here: every model-facing prompt
// moved to the server (crates/du-prompts/assets) with the Rust port.
const ja: PromptSet = {
  kickoff:
    "今すぐ物語を始めること。導入の一節を書く:場面、プレイヤーのキャラクター、そして差し迫った状況を二人称で設定し、プレイヤーの最初の行動を誘う瞬間で締めること。設定について質問しないこと。物語はすでに始まっている。",

  continue:
    "中断したまさにその地点から物語を続けること。この手番でプレイヤーは行動しない——語り・会話・出来事を通じて場面を自然に展開させ、それからプレイヤーの次の行動を誘う瞬間で間を置くこと。",

  playerDirection: "プレイヤーによる冒頭の指示（これを軸に場面を組み立てること）：",

  checkResolved:
    "判定が決まりました：{outcome}。この結果が物語にもたらす具体的な帰結を描写してください。成功なら好ましい展開、失敗なら厄介事・罠・つまずきを。必要ならメカニクスを通じてダメージや戦利品を反映してください。プレイヤーの次の行動を促す瞬間で締めること。この段落で新たな判定を持ち込まないこと。",
};

export default ja;
