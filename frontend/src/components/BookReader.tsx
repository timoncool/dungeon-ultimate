"use client";

import { ChevronLeft, ChevronRight, Loader2, Volume2 } from "lucide-react";
import { useEffect, useLayoutEffect, useMemo, useRef, useState } from "react";
import HTMLFlipBook from "react-pageflip";
import { cn } from "@/lib/cn";
import { splitSentences } from "@/lib/text";
import type { CheckResult } from "@/lib/rpg/dice";
import type { GameEvent, Item } from "@/lib/rpg/types";
import type { StoryMessage } from "@/lib/types";

type FlipApi = {
  pageFlip: () => {
    flipNext: () => void;
    flipPrev: () => void;
    turnToPage: (page: number) => void;
  };
};
type Block =
  | { kind: "image"; url: string; message: StoryMessage }
  | { kind: "text"; text: string; message: StoryMessage; drop: boolean }
  | { kind: "action"; text: string; message: StoryMessage }
  | { kind: "event"; event: GameEvent; message: StoryMessage };

// A resolved game event as a parchment card INSIDE the book — a rolled die / check,
// a loot drop with its portrait, a foe, damage/heal, a buff. Part of the story,
// styled for the page (not the dark feed card).
function BookEventCard({ event }: { event: GameEvent }) {
  if (event.kind === "item") {
    const item = (event.data as { item?: Item } | undefined)?.item;
    return (
      <div className="mb-3.5 flex items-center gap-3 rounded border-2 border-[#8a6a3a]/55 bg-[#efe0c0] px-3 py-2 shadow-sm">
        {item?.imageUrl ? (
          // eslint-disable-next-line @next/next/no-img-element
          <img src={item.imageUrl} alt="" className="size-11 shrink-0 rounded object-cover" />
        ) : (
          <span className="grid size-11 shrink-0 place-items-center rounded bg-[#dcc99e] text-xl" aria-hidden="true">
            📦
          </span>
        )}
        <div className="min-w-0">
          <div className="truncate font-serif font-bold text-[#3a2a18]">{item?.name ?? "Предмет"}</div>
          <div className="truncate font-serif text-xs text-[#6a4f2c]">
            {event.text.replace(/^📦\s*Получен предмет:\s*/, "")}
          </div>
        </div>
      </div>
    );
  }
  if (event.kind === "roll") {
    const result = (event.data as { result?: CheckResult } | undefined)?.result;
    const tone =
      result?.crit === "success"
        ? "border-amber-600 text-amber-800"
        : result?.crit === "fail"
          ? "border-red-700 text-red-800"
          : result?.success
            ? "border-emerald-700 text-emerald-800"
            : "border-stone-600 text-stone-700";
    return (
      <div className="mb-3.5 flex items-center gap-2.5 rounded border border-[#9c7b46]/50 bg-[#ece0c4]/80 px-3 py-2">
        {result ? (
          <span
            className={cn(
              "inline-flex size-7 shrink-0 items-center justify-center rounded-md border-2 bg-[#f5ead0] text-sm font-bold tabular-nums",
              tone,
            )}
          >
            {result.d20}
          </span>
        ) : (
          <span aria-hidden="true">🎲</span>
        )}
        <span className="font-serif text-sm text-[#3a2a18]">{event.text.replace(/^🎲\s*/, "")}</span>
      </div>
    );
  }
  const heal = event.kind === "hp" && /[💚✨]/u.test(event.text);
  const tone =
    event.kind === "combat"
      ? "border-red-800/40 bg-red-200/40"
      : event.kind === "death"
        ? "border-stone-700/50 bg-stone-300/50"
        : event.kind === "hp"
          ? heal
            ? "border-emerald-800/40 bg-emerald-200/40"
            : "border-rose-800/40 bg-rose-200/40"
          : event.kind === "effect"
            ? "border-amber-800/40 bg-amber-200/40"
            : "border-[#9c7b46]/40 bg-[#9c7b46]/10";
  return (
    <div
      className={cn(
        "mb-3.5 rounded border px-3 py-1.5 font-serif text-sm text-[#3a2a18]",
        tone,
        event.kind === "note" && "italic",
      )}
    >
      {event.text}
    </div>
  );
}

const PAD = 28; // page inner padding (px)
const FOOTER = 44; // voice button + page-number row
const GAP = 14; // space below each block (px) — matches mb-3.5 in render
const FONT_PX = 17;
const LINE_PX = 28; // leading
// Horizontal/vertical chrome around the text column: card margin (9*2) + padding
// (PAD-9)*2 + border (1*2). margin+padding collapse to PAD*2, leaving the +2 border.
const CHROME = PAD * 2 + 2;
// Drop-cap styling shared by the rendered <p> and the measurer, so measured height
// matches what is painted (the floated text-5xl capital is taller than one line).
const DROP_CAP =
  "[&::first-letter]:float-left [&::first-letter]:mr-1 [&::first-letter]:mt-0.5 [&::first-letter]:font-bold [&::first-letter]:text-5xl [&::first-letter]:leading-[0.8] [&::first-letter]:text-[#7a3b18]";

// A storybook reader: the whole story FLOWS across parchment pages, packed so each
// page fills with text (no scrollbars — like a real book). Scene images appear
// inline as framed illustrations, a passage's first paragraph gets a drop-cap, and
// every page has an "Озвучить" button. Client-only (react-pageflip needs the DOM).
export default function BookReader({
  messages,
  onSpeak,
  speakingId,
}: {
  messages: StoryMessage[];
  // Voice an arbitrary chunk of prose (a whole open spread) under a stable id, so the
  // one "Озвучить" reads BOTH open pages, not a single message.
  onSpeak?: (id: string, text: string) => void;
  speakingId?: string;
}) {
  const wrapRef = useRef<HTMLDivElement>(null);
  const measureRef = useRef<HTMLDivElement>(null);
  const bookRef = useRef<FlipApi | null>(null);
  const passages = useMemo(
    () => messages.filter((message) => message.role === "assistant" && message.content.trim()),
    [messages],
  );
  // The full ordered stream the book paginates: the narrator's passages, the
  // player's own actions, and each turn's resolved game events — one interleaved
  // adventure, not just the narration.
  const stream = useMemo(
    () => messages.filter((message) => message.content.trim() || message.events?.length),
    [messages],
  );
  const [dims, setDims] = useState({ pageW: 520, pageH: 760, twoUp: true });
  const [pages, setPages] = useState<Block[][]>([]);
  // Which page is open (left page of the current spread), tracked from react-pageflip
  // so the single "Озвучить" knows which spread to read.
  const [currentPage, setCurrentPage] = useState(0);

  // Page size from the container: a two-page spread on wide screens, one on narrow.
  useEffect(() => {
    const el = wrapRef.current;
    if (!el) return;
    const recompute = () => {
      const W = el.clientWidth;
      const H = el.clientHeight - 58;
      const twoUp = W >= 720;
      const pageW = Math.min(twoUp ? Math.floor(W / 2) - 10 : W - 12, 560);
      const pageH = Math.min(Math.max(H, 460), 940);
      setDims((prev) =>
        prev.pageW === pageW && prev.pageH === pageH && prev.twoUp === twoUp
          ? prev
          : { pageW, pageH, twoUp },
      );
    };
    recompute();
    const ro = new ResizeObserver(recompute);
    ro.observe(el);
    return () => ro.disconnect();
  }, []);

  // Flatten the story into blocks, then pack them so every page fills.
  useLayoutEffect(() => {
    const m = measureRef.current;
    if (!m || !passages.length) {
      setPages([]);
      return;
    }
    m.style.width = `${dims.pageW - CHROME}px`;
    // -8 keeps a small safety margin against sub-pixel / font-metric rounding.
    const contentH = dims.pageH - CHROME - FOOTER - 8;
    const imgH = Math.round(dims.pageH * 0.33);
    const dropClasses = DROP_CAP.split(" ");
    const textHeight = (s: string, drop = false) => {
      m.textContent = s;
      // Measure drop-cap blocks WITH the floated capital so their height isn't
      // underestimated (the float overhangs and narrows the first lines).
      dropClasses.forEach((c) => m.classList.toggle(c, drop));
      return m.scrollHeight;
    };

    // Pour the story across pages like a real book: each paragraph is split at
    // sentence boundaries (then by words, if a lone sentence is taller than a whole
    // page) and greedily filled into the space left on the page. The invariant is
    // that we NEVER push a block taller than what remains — so nothing is ever
    // clipped at the page bottom and every page fills as far as whole sentences go.
    const out: Block[][] = [];
    let page: Block[] = [];
    let h = 0;
    let curMsg = passages[0];
    const avail = () => contentH - h;
    const flushPage = () => {
      if (page.length) {
        out.push(page);
        page = [];
        h = 0;
      }
    };
    const pushText = (text: string, drop: boolean) => {
      // Measure the SAME (trimmed) string we render, so packing height == painted height.
      const trimmed = text.trim();
      page.push({ kind: "text", text: trimmed, message: curMsg, drop });
      h += textHeight(trimmed, drop) + GAP;
    };
    // Place whole sentences greedily; when the next sentence won't fit the space
    // left on the page, pour in as many of its leading WORDS as fit and carry the
    // rest to the next page — so a sentence flows across the page break and every
    // page fills down to the last line, exactly like a real book. The invariant is
    // that we never push a block taller than the remaining height (no clipping).
    const splitWords = (s: string) => s.match(/\S+\s*/g) ?? [s];
    // Largest count of leading `words` (prefixed by `prefix`) that still fits the space
    // left. Height grows monotonically with word count, so binary-search it instead of
    // re-measuring an ever-growing prefix word-by-word (O(log W) reflows, not O(W)).
    const fitWordCount = (prefix: string, words: string[], drop: boolean) => {
      let lo = 0;
      let hi = words.length;
      while (lo < hi) {
        const mid = (lo + hi + 1) >> 1;
        if (textHeight(prefix + words.slice(0, mid).join(""), drop) + GAP <= avail()) lo = mid;
        else hi = mid - 1;
      }
      return lo;
    };
    const fill = (sentences: string[], dropFirst: boolean) => {
      const queue = [...sentences];
      let buf = "";
      let drop = dropFirst;
      while (queue.length) {
        const sentence = queue.shift() as string;
        if (!buf && textHeight(sentence, drop) + GAP > avail()) {
          // Doesn't fit in what's left (incl. a sentence taller than a whole empty page):
          // fill the leftover lines with leading words, break, and continue on the next.
          const words = splitWords(sentence);
          let i = fitWordCount("", words, drop);
          // On a fresh page nothing else can shrink it, so always take at least one word
          // (a lone over-tall token) to guarantee progress and avoid an infinite re-queue.
          if (i === 0 && !page.length) i = 1;
          if (i > 0) {
            pushText(words.slice(0, i).join(""), drop);
            drop = false;
          }
          flushPage();
          const rest = words.slice(i).join("");
          if (rest) queue.unshift(rest);
          continue;
        }
        if (!buf) {
          buf = sentence;
        } else if (textHeight(buf + sentence, drop) + GAP <= avail()) {
          buf += sentence;
        } else {
          // `buf` fits — top up the leftover with leading words of `sentence`, break,
          // and carry the remaining words forward.
          const words = splitWords(sentence);
          const i = fitWordCount(buf, words, drop);
          pushText(buf + words.slice(0, i).join(""), drop);
          drop = false;
          flushPage();
          buf = "";
          const rest = words.slice(i).join("");
          if (rest) queue.unshift(rest);
        }
      }
      if (buf) {
        if (page.length && textHeight(buf, drop) + GAP > avail()) flushPage();
        pushText(buf, drop);
      }
    };

    // Atomic-block heights (player action / event card): measured at the block's
    // inner width, generously padded so a card is NEVER clipped — over-reserving
    // only leaves a little whitespace, while the fit-or-flush guarantees no overflow.
    const actionHeight = (text: string) => textHeight(`❯ ${text}`) + 16;
    const eventHeight = (event: GameEvent) => {
      const prevW = m.style.width;
      m.style.width = `${dims.pageW - CHROME - 26}px`;
      const measured = textHeight(event.text);
      m.style.width = prevW;
      return Math.max(measured, event.kind === "item" ? 52 : 0) + 24;
    };
    const pushAtomic = (block: Block, height: number) => {
      if (page.length && height + GAP > avail()) flushPage();
      page.push(block);
      h += height + GAP;
    };

    for (const message of stream) {
      curMsg = message;
      if (message.role === "user") {
        // The player's own action, woven into the story as a styled block.
        const text = message.content.replace(/^>\s*/, "").trim();
        if (text) pushAtomic({ kind: "action", text, message }, actionHeight(text));
        continue;
      }
      if (message.generatedImage?.url) {
        pushAtomic({ kind: "image", url: message.generatedImage.url, message }, imgH);
      }
      const paras = message.content
        .split(/\n\s*\n/)
        .map((p) => p.trim())
        .filter(Boolean);
      paras.forEach((para, index) => fill(splitSentences(para), index === 0));
      // This turn's resolved events as cards, right after the passage they belong to.
      for (const event of message.events ?? []) {
        pushAtomic({ kind: "event", event, message }, eventHeight(event));
      }
    }
    flushPage();
    setPages(out);
  }, [stream, dims]);

  // New narration extends the book and react-pageflip remounts (its key includes
  // pages.length + the page size), which otherwise snaps it back to page 1. Re-assert
  // the position after the remount so the player lands on the NEWEST page instead of
  // having to flip all the way back every turn.
  const lastPage = pages.length - 1;
  useEffect(() => {
    if (lastPage < 0) return;
    const id = window.setTimeout(() => {
      try {
        bookRef.current?.pageFlip().turnToPage(lastPage);
        setCurrentPage(lastPage);
      } catch {
        // flipbook not mounted yet — harmless
      }
    }, 60);
    return () => window.clearTimeout(id);
  }, [lastPage, dims.pageW, dims.pageH]);

  const measurer = (
    <div
      ref={measureRef}
      aria-hidden
      lang="ru"
      className="pointer-events-none invisible fixed left-[-9999px] top-0 whitespace-pre-wrap font-serif hyphens-auto text-justify"
      style={{ fontSize: FONT_PX, lineHeight: `${LINE_PX}px` }}
    />
  );

  if (!passages.length) {
    return (
      <div ref={wrapRef} className="flex h-full items-center justify-center px-6 text-center font-serif text-stone-500">
        {measurer}
        Книга пока пуста — начни историю.
      </div>
    );
  }

  // The open spread = the left page (normalised to even in two-up mode) plus its
  // facing page; the single "Озвучить" reads the prose of BOTH pages, in order.
  const spreadLeft = dims.twoUp ? currentPage - (currentPage % 2) : currentPage;
  const blockProse = (block: Block): string =>
    block.kind === "text" || block.kind === "action" ? block.text : "";
  const pageProse = (p?: Block[]) => (p ?? []).map(blockProse).filter(Boolean).join("\n\n");
  const spreadText = [
    pageProse(pages[spreadLeft]),
    dims.twoUp ? pageProse(pages[spreadLeft + 1]) : "",
  ]
    .filter(Boolean)
    .join("\n\n");
  const spreadId = `spread:${spreadLeft}`;

  return (
    <div ref={wrapRef} className="flex h-full min-h-0 w-full flex-col items-center justify-center gap-3 py-1">
      {measurer}
      {pages.length > 0 && (
        // @ts-expect-error react-pageflip ships loose JS prop types
        <HTMLFlipBook
          key={`${dims.pageW}x${dims.pageH}x${pages.length}`}
          ref={bookRef}
          width={dims.pageW}
          height={dims.pageH}
          size="fixed"
          minWidth={300}
          maxWidth={560}
          minHeight={460}
          maxHeight={940}
          drawShadow
          maxShadowOpacity={0.6}
          showCover={false}
          mobileScrollSupport
          useMouseEvents
          onFlip={(e: { data?: number }) =>
            setCurrentPage(typeof e?.data === "number" ? e.data : 0)
          }
          className="rpg-book"
        >
          {pages.map((page, index) => (
            <div key={index} className="h-full">
              <div
                className="flex h-full flex-col overflow-hidden"
                style={{
                  background:
                    "radial-gradient(130% 90% at 50% 0%, #f5ead0 0%, #e9d8b4 65%, #ddc99e 100%)",
                  boxShadow: "inset 0 0 70px rgba(120,80,30,0.28)",
                  color: "#3a2a18",
                }}
              >
                <div
                  className="flex min-h-0 flex-1 flex-col overflow-hidden rounded-sm border border-[#9c7b46]/45"
                  style={{ margin: 9, padding: PAD - 9 }}
                >
                  <div className="min-h-0 flex-1 overflow-hidden">
                    {page.map((block, bi) => {
                      if (block.kind === "image") {
                        return (
                          <div
                            key={bi}
                            className="mb-3.5 overflow-hidden rounded-sm border-2 border-[#8a6a3a]/55 shadow-md"
                            style={{ height: Math.round(dims.pageH * 0.33) }}
                          >
                            {/* eslint-disable-next-line @next/next/no-img-element */}
                            <img src={block.url} alt="" className="h-full w-full object-cover" />
                          </div>
                        );
                      }
                      if (block.kind === "action") {
                        return (
                          <div
                            key={bi}
                            className="mb-3.5 flex items-start gap-2 rounded border border-[#9c7b46]/45 bg-[#9c7b46]/12 px-3 py-1.5 font-serif text-[15px] italic text-[#5a4326]"
                          >
                            <span className="not-italic text-[#8a6a3a]" aria-hidden="true">
                              ❯
                            </span>
                            <span>{block.text}</span>
                          </div>
                        );
                      }
                      if (block.kind === "event") {
                        return <BookEventCard key={bi} event={block.event} />;
                      }
                      return (
                        <p
                          key={bi}
                          lang="ru"
                          className={cn(
                            "mb-3.5 whitespace-pre-wrap font-serif hyphens-auto text-justify",
                            block.drop && DROP_CAP,
                          )}
                          style={{ fontSize: FONT_PX, lineHeight: `${LINE_PX}px` }}
                        >
                          {block.text}
                        </p>
                      );
                    })}
                  </div>
                  <div className="mt-2 flex shrink-0 items-center justify-center border-t border-[#9c7b46]/30 pt-2">
                    <span className="font-serif text-xs italic text-[#8a6a3a]">— {index + 1} —</span>
                  </div>
                </div>
              </div>
            </div>
          ))}
        </HTMLFlipBook>
      )}
      <div className="flex items-center gap-3">
        <button
          type="button"
          onClick={() => bookRef.current?.pageFlip().flipPrev()}
          className="inline-flex size-9 items-center justify-center rounded-full border border-stone-700 text-stone-300 transition hover:border-amber-300 hover:text-amber-200"
          aria-label="Предыдущая страница"
        >
          <ChevronLeft className="size-5" aria-hidden="true" />
        </button>
        {onSpeak && spreadText ? (
          <button
            type="button"
            onClick={() => onSpeak(spreadId, spreadText)}
            className="inline-flex items-center gap-1.5 rounded-full border border-stone-700 px-3 py-1.5 text-xs font-medium text-stone-300 transition hover:border-amber-300 hover:text-amber-200"
            title="Озвучить открытый разворот"
          >
            {speakingId === spreadId ? (
              <Loader2 className="size-4 animate-spin" aria-hidden="true" />
            ) : (
              <Volume2 className="size-4" aria-hidden="true" />
            )}
            Озвучить
          </button>
        ) : (
          <span className="font-serif text-xs italic text-stone-500">
            листай · {pages.length}
          </span>
        )}
        <button
          type="button"
          onClick={() => bookRef.current?.pageFlip().flipNext()}
          className="inline-flex size-9 items-center justify-center rounded-full border border-stone-700 text-stone-300 transition hover:border-amber-300 hover:text-amber-200"
          aria-label="Следующая страница"
        >
          <ChevronRight className="size-5" aria-hidden="true" />
        </button>
      </div>
    </div>
  );
}
