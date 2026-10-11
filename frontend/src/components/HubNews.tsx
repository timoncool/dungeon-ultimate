import { useEffect, useState } from "react";
import { ChevronRight, Newspaper } from "lucide-react";
import { reportNotice, useHubState } from "@/lib/studioHub";
import { useUiLanguage } from "@/lib/ui-text-context";
import { panelText } from "@/lib/ui-text-panels";
import { inline } from "./HubBars";

// Новости хаба — как вкладка новостей Dub Studio: новая доходит до игры без релиза.

type News = { id: string; date: string; title: string; body: string; link?: { url: string; label: string; button: string } };

function NewsBody({ text }: { text: string }) {
  return (
    <div className="mt-1.5 space-y-2 text-xs leading-relaxed text-stone-400">
      {text.split(/\n\s*\n/).map((block, i) => {
        const lines = block.split("\n").filter((l) => l.trim());
        if (lines.length && lines.every((l) => l.trimStart().startsWith("- "))) {
          return (
            <ul key={i} className="list-disc space-y-1 pl-4">
              {lines.map((l, j) => <li key={j}>{inline(l.trimStart().slice(2), `${i}-${j}`)}</li>)}
            </ul>
          );
        }
        return <p key={i}>{inline(block, String(i))}</p>;
      })}
    </div>
  );
}

export default function HubNews() {
  const lang = useUiLanguage();
  const panel = panelText(lang);
  const hub = useHubState(lang);
  const [open, setOpen] = useState(false);
  const news: News[] = (hub?.items ?? [])
    .filter((item) => item.kind === "news")
    .map((item) => {
      const local = item.content[lang] ?? item.content.en ?? Object.values(item.content)[0];
      const index = local?.buttons.findIndex((b) => b.action === "url" && b.url) ?? -1;
      const button = index >= 0 ? local?.buttons[index] : undefined;
      return {
        id: item.id,
        date: item.date ?? "",
        title: local?.title ?? "",
        body: local?.body ?? "",
        link: button?.url ? { url: button.url, label: button.label, button: `b${index}` } : undefined,
      };
    })
    .sort((a, b) => b.date.localeCompare(a.date));
  const shownIds = open ? news.map((n) => n.id).join(",") : "";
  useEffect(() => {
    for (const id of shownIds ? shownIds.split(",") : []) reportNotice(id, "shown").catch((e: Error) => console.warn("[hub] news shown not recorded:", e.message));
  }, [shownIds]);

  return (
    <section className="space-y-2.5">
      <button
        type="button"
        aria-expanded={open}
        onClick={() => setOpen((value) => !value)}
        className="group flex w-full min-w-0 items-center gap-3 rounded border border-stone-800 bg-stone-950/70 p-2 text-left transition hover:border-amber-800/60 hover:bg-stone-900/70"
      >
        <span className="flex size-12 shrink-0 items-center justify-center overflow-hidden rounded-lg border border-amber-200/15 bg-stone-950 text-amber-200 shadow-[0_0_16px_rgba(251,191,36,0.1)]">
          <Newspaper className="size-6" aria-hidden="true" />
        </span>
        <span className="min-w-0 flex-1">
          <span className="block truncate text-sm font-medium text-stone-300">{panel.news}</span>
          <span className="block truncate text-xs text-stone-500">{news.length ? news[0].title : panel.newsEmpty}</span>
        </span>
        <ChevronRight
          className={`ml-auto size-4 shrink-0 text-stone-500 transition group-hover:text-amber-200/80 ${open ? "rotate-90 text-amber-200" : ""}`}
          aria-hidden="true"
        />
      </button>
      {open && (
        <div className="space-y-2">
          {news.length === 0 && <p className="text-xs text-stone-500">{panel.newsEmpty}</p>}
          {news.map((n) => (
            <article key={n.id} className="rounded border border-stone-800 bg-stone-950 px-3 py-2">
              <div className="flex items-baseline justify-between gap-2">
                <h3 className="text-sm text-stone-200">{n.title}</h3>
                <span className="shrink-0 text-[11px] tabular-nums text-stone-600">{n.date.slice(0, 10)}</span>
              </div>
              <NewsBody text={n.body} />
              {n.link && (
                <a
                  href={n.link.url}
                  target="_blank"
                  rel="noreferrer"
                  onClick={() => reportNotice(n.id, "clicked", n.link?.button).catch((e: Error) => console.warn("[hub] news click not recorded:", e.message))}
                  className="mt-2 inline-block text-xs text-amber-200 underline underline-offset-2 hover:text-amber-100"
                >
                  {n.link.label}
                </a>
              )}
            </article>
          ))}
        </div>
      )}
    </section>
  );
}
