// Minimal ambient types for @3d-dice/dice-box-threejs (ships no declarations).
// Only the surface we use: construct, initialize, roll with `@`-forced results.
declare module "@3d-dice/dice-box-threejs" {
  export interface DiceBoxConfig {
    assetPath?: string;
    framerate?: number;
    sounds?: boolean;
    volume?: number;
    shadows?: boolean;
    theme_surface?: string;
    sound_dieMaterial?: string;
    theme_customColorset?: unknown;
    theme_colorset?: string;
    theme_texture?: string;
    theme_material?: string;
    gravity_multiplier?: number;
    light_intensity?: number;
    baseScale?: number;
    strength?: number;
    iterationLimit?: number;
    onRollComplete?: (result: unknown) => void;
  }

  export default class DiceBox {
    constructor(selector: string, config?: DiceBoxConfig);
    initialize(): Promise<void>;
    loadTheme(opts: { colorset?: string; texture?: string; material?: string }): Promise<void>;
    roll(notation: string): Promise<unknown>;
    reroll(ids: number[]): Promise<unknown>;
    add(notation: string): Promise<unknown>;
    remove(ids: number[]): Promise<unknown>;
    clearDice(): void;
  }
}
