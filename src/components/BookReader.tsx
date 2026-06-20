"use client";

import { ChevronLeft, ChevronRight, Loader2, Volume2 } from "lucide-react";
import { useEffect, useLayoutEffect, useMemo, useRef, useState } from "react";
import HTMLFlipBook from "react-pageflip";
import { cn } from "@/lib/cn";
import type { StoryMessage } from "@/lib/types";

type FlipApi = { pageFlip: () => { flipNext: () => void; flipPrev: () => void } };
type Block =
  | { kind: "image"; url: string; message: StoryMessage }
  | { kind: "text"; text: string; message: StoryMessage; drop: boolean };

const PAD = 28; // page inner padding (px)
const FOOTER = 44; // voice button + page-number row
const GAP = 14; // space below each block (px) — matches mb-3.5 in render
const FONT_PX = 17;
const LINE_PX = 28; // leading

function splitSentences(paragraph: string): string[] {
  return paragraph.match(/[^.!?…]+[.!?…]*\s*/g) ?? [paragraph];
}

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
  onSpeak?: (message: StoryMessage) => void;
  speakingId?: string;
}) {
  const wrapRef = useRef<HTMLDivElement>(null);
  const measureRef = useRef<HTMLDivElement>(null);
  const bookRef = useRef<FlipApi | null>(null);
  const passages = useMemo(
    () => messages.filter((message) => message.role === "assistant" && message.content.trim()),
    [messages],
  );
  const [dims, setDims] = useState({ pageW: 520, pageH: 760 });
  const [pages, setPages] = useState<Block[][]>([]);

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
      setDims((prev) => (prev.pageW === pageW && prev.pageH === pageH ? prev : { pageW, pageH }));
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
    m.style.width = `${dims.pageW - PAD * 2}px`;
    const contentH = dims.pageH - PAD * 2 - FOOTER;
    const imgH = Math.round(dims.pageH * 0.33);
    const textHeight = (s: string) => {
      m.textContent = s;
      return m.scrollHeight;
    };

    // Pack the story so every page fills: paragraphs are split at sentence
    // boundaries when the rest won't fit the remaining space (real-book flow).
    const out: Block[][] = [];
    let page: Block[] = [];
    let h = 0;
    const avail = () => contentH - h;
    const flushPage = () => {
      if (page.length) {
        out.push(page);
        page = [];
        h = 0;
      }
    };
    for (const message of passages) {
      if (message.generatedImage?.url) {
        if (page.length && imgH + GAP > avail()) flushPage();
        page.push({ kind: "image", url: message.generatedImage.url, message });
        h += imgH + GAP;
      }
      const paras = message.content
        .split(/\n\s*\n/)
        .map((p) => p.trim())
        .filter(Boolean);
      paras.forEach((para, index) => {
        let buf = "";
        let drop = index === 0;
        const flushBuf = () => {
          if (!buf.trim()) return;
          page.push({ kind: "text", text: buf.trim(), message, drop });
          h += textHeight(buf) + GAP;
          buf = "";
          drop = false;
        };
        for (const sentence of splitSentences(para)) {
          if (buf && textHeight(buf + sentence) + GAP > avail()) {
            flushBuf();
            flushPage();
          }
          buf += sentence;
        }
        if (buf.trim()) {
          if (page.length && textHeight(buf) + GAP > avail()) flushPage();
          flushBuf();
        }
      });
    }
    flushPage();
    setPages(out);
  }, [passages, dims]);

  const measurer = (
    <div
      ref={measureRef}
      aria-hidden
      className="pointer-events-none invisible fixed left-[-9999px] top-0 whitespace-pre-wrap font-serif"
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
                    {page.map((block, bi) =>
                      block.kind === "image" ? (
                        <div
                          key={bi}
                          className="mb-3.5 overflow-hidden rounded-sm border-2 border-[#8a6a3a]/55 shadow-md"
                          style={{ height: Math.round(dims.pageH * 0.33) }}
                        >
                          {/* eslint-disable-next-line @next/next/no-img-element */}
                          <img src={block.url} alt="" className="h-full w-full object-cover" />
                        </div>
                      ) : (
                        <p
                          key={bi}
                          className={cn(
                            "mb-3.5 whitespace-pre-wrap font-serif",
                            block.drop &&
                              "[&::first-letter]:float-left [&::first-letter]:mr-2 [&::first-letter]:font-bold [&::first-letter]:text-5xl [&::first-letter]:leading-[0.85] [&::first-letter]:text-[#7a3b18]",
                          )}
                          style={{ fontSize: FONT_PX, lineHeight: `${LINE_PX}px` }}
                        >
                          {block.text}
                        </p>
                      ),
                    )}
                  </div>
                  <div className="mt-2 flex shrink-0 items-center justify-between border-t border-[#9c7b46]/30 pt-2">
                    {onSpeak && page[0] ? (
                      <button
                        type="button"
                        onClick={() => onSpeak(page[0].message)}
                        className="inline-flex items-center gap-1.5 rounded-full border border-[#9c7b46]/50 px-2.5 py-1 text-[11px] font-medium text-[#5a4326] transition hover:bg-[#9c7b46]/15"
                      >
                        {speakingId === page[0].message.id ? (
                          <Loader2 className="size-3.5 animate-spin" aria-hidden="true" />
                        ) : (
                          <Volume2 className="size-3.5" aria-hidden="true" />
                        )}
                        Озвучить
                      </button>
                    ) : (
                      <span />
                    )}
                    <span className="font-serif text-xs italic text-[#8a6a3a]">— {index + 1} —</span>
                  </div>
                </div>
              </div>
            </div>
          ))}
        </HTMLFlipBook>
      )}
      <div className="flex items-center gap-6">
        <button
          type="button"
          onClick={() => bookRef.current?.pageFlip().flipPrev()}
          className="inline-flex size-10 items-center justify-center rounded-full border border-stone-700 text-stone-300 transition hover:border-amber-300 hover:text-amber-200"
          aria-label="Предыдущая страница"
        >
          <ChevronLeft className="size-5" aria-hidden="true" />
        </button>
        <span className="font-serif text-xs italic text-stone-500">листай страницы · {pages.length}</span>
        <button
          type="button"
          onClick={() => bookRef.current?.pageFlip().flipNext()}
          className="inline-flex size-10 items-center justify-center rounded-full border border-stone-700 text-stone-300 transition hover:border-amber-300 hover:text-amber-200"
          aria-label="Следующая страница"
        >
          <ChevronRight className="size-5" aria-hidden="true" />
        </button>
      </div>
    </div>
  );
}
