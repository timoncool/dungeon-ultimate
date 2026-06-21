// Split prose into sentences, keeping each sentence's terminal punctuation and
// trailing whitespace, so concatenating the parts reconstructs the original text
// (the book reader relies on that spacing to measure line widths). A trailing run
// without terminal punctuation is returned as its own sentence.
//
// This is the shared primitive: the book reader uses it as-is; the TTS path wraps
// it with whitespace-collapsing + short-fragment merging (see splitForSpeech).
export function splitSentences(text: string): string[] {
  // Terminal punctuation AND trailing closers (»”")) share one class so a run
  // like `!»…` stays whole — keeping the closer with its sentence without ever
  // dropping a terminal that follows it (concat must reconstruct the original).
  return text.match(/[^.!?…]+[.!?…"”»)]*\s*/g) ?? (text ? [text] : []);
}
