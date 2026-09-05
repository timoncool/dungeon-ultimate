import type { PromptSet } from "./types";

// Chinese. Only the player-side directives live here: every model-facing prompt
// moved to the server (crates/du-prompts/assets) with the Rust port.
const zh: PromptSet = {
  kickoff:
    "现在就开始故事。写下开场段落：用第二人称确立场景、玩家的角色和眼前的处境，收尾于一个邀请玩家首次行动的时刻。不要向玩家提设置类的问题；故事已经开始了。",

  continue:
    "从故事中断之处接着往下写。玩家本回合不采取行动——通过叙述、对话或事件自然地推进场景，然后停在一个邀请他下一步行动的时刻。",

  playerDirection: "玩家对开场的指引（围绕它构建场景）：",
};

export default zh;
