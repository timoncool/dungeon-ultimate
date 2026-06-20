"use client";

import { ChevronLeft, ChevronRight, Loader2, Volume2 } from "lucide-react";
import { useRef } from "react";
import HTMLFlipBook from "react-pageflip";
import type { StoryMessage } from "@/lib/types";

type FlipApi = { pageFlip: () => { flipNext: () => void; flipPrev: () => void } };

// A storybook reader: each narrated passage becomes a parchment page (its scene as
// a framed illustration, the prose below with a drop-cap initial), shown as a real
// two-page spread that you flip with a 3D animation — like reading a fairy tale.
// A per-page "Озвучить" button reads the passage aloud. Loaded client-only
// (dynamic ssr:false) since react-pageflip touches the DOM on mount.
export default function BookReader({
  messages,
  onSpeak,
  speakingId,
}: {
  messages: StoryMessage[];
  onSpeak?: (message: StoryMessage) => void;
  speakingId?: string;
}) {
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
    <div className="flex h-full min-h-0 w-full flex-col items-center justify-center gap-4 py-2">
      {/* @ts-expect-error react-pageflip ships loose JS prop types */}
      <HTMLFlipBook
        ref={bookRef}
        width={480}
        height={720}
        size="stretch"
        minWidth={300}
        maxWidth={560}
        minHeight={480}
        maxHeight={940}
        drawShadow
        maxShadowOpacity={0.6}
        showCover={false}
        mobileScrollSupport
        useMouseEvents
        className="rpg-book"
      >
        {pages.map((message, index) => (
          <div key={message.id} className="h-full">
            <div
              className="flex h-full flex-col overflow-hidden"
              style={{
                background:
                  "radial-gradient(130% 90% at 50% 0%, #f5ead0 0%, #e9d8b4 65%, #ddc99e 100%)",
                boxShadow: "inset 0 0 70px rgba(120,80,30,0.28)",
                color: "#3a2a18",
              }}
            >
              <div className="m-3 flex min-h-0 flex-1 flex-col rounded-sm border border-[#9c7b46]/45 p-5">
                {message.generatedImage?.url && (
                  <div className="mb-4 shrink-0 overflow-hidden rounded-sm border-2 border-[#8a6a3a]/55 shadow-md">
                    {/* eslint-disable-next-line @next/next/no-img-element */}
                    <img
                      src={message.generatedImage.url}
                      alt=""
                      className="h-44 w-full object-cover sm:h-56"
                    />
                  </div>
                )}
                <div className="min-h-0 flex-1 overflow-y-auto whitespace-pre-wrap font-serif text-[16px] leading-7 [&::first-letter]:float-left [&::first-letter]:mr-2 [&::first-letter]:font-bold [&::first-letter]:font-serif [&::first-letter]:text-5xl [&::first-letter]:leading-[0.85] [&::first-letter]:text-[#7a3b18]">
                  {message.content}
                </div>
                <div className="mt-4 flex shrink-0 items-center justify-between border-t border-[#9c7b46]/30 pt-2">
                  {onSpeak ? (
                    <button
                      type="button"
                      onClick={() => onSpeak(message)}
                      className="inline-flex items-center gap-1.5 rounded-full border border-[#9c7b46]/50 px-2.5 py-1 text-[11px] font-medium text-[#5a4326] transition hover:bg-[#9c7b46]/15"
                    >
                      {speakingId === message.id ? (
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
      <div className="flex items-center gap-6">
        <button
          type="button"
          onClick={() => bookRef.current?.pageFlip().flipPrev()}
          className="inline-flex size-10 items-center justify-center rounded-full border border-stone-700 text-stone-300 transition hover:border-amber-300 hover:text-amber-200"
          aria-label="Предыдущая страница"
        >
          <ChevronLeft className="size-5" aria-hidden="true" />
        </button>
        <span className="font-serif text-xs italic text-stone-500">листай страницы</span>
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
