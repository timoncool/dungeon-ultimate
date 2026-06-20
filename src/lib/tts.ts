import type { StoryCharacter, StorySettings } from "@/lib/types";

// ---------------------------------------------------------------------------
// Multi-voice TTS hook (design seam).
//
// The narrator writes one Russian passage per turn; today it is read aloud in a
// single voice (StorySettings.voice). This module lets a caller map dialogue to
// per-character voices WITHOUT breaking that single-voice path:
//
//   • voiceForCharacter() — the voice id to read a known speaker's lines in,
//     falling back to the narrator's single voice when the character has none
//     or multi-voice is off.
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
// ---------------------------------------------------------------------------

export type VoiceSegment = {
  text: string;
  voice: string;
  // The character this run is attributed to, when known (null for narration).
  characterId: string | null;
};

// The voice a specific character should be read in. Honors multi-voice only when
// enabled and the character actually has a voice set; otherwise the single
// narrator voice. Safe to call unconditionally.
export function voiceForCharacter(
  settings: Pick<StorySettings, "voice" | "multiVoice">,
  character: Pick<StoryCharacter, "voice"> | null | undefined,
): string {
  if (settings.multiVoice && character?.voice && character.voice.trim()) {
    return character.voice.trim();
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
  if (nameLower.length < 2) {
    return false;
  }
  if (/^[\x00-\x7f]+$/.test(nameLower)) {
    const escaped = nameLower.replace(/[.*+?^${}()|[\]\\]/g, "\\$&");
    return new RegExp(`\\b${escaped}\\b`).test(haystackLower);
  }
  return nameLower.length >= 3 && haystackLower.includes(nameLower);
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
): VoiceSegment[] {
  const text = passage.trim();
  if (!text) {
    return [];
  }

  const narratorVoice = settings.voice;
  // Fast path: single voice for the entire passage (the existing contract).
  if (!settings.multiVoice || characters.length === 0) {
    return [{ text, voice: narratorVoice, characterId: null }];
  }

  const segments: VoiceSegment[] = [];
  let cursor = 0;

  const pushNarration = (slice: string) => {
    const trimmed = slice.trim();
    if (trimmed) {
      segments.push({ text: trimmed, voice: narratorVoice, characterId: null });
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
    const voice =
      speaker?.voice && speaker.voice.trim() ? speaker.voice.trim() : narratorVoice;
    segments.push({
      text: quoted.trim(),
      voice,
      characterId: speaker?.id ?? null,
    });

    cursor = closeAt + span.close.length;
  }

  // Collapse to the single-voice contract if nothing actually got a distinct
  // voice — keeps callers that expect one segment on the happy path.
  if (segments.every((segment) => segment.voice === narratorVoice)) {
    return [{ text, voice: narratorVoice, characterId: null }];
  }

  return segments.length ? segments : [{ text, voice: narratorVoice, characterId: null }];
}
