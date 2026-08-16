import type { StoryCharacter, StorySettings } from "@/lib/types";

// ---------------------------------------------------------------------------
// Multi-voice TTS hook (design seam).
//
// The narrator writes one Russian passage per turn; today it is read aloud in a
// single voice (StorySettings.voice). This module lets a caller map dialogue to
// per-character voices WITHOUT breaking that single-voice path:
//
//   • voiceForCharacter() — the voice id to read a known speaker's lines in:
//     the character's explicit voice, else a deterministic voice auto-assigned
//     from the available pool, else the narrator's single voice (multi-voice off
//     or no pool).
//   • detectSpeaker() — best-effort attribution of a dialogue line to a saved
//     character by name, so a future per-line player (or the TTS route, given a
//     characterId) can choose a voice. Returns null when unsure — the caller
//     then uses the narrator voice, exactly as before.
//   • splitDialogueSegments() — chunk a passage into narration vs. quoted
//     dialogue runs, each tagged with the voice that should read it. With
//     multi-voice off (or no character match) this collapses to a single
//     narrator-voiced segment, so the existing whole-passage path is unchanged.
//
// No network calls and no TTS-server dependency live here: this only decides
// WHICH voice id each chunk should use. POST /api/tts still performs synthesis,
// one { text, voice } request at a time, so wiring any of this up is additive.
// Keep this module free of node-only imports so the client can import it too.
// ---------------------------------------------------------------------------

// Filesystem-safe slug for a value used in a filename (TTS WAVs, uploaded
// clones). Pure string fn shared by the tts/tts-voice routes.
export function safeName(value: string, max = 120): string {
  return value.replace(/[^a-zA-Z0-9_-]/g, "_").slice(0, max);
}

// Stable 32-bit hash (FNV-1a) of a string. Deterministic across runs/sessions so
// a character always maps to the same auto-assigned voice.
function hashString(value: string): number {
  let hash = 0x811c9dc5;
  for (let i = 0; i < value.length; i += 1) {
    hash ^= value.charCodeAt(i);
    hash = Math.imul(hash, 0x01000193);
  }
  return hash >>> 0;
}

// Voices a character may be auto-assigned: the available pool minus the
// narrator's voice, so an unset character never collides with narration.
function speakerPool(voicePool: readonly string[], narratorVoice: string): string[] {
  const seen = new Set<string>();
  const pool: string[] = [];
  for (const raw of voicePool) {
    const voice = raw.trim();
    if (voice && voice !== narratorVoice && !seen.has(voice)) {
      seen.add(voice);
      pool.push(voice);
    }
  }
  return pool;
}

// A character's stable position among its chat's characters, sorted by id so it
// is deterministic across sessions. Drives distinct auto-voice assignment.
// Falls back to a stable hash when the character isn't in the list (e.g. preview
// before persist), so the result is always a valid non-negative index.
export function sortedCharacterIndex(
  characterId: string,
  characters: ReadonlyArray<Pick<StoryCharacter, "id">>,
): number {
  const ids = characters.map((c) => c.id).sort();
  const at = ids.indexOf(characterId);
  return at === -1 ? hashString(characterId) : at;
}

// Deterministic voice for a character with no explicit voice set: pick from the
// pool by the character's stable index, so the first pool.length characters get
// DISTINCT voices (only wrapping/colliding once the pool is exhausted). Returns
// null when the pool is empty.
export function autoVoiceForCharacter(
  index: number,
  voicePool: readonly string[],
  narratorVoice: string,
): string | null {
  const pool = speakerPool(voicePool, narratorVoice);
  if (!pool.length) {
    return null;
  }
  return pool[index % pool.length];
}

export type VoiceSegment = {
  text: string;
  voice: string;
  // The character this run is attributed to, when known (null for narration).
  characterId: string | null;
  // "quote" runs are spoken dialogue (never folded into narration); "narration"
  // is everything else.
  kind: "narration" | "quote";
};

// The voice a specific character should be read in. Honors multi-voice only when
// enabled; then prefers the character's explicit voice, else a distinct voice
// auto-assigned from `voicePool` by the character's position in `characters`,
// else the single narrator voice. Pass the full character list for distinct
// assignment; omit it and auto-assignment falls back to a stable hash. Safe to
// call unconditionally.
export function voiceForCharacter(
  settings: Pick<StorySettings, "voice" | "multiVoice">,
  character: Pick<StoryCharacter, "id" | "voice"> | null | undefined,
  voicePool: readonly string[] = [],
  characters: ReadonlyArray<Pick<StoryCharacter, "id">> = [],
): string {
  if (settings.multiVoice && character) {
    if (character.voice && character.voice.trim()) {
      return character.voice.trim();
    }
    const index = sortedCharacterIndex(character.id, characters);
    const auto = autoVoiceForCharacter(index, voicePool, settings.voice);
    if (auto) {
      return auto;
    }
  }
  return settings.voice;
}

// Index characters by a normalized name for cheap speaker lookup. Longer names
// first so "Captain Mara" wins over "Mara" when both are present.
function charactersByName(
  characters: Array<Pick<StoryCharacter, "id" | "name" | "voice">>,
): Array<{ id: string; nameLower: string; voice?: string }> {
  return characters
    .map((character) => ({
      id: character.id,
      nameLower: character.name.trim().toLowerCase(),
      voice: character.voice,
    }))
    .filter((entry) => entry.nameLower.length > 0)
    .sort((a, b) => b.nameLower.length - a.nameLower.length);
}

// Does `name` occur as a whole word in `haystackLower`? ASCII gets a \b guard so
// short Latin names don't match inside other words; Cyrillic/other uses a plain
// substring with a length floor (JS \b is unreliable outside ASCII). Mirrors the
// matching rule already used for item names in the images route.
function mentionsName(haystackLower: string, nameLower: string): boolean {
  // Floor at <2 so 2-letter character names still attribute.
  if (nameLower.length < 2) {
    return false;
  }
  if (/^[\x00-\x7f]+$/.test(nameLower)) {
    const escaped = nameLower.replace(/[.*+?^${}()|[\]\\]/g, "\\$&");
    return new RegExp(`\\b${escaped}\\b`).test(haystackLower);
  }
  return haystackLower.includes(nameLower);
}

// Best-effort: which saved character is most likely speaking in `context`
// (typically the sentence right before a quote). Returns null when no name is
// mentioned, so the caller defaults to the narrator voice.
export function detectSpeaker(
  context: string,
  characters: Array<Pick<StoryCharacter, "id" | "name" | "voice">>,
): { id: string; voice?: string } | null {
  if (!context.trim()) {
    return null;
  }
  const haystack = context.toLowerCase();
  for (const entry of charactersByName(characters)) {
    if (mentionsName(haystack, entry.nameLower)) {
      return { id: entry.id, voice: entry.voice };
    }
  }
  return null;
}

// Russian/Latin quotation styles the narrator uses for dialogue. Each pair is
// [open, close]; "—" leads a dialogue line to end-of-line.
const QUOTE_SPANS: Array<{ open: string; close: string }> = [
  { open: "«", close: "»" },
  { open: "“", close: "”" },
  { open: '"', close: '"' },
];

// Split a passage into alternating narration / dialogue runs, each tagged with
// the voice to read it. When multi-voice is off this returns a single
// narrator-voiced segment for the whole passage — identical to today's behavior.
// When on, quoted spans are attributed to the nearest preceding named character
// (if any) and voiced accordingly; everything else stays narrator-voiced.
export function splitDialogueSegments(
  passage: string,
  settings: Pick<StorySettings, "voice" | "multiVoice">,
  characters: Array<Pick<StoryCharacter, "id" | "name" | "voice">>,
  voicePool: readonly string[] = [],
): VoiceSegment[] {
  const text = passage.trim();
  if (!text) {
    return [];
  }

  const narratorVoice = settings.voice;
  // Fast path: single voice for the entire passage (the existing contract).
  if (!settings.multiVoice || characters.length === 0) {
    return [{ text, voice: narratorVoice, characterId: null, kind: "narration" }];
  }

  const segments: VoiceSegment[] = [];
  let cursor = 0;

  const pushNarration = (slice: string) => {
    const trimmed = slice.trim();
    if (trimmed) {
      segments.push({ text: trimmed, voice: narratorVoice, characterId: null, kind: "narration" });
    }
  };

  while (cursor < text.length) {
    // Find the next quote opener of any style from the cursor.
    let nextOpen = -1;
    let span: { open: string; close: string } | null = null;
    for (const candidate of QUOTE_SPANS) {
      const at = text.indexOf(candidate.open, cursor);
      if (at !== -1 && (nextOpen === -1 || at < nextOpen)) {
        nextOpen = at;
        span = candidate;
      }
    }

    if (nextOpen === -1 || !span) {
      pushNarration(text.slice(cursor));
      break;
    }

    const closeAt = text.indexOf(span.close, nextOpen + span.open.length);
    if (closeAt === -1) {
      // Unterminated quote — treat the remainder as narration and stop.
      pushNarration(text.slice(cursor));
      break;
    }

    // Narration before the quote (its tail names the speaker).
    const leading = text.slice(cursor, nextOpen);
    pushNarration(leading);

    const quoted = text.slice(nextOpen, closeAt + span.close.length);
    const speaker = detectSpeaker(leading, characters);
    // Explicit voice wins, else a distinct auto-assigned one, else narrator —
    // the shared ladder, with the full list so auto-assignment stays distinct.
    const voice = voiceForCharacter(settings, speaker, voicePool, characters);
    segments.push({
      text: quoted.trim(),
      voice,
      characterId: speaker?.id ?? null,
      kind: "quote",
    });

    cursor = closeAt + span.close.length;
  }

  // Fold a tiny NARRATION fragment (lone speaker tag / ":" / "." — under 14 chars
  // or punctuation-only) into the PREVIOUS segment, but only when that segment is
  // also narrator-voiced — so narration is never read in a character's voice and a
  // quote run is never swallowed. A tiny bit with no same-voice neighbor stays its
  // own short narrator clip (acceptable, and never mis-voiced).
  const isTinyNarration = (segment: VoiceSegment) =>
    segment.kind === "narration" &&
    (segment.text.length < 14 || /^[\s\p{P}\p{S}]+$/u.test(segment.text));
  const merged: VoiceSegment[] = [];
  for (const segment of segments) {
    const last = merged[merged.length - 1];
    if (isTinyNarration(segment) && last && last.voice === segment.voice) {
      last.text = `${last.text} ${segment.text}`.trim();
      continue;
    }
    merged.push(segment);
  }

  // Collapse to the single-voice contract if nothing actually got a distinct
  // voice — keeps callers that expect one segment on the happy path.
  if (merged.every((segment) => segment.voice === narratorVoice)) {
    return [{ text, voice: narratorVoice, characterId: null, kind: "narration" }];
  }

  return merged.length
    ? merged
    : [{ text, voice: narratorVoice, characterId: null, kind: "narration" }];
}
