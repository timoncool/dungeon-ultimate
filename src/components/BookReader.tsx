"use client";

import { ChevronLeft, ChevronRight } from "lucide-react";
import { useRef } from "react";
import HTMLFlipBook from "react-pageflip";
import type { StoryMessage } from "@/lib/types";

type FlipApi = { pageFlip: () => { flipNext: () => void; flipPrev: () => void } };

// A real page-flip book: each narrated passage becomes a page (its generated
// scene on top, the prose below), with left/right flip arrows + a 3D flip. Loaded
// client-only (dynamic ssr:false) since react-pageflip touches the DOM on mount.
export default function BookReader({ messages }: { messages: StoryMessage[] }) {
  const bookRef = useRef<FlipApi | null>(null);
  const pages = messages.filter((message) => message.role === "assistant" && message.content.trim());

  if (!pages.length) {
    return (
      <div className="flex h-full items-center justify-center px-6 text-center font-serif text-stone-500">
        Книга пока пуста — начни историю.
      </div>
    );
  }

  return (
    <div className="flex h-full min-h-0 flex-col items-center justify-center gap-3 py-2">
      {/* @ts-expect-error react-pageflip ships loose JS prop types */}
      <HTMLFlipBook
        ref={bookRef}
        width={440}
        height={600}
        size="stretch"
        minWidth={300}
        maxWidth={620}
        minHeight={420}
        maxHeight={760}
        drawShadow
        maxShadowOpacity={0.5}
        showCover={false}
        mobileScrollSupport
        useMouseEvents
        className="rpg-book"
      >
        {pages.map((message, index) => (
          <div
            key={message.id}
            className="flex h-full flex-col overflow-hidden rounded-r-md border border-amber-900/40 bg-[#1c140d] p-5 shadow-inner"
          >
            {message.generatedImage?.url && (
              // eslint-disable-next-line @next/next/no-img-element
              <img
                src={message.generatedImage.url}
                alt=""
                className="mb-3 max-h-[44%] w-full shrink-0 rounded object-cover"
              />
            )}
            <div className="min-h-0 flex-1 overflow-y-auto whitespace-pre-wrap font-serif text-[15px] leading-relaxed text-stone-200">
              {message.content}
            </div>
            <div className="mt-2 shrink-0 text-right text-[10px] tabular-nums text-stone-600">
              {index + 1} / {pages.length}
            </div>
          </div>
        ))}
      </HTMLFlipBook>
      <div className="flex items-center gap-6">
        <button
          type="button"
          onClick={() => bookRef.current?.pageFlip().flipPrev()}
          className="inline-flex size-9 items-center justify-center rounded-full border border-stone-700 text-stone-300 transition hover:border-amber-300 hover:text-amber-200"
          aria-label="Предыдущая страница"
        >
          <ChevronLeft className="size-5" aria-hidden="true" />
        </button>
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
