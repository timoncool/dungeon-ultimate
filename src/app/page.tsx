"use client";

import * as AlertDialog from "@radix-ui/react-alert-dialog";
import * as Dialog from "@radix-ui/react-dialog";
import {
  Aperture,
  Backpack,
  BookOpen,
  Check,
  ChevronRight,
  Cpu,
  Dices,
  Eraser,
  FolderOpen,
  Heart,
  ImagePlus,
  Library,
  Loader2,
  Mic,
  Paperclip,
  Pencil,
  Play,
  Plus,
  RotateCcw,
  Send,
  Settings2,
  Shield,
  ShieldHalf,
  Sparkles,
  Swords,
  Trash2,
  Type,
  UserRound,
  Volume2,
  WandSparkles,
  X,
} from "lucide-react";
import {
  type Dispatch,
  type FormEvent,
  type ReactNode,
  type SetStateAction,
  useCallback,
  useEffect,
  useMemo,
  useRef,
  useState,
} from "react";
import dynamic from "next/dynamic";
import { cn } from "@/lib/cn";
import { DEFAULT_STORY_SETTINGS, titleFromInput } from "@/lib/defaults";
import { LOCAL_TEXT_MODELS, type LocalTextModelId, type TextProvider } from "@/lib/text-models";
import type {
  AspectPreset,
  Attachment,
  GeneratedImage,
  ImageBackend,
  ImageMode,
  ProseSize,
  ResponseLength,
  StoryChat,
  StoryCharacter,
  StoryChatSummary,
  StoryMessage,
  StorySettings,
} from "@/lib/types";
import { ABILITIES, ABILITY_LABELS_RU, abilityMod } from "@/lib/rpg/dice";
import type { CharacterRpg, GameEvent, Item } from "@/lib/rpg/types";
import type DiceBox from "@3d-dice/dice-box-threejs";

// Page-flip reader (react-pageflip touches the DOM) — client-only.
const BookReader = dynamic(() => import("@/components/BookReader"), { ssr: false });

const SELECTED_CHAT_KEY = "local-roleplay:selected-chat";
const MAX_IMAGE_REFERENCES = 2;
const STORY_REQUEST_TIMEOUT_MS = 7 * 60 * 1000;

const KICKOFF_DIRECTIVE =
  "Начни историю прямо сейчас. Напиши вводный отрывок: установи сцену, персонажа игрока и немедленную ситуацию от второго лица, завершив на моменте, который приглашает первое действие игрока. Не задавай игроку вопросы по настройке; история уже началась.";

const CONTINUE_DIRECTIVE =
  "Продолжи историю ровно там, где она прервалась. Игрок не совершает действия на этом ходу — развивай сцену естественно через повествование, диалог или события, затем сделай паузу на моменте, который приглашает его следующее действие.";

const SIDEBAR_ICONS = {
  chats: "/sidebar-icons/chats.png",
  characters: "/sidebar-icons/characters.png",
  textModel: "/sidebar-icons/text-model.png",
  story: "/sidebar-icons/story.png",
  images: "/sidebar-icons/images.png",
  localData: "/sidebar-icons/local-data.png",
  support: "/sidebar-icons/support.png",
} as const;

type InputMode = "do" | "say" | "story";

const INPUT_MODES: Array<{ value: InputMode; label: string; placeholder: string }> = [
  { value: "do", label: "Действие", placeholder: "Что ты делаешь?" },
  { value: "say", label: "Речь", placeholder: "Что ты говоришь?" },
  { value: "story", label: "История", placeholder: "Напиши следующую часть истории…" },
];

const PROSE_SIZE_OPTIONS: Array<{ value: ProseSize; label: string; className: string }> = [
  { value: "tiny", label: "12px", className: "text-xs leading-5" },
  { value: "xsmall", label: "14px", className: "text-sm leading-6" },
  { value: "small", label: "16px", className: "text-base leading-7" },
  { value: "medium", label: "18px", className: "text-lg leading-8" },
  { value: "large", label: "20px", className: "text-xl leading-9" },
  { value: "xlarge", label: "22px", className: "text-[1.375rem] leading-10" },
  { value: "xxlarge", label: "24px", className: "text-2xl leading-10" },
  { value: "huge", label: "28px", className: "text-[1.75rem] leading-[2.75rem]" },
  { value: "giant", label: "32px", className: "text-[2rem] leading-[3rem]" },
];

const RESPONSE_LENGTH_OPTIONS: Array<{ value: ResponseLength; label: string }> = [
  { value: "short", label: "Кратко" },
  { value: "medium", label: "Средне" },
  { value: "long", label: "Длинно" },
  { value: "epic", label: "Эпик" },
];

function formatPlayerInput(mode: InputMode, text: string): string {
  const trimmed = text.trim();
  if (mode === "say") {
    const quoted = /^["'"].*["'"]$/.test(trimmed) ? trimmed : `"${trimmed}"`;
    return `> You say ${quoted}`;
  }
  if (mode === "do") {
    return `> ${trimmed}`;
  }
  return trimmed;
}

const STORY_PRESETS = [
  {
    id: "fantasy",
    label: "Фэнтези",
    flavor: "Рыцари, магия, старые дороги",
    seed: "Высокое фэнтези: враждующие королевства, древняя магия и дороги, что перестают быть безопасными после заката.",
    rolePlaceholder: "странствующий наёмник",
  },
  {
    id: "mystery",
    label: "Детектив",
    flavor: "Дождь, секреты, незакрытые нити",
    seed: "Залитый дождём город, полный секретов, где каждое дело — дверь, которую кто-то хочет держать закрытой.",
    rolePlaceholder: "частный детектив",
  },
  {
    id: "cyberpunk",
    label: "Киберпанк",
    flavor: "Неон, хром, дурные долги",
    seed: "Залитый неоном мегаполис под властью корпораций, где память — валюта, и каждый кому-то должен.",
    rolePlaceholder: "выгоревший нетраннер",
  },
  {
    id: "apocalyptic",
    label: "Постапокалипсис",
    flavor: "После конца всего",
    seed: "Спустя годы после краха разрозненные выжившие шарят по руинам, меняются и травят байки о том, как было раньше.",
    rolePlaceholder: "сборщик хлама с картой",
  },
  {
    id: "horror",
    label: "Хоррор",
    flavor: "Здесь что-то не так",
    seed: "Глухой городок, где ночи тянутся долго, а местные не говорят о том, что в них творится.",
    rolePlaceholder: "приезжий",
  },
  {
    id: "romance",
    label: "Романтика",
    flavor: "Искры в неожиданных местах",
    seed: "Тесный приморский городок на исходе лета, где случайные встречи имеют свойство перерастать в нечто большее.",
    rolePlaceholder: "новичок с прошлым",
  },
] as const;

type StoryPresetId = (typeof STORY_PRESETS)[number]["id"] | "custom";

type ImageStatus = Record<string, "loading" | "error">;
type ChatResponse = {
  chat: StoryChat;
  heroId?: string | null;
  heroRpg?: CharacterRpg | null;
  items?: Item[];
  events?: GameEvent[];
};
type ChatsResponse = { chats: StoryChatSummary[] };
type CharacterResponse = { character: StoryCharacter };
type CharacterDraft = {
  name: string;
  details: string;
  inventory: string;
  skills: string;
  spells: string;
  portrait?: Attachment;
};
type MobileTool = "characters" | "story" | "images" | "data";
type DesktopPanel =
  | "chats"
  | "characters"
  | "textModel"
  | "story"
  | "images"
  | "voice"
  | "localData"
  | "support";
type LocalTextStatus = { ok: boolean; installedModels: string[] };
type ImageWorkerStatus = {
  ok: boolean;
  loaded?: boolean;
  defaultBackend?: string;
  mfluxDir?: string;
  ultraRepo?: string;
};
type RuntimeDefaultsResponse = { settings?: StorySettings };
type RuntimeHealthResponse = {
  localText?: LocalTextStatus;
  flux?: ImageWorkerStatus;
};
type ImageWorkerActionResponse = {
  ok: boolean;
  status?: "running" | "starting";
  message?: string;
  error?: string;
  path?: string;
  logPath?: string;
  health?: ImageWorkerStatus;
};
type PanelControlProps = {
  open?: boolean;
  onOpenChange?: (open: boolean) => void;
  divided?: boolean;
};

const DESKTOP_PANEL_ORDER: DesktopPanel[] = [
  "chats",
  "characters",
  "textModel",
  "story",
  "images",
  "voice",
  "localData",
  "support",
];

function emptyCharacterDraft(): CharacterDraft {
  return {
    name: "",
    details: "",
    inventory: "",
    skills: "",
    spells: "",
  };
}

function makeId() {
  if (typeof crypto?.randomUUID === "function") {
    return crypto.randomUUID();
  }

  const bytes = new Uint8Array(16);
  crypto.getRandomValues(bytes);
  bytes[6] = (bytes[6] & 0x0f) | 0x40;
  bytes[8] = (bytes[8] & 0x3f) | 0x80;

  const hex = Array.from(bytes, (byte) => byte.toString(16).padStart(2, "0"));
  return `${hex.slice(0, 4).join("")}-${hex.slice(4, 6).join("")}-${hex
    .slice(6, 8)
    .join("")}-${hex.slice(8, 10).join("")}-${hex.slice(10).join("")}`;
}

function isRecord(value: unknown): value is Record<string, unknown> {
  return typeof value === "object" && value !== null;
}

function isAbortError(error: unknown) {
  return error instanceof Error && error.name === "AbortError";
}

async function readApi<T>(response: Response): Promise<T> {
  const payload = (await response.json().catch(() => ({}))) as unknown;

  if (!response.ok) {
    const message =
      isRecord(payload) && typeof payload.error === "string"
        ? payload.error
        : `Request failed (${response.status}).`;
    throw new Error(message);
  }

  return payload as T;
}

async function fetchRuntimeHealth(): Promise<RuntimeHealthResponse> {
  try {
    const response = await fetch("/api/health", { cache: "no-store" });
    return (await response.json().catch(() => ({}))) as RuntimeHealthResponse;
  } catch {
    return {};
  }
}

function fileToDataUrl(file: File) {
  return new Promise<string>((resolve, reject) => {
    const reader = new FileReader();
    reader.onload = () => resolve(String(reader.result));
    reader.onerror = () => reject(reader.error);
    reader.readAsDataURL(file);
  });
}

async function uploadImageFile(file: File) {
  const dataUrl = await fileToDataUrl(file);
  const response = await fetch("/api/upload", {
    method: "POST",
    headers: { "Content-Type": "application/json" },
    body: JSON.stringify({ dataUrl, name: file.name, type: file.type }),
  });

  return readApi<Attachment>(response);
}

// Some chat-template models leak control tokens such as <|im_end|> or
// <|eot_id|>. Strip any complete <|...|> token, and also drop a trailing
// unclosed '<|...' so a partially streamed token never flashes on screen.
function stripModelMarkup(content: string): string {
  return content.replace(/<\|[^>]*?\|>/g, "").replace(/<\|[^|]*$/, "");
}

// Narrator models often use markdown emphasis; render just *italic* and
// **bold** inline without pulling in a full markdown pipeline.
function renderStoryEmphasis(content: string): ReactNode[] {
  return content.split(/(\*\*[^*\n]+\*\*|\*[^*\n]+\*)/g).map((segment, index) => {
    if (segment.startsWith("**") && segment.endsWith("**") && segment.length > 4) {
      return <strong key={index}>{segment.slice(2, -2)}</strong>;
    }
    if (segment.startsWith("*") && segment.endsWith("*") && segment.length > 2) {
      return <em key={index}>{segment.slice(1, -1)}</em>;
    }
    return segment;
  });
}

function formatChatDate(value: string) {
  const date = new Date(value);
  if (Number.isNaN(date.getTime())) {
    return "";
  }

  return new Intl.DateTimeFormat(undefined, {
    month: "short",
    day: "numeric",
  }).format(date);
}

function storyProseClassName(size: ProseSize) {
  return (
    PROSE_SIZE_OPTIONS.find((option) => option.value === size)?.className ??
    "text-lg leading-8"
  );
}

function proseSizeSliderValue(size: ProseSize) {
  return Math.max(
    0,
    PROSE_SIZE_OPTIONS.findIndex((option) => option.value === size),
  );
}

function chatToSummary(chat: StoryChat): StoryChatSummary {
  return {
    id: chat.id,
    title: chat.title,
    settings: chat.settings,
    createdAt: chat.createdAt,
    updatedAt: chat.updatedAt,
    messageCount: chat.messageCount,
    lastMessagePreview: chat.lastMessagePreview,
  };
}

// Split a passage into sentence-ish chunks so TTS can stream: play the first
// while the next is still synthesizing. Russian punctuation aware; tiny
// fragments merge into the neighbour so we don't synth two-word clips.
function splitSentences(text: string): string[] {
  const raw = text.replace(/\s+/g, " ").trim();
  if (!raw) return [];
  const parts = raw.match(/[^.!?…]+[.!?…]+(?:["”»)]*)|\S[^.!?…]*$/g) || [raw];
  const out: string[] = [];
  for (const part of parts) {
    const s = part.trim();
    if (!s) continue;
    if (out.length && (s.length < 14 || out[out.length - 1].length < 14)) {
      out[out.length - 1] = `${out[out.length - 1]} ${s}`;
    } else {
      out.push(s);
    }
  }
  return out.length ? out : [raw];
}

export default function Home() {
  const [chats, setChats] = useState<StoryChatSummary[]>([]);
  const [selectedChatId, setSelectedChatId] = useState("");
  const [messages, setMessages] = useState<StoryMessage[]>([]);
  const [characters, setCharacters] = useState<StoryCharacter[]>([]);
  const [characterDraft, setCharacterDraft] = useState<CharacterDraft>(emptyCharacterDraft);
  const [settings, setSettings] = useState<StorySettings>(DEFAULT_STORY_SETTINGS);
  const [input, setInput] = useState("");
  const [attachments, setAttachments] = useState<Attachment[]>([]);
  const [busy, setBusy] = useState(false);
  const [uploading, setUploading] = useState(false);
  const [characterSaving, setCharacterSaving] = useState(false);
  const [characterUploadingId, setCharacterUploadingId] = useState("");
  const [clearingLocalData, setClearingLocalData] = useState(false);
  const [libraryLoading, setLibraryLoading] = useState(true);
  const [loadingChat, setLoadingChat] = useState(false);
  const [error, setError] = useState("");
  const [imageStatus, setImageStatus] = useState<ImageStatus>({});
  const [mobileToolsOpen, setMobileToolsOpen] = useState(false);
  const [mobileTool, setMobileTool] = useState<MobileTool>("characters");
  const [activeDesktopPanel, setActiveDesktopPanel] = useState<DesktopPanel | null>(null);
  const [localTextStatus, setLocalTextStatus] = useState<LocalTextStatus | null>(null);
  const [imageWorkerStatus, setImageWorkerStatus] = useState<ImageWorkerStatus | null>(null);
  const [imageWorkerBusy, setImageWorkerBusy] = useState(false);
  const [imageWorkerMessage, setImageWorkerMessage] = useState("");
  const [newStoryOpen, setNewStoryOpen] = useState(false);
  const [inputMode, setInputMode] = useState<InputMode>("do");
  const [suggestedActions, setSuggestedActions] = useState<Array<{ emoji?: string; label: string }>>([]);
  const [journal, setJournal] = useState<GameEvent[]>([]);
  const [diceQueue, setDiceQueue] = useState<DiceJob[]>([]);
  const [heroRpg, setHeroRpg] = useState<CharacterRpg | null>(null);
  const [items, setItems] = useState<Item[]>([]);
  const [bookMode, setBookMode] = useState(false);
  // DEV-only manual dice trigger (stripped from production builds):
  //   window.__odRollDie(20, "critSuccess", "Сила · d20 20 +4 = 24 ≥ 15 → крит. успех")
  useEffect(() => {
    if (process.env.NODE_ENV === "production") return;
    (window as unknown as { __odRollDie?: (d: number, o?: string, c?: string) => void }).__odRollDie = (
      d,
      o = "success",
      c = "Ловкость · d20 = проверка",
    ) => {
      setSettings((cur) => (cur.rpgEnabled ? cur : { ...cur, rpgEnabled: true }));
      setDiceQueue((q) => [
        ...q,
        { id: makeId(), d20: Math.max(1, Math.min(20, Math.round(d) || 1)), outcome: o as DiceOutcome, caption: c },
      ]);
    };
  }, []);
  const [editingId, setEditingId] = useState("");
  const [editDraft, setEditDraft] = useState("");
  const lastSavedSettingsRef = useRef(JSON.stringify(DEFAULT_STORY_SETTINGS));
  const defaultSettingsRef = useRef(DEFAULT_STORY_SETTINGS);
  const fileInputRef = useRef<HTMLInputElement>(null);
  const endRef = useRef<HTMLDivElement>(null);
  const audioRef = useRef<HTMLAudioElement | null>(null);
  const storyAbortRef = useRef<AbortController | null>(null);
  const stoppedByUserRef = useRef(false);
  const [speakingId, setSpeakingId] = useState("");
  const audioCacheRef = useRef<Map<string, string>>(new Map());
  const speakSeqRef = useRef(0);

  const activeChat = useMemo(
    () => chats.find((chat) => chat.id === selectedChatId),
    [chats, selectedChatId],
  );

  const lastUserAttachments = useMemo(() => {
    return [...messages].reverse().find((message) => message.role === "user")?.attachments || [];
  }, [messages]);

  const orderedDesktopPanels = useMemo(() => {
    return activeDesktopPanel ? [activeDesktopPanel] : DESKTOP_PANEL_ORDER;
  }, [activeDesktopPanel]);

  const applyChat = useCallback(
    (
      chat: StoryChat,
      hero: CharacterRpg | null = null,
      ownedItems: Item[] = [],
      events: GameEvent[] = [],
    ) => {
      setSelectedChatId(chat.id);
      window.localStorage.setItem(SELECTED_CHAT_KEY, chat.id);
      setMessages(chat.messages);
      setCharacters(chat.characters || []);
      setCharacterDraft(emptyCharacterDraft());
      setSettings(chat.settings);
      setAttachments([]);
      setImageStatus({});
      setHeroRpg(hero);
      setItems(ownedItems);
      setJournal(events);
      lastSavedSettingsRef.current = JSON.stringify(chat.settings);
    },
    [],
  );

  const applyDefaultSettings = useCallback(() => {
    const defaults = defaultSettingsRef.current;
    setSettings(defaults);
    lastSavedSettingsRef.current = JSON.stringify(defaults);
  }, []);

  const refreshChats = useCallback(async () => {
    const response = await fetch("/api/chats", { cache: "no-store" });
    const payload = await readApi<ChatsResponse>(response);
    setChats(payload.chats);
    return payload.chats;
  }, []);

  const loadChat = useCallback(
    async (chatId: string) => {
      setLoadingChat(true);
      setError("");

      try {
        const response = await fetch(`/api/chats/${chatId}`, { cache: "no-store" });
        const payload = await readApi<ChatResponse>(response);
        applyChat(payload.chat, payload.heroRpg ?? null, payload.items ?? [], payload.events ?? []);
      } catch (loadError) {
        setError(loadError instanceof Error ? loadError.message : "Не удалось загрузить чат.");
      } finally {
        setLoadingChat(false);
      }
    },
    [applyChat],
  );

  const clearSelectedChat = useCallback(() => {
    window.localStorage.removeItem(SELECTED_CHAT_KEY);
    setSelectedChatId("");
    setMessages([]);
    setCharacters([]);
    setCharacterDraft(emptyCharacterDraft());
    applyDefaultSettings();
    setAttachments([]);
    setImageStatus({});
    setHeroRpg(null);
    setItems([]);
  }, [applyDefaultSettings]);

  const equipItem = useCallback(
    async (itemId: string, equipped: boolean) => {
      if (!selectedChatId) return;
      const snapshot = items;
      setItems((current) => current.map((it) => (it.id === itemId ? { ...it, equipped } : it)));
      try {
        const response = await fetch(`/api/chats/${selectedChatId}/items/${itemId}`, {
          method: "PATCH",
          headers: { "Content-Type": "application/json" },
          body: JSON.stringify({ equipped }),
        });
        const payload = await readApi<{ item: Item }>(response);
        setItems((current) => current.map((it) => (it.id === itemId ? payload.item : it)));
      } catch (equipError) {
        setItems(snapshot);
        setError(equipError instanceof Error ? equipError.message : "Не удалось обновить предмет.");
      }
    },
    [items, selectedChatId],
  );

  // Pull the authoritative hero stats + inventory back after a turn changed them,
  // so the HUD's HP bar / inventory update live instead of only on reload.
  const refreshRpg = useCallback(async (chatId: string) => {
    try {
      const response = await fetch(`/api/chats/${chatId}`, { cache: "no-store" });
      const payload = await readApi<ChatResponse>(response);
      setHeroRpg(payload.heroRpg ?? null);
      setItems(payload.items ?? []);
    } catch {
      // best-effort HUD refresh
    }
  }, []);

  // When the narrator marks a loot drop as illustrated (withImage), generate a
  // dedicated portrait for each new item, then refresh so the inventory shows it.
  // Sequential on purpose: one image worker, one GPU. Best-effort.
  const illustrateDroppedItems = useCallback(
    async (chatId: string, events: GameEvent[]) => {
      if (!settings.imageGenerationEnabled) return;
      const itemIds = events
        .filter((event) => event.kind === "item")
        .map((event) => event.data as { item?: Item; withImage?: boolean } | undefined)
        .filter((data) => data?.withImage && data.item?.id)
        .map((data) => data!.item!.id);
      if (!itemIds.length) return;
      for (const itemId of itemIds) {
        try {
          await fetch(`/api/chats/${chatId}/items/${itemId}/image`, {
            method: "POST",
            headers: { "Content-Type": "application/json" },
            body: "{}",
          });
        } catch {
          // best-effort item illustration
        }
      }
      void refreshRpg(chatId);
    },
    [settings.imageGenerationEnabled, refreshRpg],
  );

  // Abort the in-flight turn when the player presses Stop (a turn can run for
  // minutes on a loaded GPU); flagged so the catch treats it as a stop, not an error.
  const stopTurn = useCallback(() => {
    stoppedByUserRef.current = true;
    storyAbortRef.current?.abort();
  }, []);

  // DEV-only mock loot for verifying the character sheet without an LLM turn:
  //   window.__odMockLoot()  -> drops gear (some equipped, with stat modifiers)
  useEffect(() => {
    if (process.env.NODE_ENV === "production") return;
    (window as unknown as { __odMockLoot?: () => void }).__odMockLoot = () => {
      const now = new Date().toISOString();
      setItems([
        {
          id: makeId(),
          name: "Клинок ярости",
          slot: "weapon",
          rarity: "rare",
          damage: "1d8+2",
          modifiers: { str: 2, ac: 1 },
          equipped: true,
          qty: 1,
          createdAt: now,
        },
        {
          id: makeId(),
          name: "Кольцо стража",
          slot: "trinket",
          rarity: "epic",
          modifiers: { maxHp: 6, con: 1 },
          equipped: true,
          qty: 1,
          createdAt: now,
        },
        {
          id: makeId(),
          name: "Зелье лечения",
          slot: "consumable",
          rarity: "common",
          modifiers: {},
          equipped: false,
          qty: 3,
          createdAt: now,
        },
      ]);
    };
  }, []);

  const deleteChatById = useCallback(
    async (chatId: string) => {
      setError("");

      try {
        const response = await fetch(`/api/chats/${chatId}`, { method: "DELETE" });
        await readApi<{ ok: true }>(response);
        const remainingChats = await refreshChats();

        if (chatId !== selectedChatId) {
          return;
        }

        if (remainingChats.length) {
          await loadChat(remainingChats[0].id);
          return;
        }

        clearSelectedChat();
      } catch (deleteError) {
        setError(deleteError instanceof Error ? deleteError.message : "Не удалось удалить.");
      }
    },
    [clearSelectedChat, loadChat, refreshChats, selectedChatId],
  );

  const clearAllLocalData = useCallback(async () => {
    setClearingLocalData(true);
    setError("");

    try {
      const response = await fetch("/api/local-data", { method: "DELETE" });
      await readApi<{ ok: true }>(response);

      setChats([]);
      clearSelectedChat();
    } catch (clearError) {
      setError(clearError instanceof Error ? clearError.message : "Не удалось очистить локальные данные.");
    } finally {
      setClearingLocalData(false);
    }
  }, [clearSelectedChat]);

  useEffect(() => {
    let cancelled = false;

    async function boot() {
      setLibraryLoading(true);
      setError("");

      try {
        try {
          const defaultsResponse = await fetch("/api/settings/defaults", { cache: "no-store" });
          const defaultsPayload = (await defaultsResponse.json().catch(() => ({}))) as RuntimeDefaultsResponse;
          if (defaultsPayload.settings) {
            defaultSettingsRef.current = defaultsPayload.settings;
          }
        } catch {
          // Static defaults are fine if runtime defaults are unavailable.
        }

        const nextChats = await refreshChats();
        let nextChatId = window.localStorage.getItem(SELECTED_CHAT_KEY) || "";

        if (!nextChats.some((chat) => chat.id === nextChatId)) {
          nextChatId = nextChats[0]?.id || "";
        }

        if (nextChatId && !cancelled) {
          const response = await fetch(`/api/chats/${nextChatId}`, { cache: "no-store" });
          const payload = await readApi<ChatResponse>(response);
          if (!cancelled) {
            applyChat(payload.chat, payload.heroRpg ?? null, payload.items ?? [], payload.events ?? []);
          }
        } else if (!cancelled) {
          applyDefaultSettings();
        }
      } catch (bootError) {
        if (!cancelled) {
          setError(bootError instanceof Error ? bootError.message : "Не удалось загрузить библиотеку историй.");
        }
      } finally {
        if (!cancelled) {
          setLibraryLoading(false);
        }
      }
    }

    void boot();

    return () => {
      cancelled = true;
    };
  }, [applyChat, applyDefaultSettings, refreshChats]);

  const applyRuntimeHealth = useCallback((payload: RuntimeHealthResponse) => {
    setLocalTextStatus(payload.localText ?? { ok: false, installedModels: [] });
    setImageWorkerStatus(payload.flux ?? { ok: false, loaded: false });
  }, []);

  const refreshRuntimeHealth = useCallback(async () => {
    applyRuntimeHealth(await fetchRuntimeHealth());
  }, [applyRuntimeHealth]);

  useEffect(() => {
    let cancelled = false;

    async function checkRuntimeHealth() {
      const payload = await fetchRuntimeHealth();
      if (!cancelled) {
        applyRuntimeHealth(payload);
      }
    }

    void checkRuntimeHealth();

    return () => {
      cancelled = true;
    };
  }, [applyRuntimeHealth]);

  useEffect(() => {
    if (!selectedChatId || libraryLoading || loadingChat) {
      return;
    }

    const serialized = JSON.stringify(settings);
    if (serialized === lastSavedSettingsRef.current) {
      return;
    }

    const controller = new AbortController();
    const timeout = window.setTimeout(async () => {
      try {
        const response = await fetch(`/api/chats/${selectedChatId}`, {
          method: "PATCH",
          headers: { "Content-Type": "application/json" },
          body: JSON.stringify({ settings }),
          signal: controller.signal,
        });
        const payload = await readApi<ChatResponse>(response);
        lastSavedSettingsRef.current = JSON.stringify(payload.chat.settings);
        setChats((current) =>
          current.map((chat) =>
            chat.id === payload.chat.id ? chatToSummary(payload.chat) : chat,
          ),
        );
      } catch (saveError) {
        if (!controller.signal.aborted) {
          setError(saveError instanceof Error ? saveError.message : "Не удалось сохранить настройки.");
        }
      }
    }, 500);

    return () => {
      controller.abort();
      window.clearTimeout(timeout);
    };
  }, [libraryLoading, loadingChat, selectedChatId, settings]);

  useEffect(() => {
    endRef.current?.scrollIntoView({ block: "end" });
  }, [messages, busy, imageStatus]);

  const setImageGenerationEnabled = useCallback((imageGenerationEnabled: boolean) => {
    setSettings((current) => ({ ...current, imageGenerationEnabled }));
    if (!imageGenerationEnabled) {
      setImageStatus({});
      setError("");
    }
  }, []);

  async function startImageWorker() {
    setImageWorkerBusy(true);
    setImageWorkerMessage("");
    setError("");

    try {
      const response = await fetch("/api/image-worker", {
        method: "POST",
        headers: { "Content-Type": "application/json" },
        body: JSON.stringify({ action: "start" }),
      });
      const payload = await readApi<ImageWorkerActionResponse>(response);
      setImageWorkerMessage(payload.message || "Запрошен запуск сервера изображений.");
      if (payload.health) {
        setImageWorkerStatus(payload.health);
      }
      await refreshRuntimeHealth();
    } catch (workerError) {
      const message =
        workerError instanceof Error ? workerError.message : "Не удалось запустить сервер изображений.";
      setImageWorkerMessage(message);
      setError(message);
    } finally {
      setImageWorkerBusy(false);
    }
  }

  async function openImageModelFolder() {
    setImageWorkerBusy(true);
    setImageWorkerMessage("");
    setError("");

    try {
      const response = await fetch("/api/image-worker", {
        method: "POST",
        headers: { "Content-Type": "application/json" },
        body: JSON.stringify({ action: "open-model-folder" }),
      });
      const payload = await readApi<ImageWorkerActionResponse>(response);
      setImageWorkerMessage(
        payload.path ? `Папка модели открыта: ${payload.path}` : "Папка модели открыта.",
      );
    } catch (workerError) {
      const message =
        workerError instanceof Error ? workerError.message : "Не удалось открыть папку модели.";
      setImageWorkerMessage(message);
      setError(message);
    } finally {
      setImageWorkerBusy(false);
    }
  }

  async function handleFiles(files: FileList | null) {
    if (!files?.length) {
      return;
    }

    setUploading(true);
    setError("");

    try {
      const uploaded: Attachment[] = [];
      for (const file of Array.from(files).slice(0, MAX_IMAGE_REFERENCES)) {
        uploaded.push(await uploadImageFile(file));
      }
      setAttachments((current) => [...current, ...uploaded].slice(0, MAX_IMAGE_REFERENCES));
    } catch (uploadError) {
      setError(uploadError instanceof Error ? uploadError.message : "Не удалось загрузить изображение.");
    } finally {
      setUploading(false);
      if (fileInputRef.current) {
        fileInputRef.current.value = "";
      }
    }
  }

  const referencesForImage = useCallback(
    (characterIds: string[] | undefined, turnRefs: Attachment[]) => {
      const characterRefs = (characterIds || []).flatMap((characterId) => {
        const portrait = characters.find((character) => character.id === characterId)?.portrait;
        return portrait ? [portrait] : [];
      });
      const seen = new Set<string>();

      return [...characterRefs, ...turnRefs]
        .filter((reference) => {
          if (seen.has(reference.id)) {
            return false;
          }
          seen.add(reference.id);
          return true;
        })
        .slice(0, MAX_IMAGE_REFERENCES);
    },
    [characters],
  );

  async function createCharacterFromDraft() {
    const name = characterDraft.name.trim();

    if (!selectedChatId || !name || characterSaving) {
      return;
    }

    setCharacterSaving(true);
    setError("");

    try {
      const response = await fetch(`/api/chats/${selectedChatId}/characters`, {
        method: "POST",
        headers: { "Content-Type": "application/json" },
        body: JSON.stringify({
          name,
          details: characterDraft.details,
          inventory: characterDraft.inventory,
          skills: characterDraft.skills,
          spells: characterDraft.spells,
          portrait: characterDraft.portrait,
        }),
      });
      const payload = await readApi<CharacterResponse>(response);
      setCharacters((current) => [
        payload.character,
        ...current.filter((character) => character.id !== payload.character.id),
      ]);
      setCharacterDraft(emptyCharacterDraft());
      void refreshChats();
    } catch (characterError) {
      setError(characterError instanceof Error ? characterError.message : "Не удалось сохранить персонажа.");
    } finally {
      setCharacterSaving(false);
    }
  }

  async function updateCharacterById(
    characterId: string,
    updates: {
      name?: string;
      details?: string;
      inventory?: string;
      skills?: string;
      spells?: string;
      portrait?: Attachment | null;
    },
  ) {
    if (!selectedChatId) {
      return;
    }

    setError("");

    try {
      const response = await fetch(`/api/chats/${selectedChatId}/characters/${characterId}`, {
        method: "PATCH",
        headers: { "Content-Type": "application/json" },
        body: JSON.stringify(updates),
      });
      const payload = await readApi<CharacterResponse>(response);
      setCharacters((current) =>
        current.map((character) =>
          character.id === payload.character.id ? payload.character : character,
        ),
      );
      void refreshChats();
    } catch (characterError) {
      setError(characterError instanceof Error ? characterError.message : "Не удалось обновить персонажа.");
    }
  }

  async function uploadCharacterPortrait(file: File, characterId?: string) {
    const uploadId = characterId || "draft";
    setCharacterUploadingId(uploadId);
    setError("");

    try {
      const portrait = await uploadImageFile(file);

      if (!characterId) {
        setCharacterDraft((current) => ({ ...current, portrait }));
        return;
      }

      await updateCharacterById(characterId, { portrait });
    } catch (uploadError) {
      setError(uploadError instanceof Error ? uploadError.message : "Не удалось загрузить портрет персонажа.");
    } finally {
      setCharacterUploadingId("");
    }
  }

  async function deleteCharacterById(characterId: string) {
    if (!selectedChatId) {
      return;
    }

    setError("");

    try {
      const response = await fetch(`/api/chats/${selectedChatId}/characters/${characterId}`, {
        method: "DELETE",
      });
      await readApi<{ ok: true }>(response);
      setCharacters((current) => current.filter((character) => character.id !== characterId));
      void refreshChats();
    } catch (deleteError) {
      setError(deleteError instanceof Error ? deleteError.message : "Не удалось удалить персонажа.");
    }
  }

  async function requestGeneratedImage(
    messageId: string,
    prompt: string,
    refs: Attachment[],
    imageRequest?: StoryMessage["imageRequest"],
  ) {
    if (!settings.imageGenerationEnabled) {
      return;
    }

    setImageStatus((current) => ({ ...current, [messageId]: "loading" }));

    try {
      const response = await fetch("/api/images", {
        method: "POST",
        headers: { "Content-Type": "application/json" },
        body: JSON.stringify({
          messageId,
          prompt,
          mode: imageRequest?.mode || settings.imageMode,
          backend: imageRequest?.backend || settings.imageBackend,
          aspect: imageRequest?.aspect || settings.aspect,
          references: refs,
        }),
      });
      const generatedImage = await readApi<GeneratedImage>(response);

      setMessages((current) =>
        current.map((message) =>
          message.id === messageId ? { ...message, generatedImage } : message,
        ),
      );
      setImageStatus((current) => {
        const next = { ...current };
        delete next[messageId];
        return next;
      });
      void refreshChats();
    } catch (imageError) {
      setError(imageError instanceof Error ? imageError.message : "Не удалось сгенерировать изображение.");
      setImageStatus((current) => ({ ...current, [messageId]: "error" }));
    }
  }

  // Shared core for every narrator turn (new turn, kickoff, continue, retry).
  function stopSpeaking() {
    speakSeqRef.current += 1; // invalidate any in-flight streaming sequence
    if (audioRef.current) {
      audioRef.current.pause();
    }
    setSpeakingId("");
  }

  async function fetchChunkUrl(
    messageId: string,
    chunkIndex: number,
    chunkText: string,
    voice: string,
  ): Promise<string | null> {
    const key = `${messageId}__c${chunkIndex}__${voice}`;
    const cached = audioCacheRef.current.get(key);
    if (cached) return cached;
    try {
      const response = await fetch("/api/tts", {
        method: "POST",
        headers: { "Content-Type": "application/json" },
        body: JSON.stringify({ messageId, chunkIndex, text: chunkText, voice }),
      });
      if (!response.ok) return null;
      const data = (await response.json()) as { url?: string };
      if (data.url) audioCacheRef.current.set(key, data.url);
      return data.url || null;
    } catch {
      return null;
    }
  }

  function playUrl(audio: HTMLAudioElement, url: string): Promise<void> {
    return new Promise((resolve) => {
      audio.src = url;
      audio.volume = Math.min(1, Math.max(0, settings.ttsVolume));
      audio.playbackRate = settings.ttsSpeed;
      const finish = () => {
        audio.onended = null;
        audio.onerror = null;
        resolve();
      };
      audio.onended = finish;
      audio.onerror = finish;
      audio.play().catch(finish);
    });
  }

  // Stream a passage: synthesize sentence-by-sentence, play each as soon as it
  // is ready while prefetching the next. First audio starts in ~1-2s instead of
  // waiting for the whole page; cached chunks replay instantly.
  async function speakText(messageId: string, text: string) {
    if (speakingId === messageId) {
      stopSpeaking();
      return;
    }
    stopSpeaking();
    const seq = (speakSeqRef.current += 1);
    setSpeakingId(messageId);
    const voice = settings.voice;
    const sentences = splitSentences(text);
    if (!sentences.length) {
      setSpeakingId("");
      return;
    }
    const audio = audioRef.current ?? new Audio();
    audioRef.current = audio;
    let nextUrl = fetchChunkUrl(messageId, 0, sentences[0], voice);
    for (let i = 0; i < sentences.length; i += 1) {
      if (speakSeqRef.current !== seq) return;
      const url = await nextUrl;
      if (speakSeqRef.current !== seq) return;
      nextUrl =
        i + 1 < sentences.length
          ? fetchChunkUrl(messageId, i + 1, sentences[i + 1], voice)
          : Promise.resolve(null);
      if (!url) continue;
      await playUrl(audio, url);
    }
    if (speakSeqRef.current === seq) setSpeakingId("");
  }

  async function runTurn(opts: {
    chatId: string;
    mode: "turn" | "kickoff" | "continue" | "retry" | "opening";
    input: string;
    history: StoryMessage[];
    settings: StorySettings;
    attachments?: Attachment[];
    userMessageId?: string;
  }) {
    setBusy(true);
    setError("");

    const controller = new AbortController();
    storyAbortRef.current = controller;
    stoppedByUserRef.current = false;
    const timeoutId = window.setTimeout(() => controller.abort(), STORY_REQUEST_TIMEOUT_MS);

    try {
      const response = await fetch("/api/story", {
        method: "POST",
        headers: { "Content-Type": "application/json" },
        signal: controller.signal,
        body: JSON.stringify({
          chatId: opts.chatId,
          mode: opts.mode,
          userMessageId: opts.userMessageId,
          input: opts.input,
          messages: opts.history,
          attachments: opts.attachments || [],
          settings: opts.settings,
        }),
      });

      const contentType = response.headers.get("content-type") || "";
      const isStream = contentType.includes("text/event-stream");

      let streamMessageId = "";

      const finalize = (final: {
        id?: string;
        content?: string;
        imageRequest?: StoryMessage["imageRequest"];
      }) => {
        const text = (final.content ?? "").trim();
        if (opts.settings.autoplay && text) {
          void speakText(final.id || streamMessageId || makeId(), text);
        }
        void refreshChats();
        void fetchSuggestedActions(text);
        if (
          opts.settings.imageGenerationEnabled &&
          final.imageRequest?.needed &&
          final.imageRequest.prompt &&
          (final.id || streamMessageId)
        ) {
          void requestGeneratedImage(
            final.id || streamMessageId,
            final.imageRequest.prompt,
            referencesForImage(final.imageRequest.characterIds, opts.attachments || []),
            final.imageRequest,
          );
        }
      };

      if (isStream && response.body) {
        const reader = response.body.getReader();
        const decoder = new TextDecoder();
        let buffer = "";
        let assembled = "";
        let streamErrorMessage = "";
        let done = false;

        // Create the assistant bubble lazily on the first text delta, then
        // append every subsequent fragment in place.
        const pushDelta = (text: string) => {
          if (!text) {
            return;
          }
          assembled += text;
          if (!streamMessageId) {
            streamMessageId = makeId();
            const created = streamMessageId;
            setMessages((current) => [
              ...current,
              {
                id: created,
                role: "assistant",
                content: text,
                createdAt: new Date().toISOString(),
              },
            ]);
          } else {
            const target = streamMessageId;
            setMessages((current) =>
              current.map((message) =>
                message.id === target
                  ? { ...message, content: message.content + text }
                  : message,
              ),
            );
          }
        };

        const handleEvent = (eventName: string, dataRaw: string) => {
          if (!dataRaw) {
            return;
          }
          let data: {
            text?: string;
            id?: string;
            content?: string;
            error?: string;
            imageRequest?: StoryMessage["imageRequest"];
            events?: GameEvent[];
          };
          try {
            data = JSON.parse(dataRaw);
          } catch {
            return;
          }
          if (eventName === "delta") {
            pushDelta(data.text || "");
          } else if (eventName === "error") {
            streamErrorMessage = data.error || "Поток истории прервался.";
            done = true;
          } else if (eventName === "done") {
            done = true;
            // Reconcile the id (server-authoritative) and final content so a
            // reload matches, and trigger autoplay + auto image.
            if (streamMessageId) {
              const fromId = streamMessageId;
              streamMessageId = data.id || streamMessageId;
              const toId = streamMessageId;
              setMessages((current) =>
                current.map((message) =>
                  message.id === fromId
                    ? {
                        ...message,
                        id: toId,
                        content: data.content ?? message.content,
                        imageRequest: data.imageRequest,
                      }
                    : message,
                ),
              );
            } else if (data.content) {
              // No deltas arrived (e.g. tool-only turn) — add the passage now.
              streamMessageId = data.id || makeId();
              const created = streamMessageId;
              setMessages((current) => [
                ...current,
                {
                  id: created,
                  role: "assistant",
                  content: data.content || "",
                  createdAt: new Date().toISOString(),
                  imageRequest: data.imageRequest,
                },
              ]);
            }
            finalize({
              id: streamMessageId,
              content: data.content ?? assembled,
              imageRequest: data.imageRequest,
            });
            if (Array.isArray(data.events) && data.events.length) {
              const incoming = data.events as GameEvent[];
              setJournal((current) => [...current, ...incoming]);
              const jobs = rollJobsFromEvents(incoming);
              if (jobs.length) setDiceQueue((queue) => [...queue, ...jobs]);
              if (
                selectedChatId &&
                incoming.some((e) => e.kind === "hp" || e.kind === "item" || e.kind === "death")
              ) {
                void refreshRpg(selectedChatId);
              }
              if (selectedChatId) void illustrateDroppedItems(selectedChatId, incoming);
            }
          }
        };

        // Drain the SSE body: split on the blank-line frame separator, then
        // read each frame's `event:` and (possibly multiline) `data:` fields.
        while (!done) {
          const { value, done: streamDone } = await reader.read();
          if (streamDone) {
            break;
          }
          buffer += decoder.decode(value, { stream: true });
          let sep = buffer.indexOf("\n\n");
          while (sep !== -1) {
            const frame = buffer.slice(0, sep);
            buffer = buffer.slice(sep + 2);
            let eventName = "message";
            const dataLines: string[] = [];
            for (const rawLine of frame.split("\n")) {
              const line = rawLine.replace(/\r$/, "");
              if (line.startsWith("event:")) {
                eventName = line.slice(6).trim();
              } else if (line.startsWith("data:")) {
                dataLines.push(line.slice(5).replace(/^ /, ""));
              }
            }
            handleEvent(eventName, dataLines.join("\n"));
            sep = buffer.indexOf("\n\n");
          }
        }

        if (streamErrorMessage) {
          throw new Error(streamErrorMessage);
        }
      } else {
        // Buffered fallback: Ollama provider, the 'opening' mode, or a JSON
        // error response. readApi throws on non-2xx with the server message.
        const payload = await readApi<{
          id?: string;
          content: string;
          imageRequest?: StoryMessage["imageRequest"];
          events?: GameEvent[];
        }>(response);

        const assistantMessage: StoryMessage = {
          id: payload.id || makeId(),
          role: "assistant",
          content: payload.content,
          createdAt: new Date().toISOString(),
          imageRequest: payload.imageRequest,
        };
        streamMessageId = assistantMessage.id;

        setMessages((current) => [...current, assistantMessage]);
        finalize({
          id: assistantMessage.id,
          content: assistantMessage.content,
          imageRequest: payload.imageRequest,
        });
        if (payload.events?.length) {
          setJournal((current) => [...current, ...payload.events!]);
          const jobs = rollJobsFromEvents(payload.events);
          if (jobs.length) setDiceQueue((queue) => [...queue, ...jobs]);
          if (
            selectedChatId &&
            payload.events.some((e) => e.kind === "hp" || e.kind === "item" || e.kind === "death")
          ) {
            void refreshRpg(selectedChatId);
          }
          if (selectedChatId) void illustrateDroppedItems(selectedChatId, payload.events);
        }
      }
    } catch (storyError) {
      if (stoppedByUserRef.current) stoppedByUserRef.current = false;
      else
        setError(
          isAbortError(storyError)
          ? "Рассказчик слишком долго отвечал. Модель ещё может работать в фоне; подожди немного, затем повтори или перезапусти локальную модель, если система под нагрузкой."
          : storyError instanceof Error
            ? storyError.message
            : "Не удалось выполнить запрос истории.",
      );
      void refreshChats();
    } finally {
      window.clearTimeout(timeoutId);
      setBusy(false);
    }
  }

  async function kickoffStory(chat: StoryChat, hint?: string) {
    const trimmedHint = hint?.trim();
    const input = trimmedHint
      ? `${KICKOFF_DIRECTIVE} Направление начала от игрока (выстрой сцену вокруг этого): ${trimmedHint}`
      : KICKOFF_DIRECTIVE;
    await runTurn({
      chatId: chat.id,
      mode: "kickoff",
      input,
      history: [],
      settings: chat.settings,
    });
  }

  async function continueStory() {
    if (busy || !selectedChatId || !messages.length) {
      return;
    }
    await runTurn({
      chatId: selectedChatId,
      mode: "continue",
      input: CONTINUE_DIRECTIVE,
      history: messages,
      settings,
    });
  }

  // Regenerate the most recent narrator passage for the same player action.
  async function retryLastTurn() {
    if (busy || !selectedChatId || !messages.length) {
      return;
    }

    const lastAssistantIndex = messages.map((m) => m.role).lastIndexOf("assistant");
    if (lastAssistantIndex < 0) {
      return;
    }

    const target = messages[lastAssistantIndex];
    const before = messages.slice(0, lastAssistantIndex);
    // The action that prompted it (if any) is the user message just before.
    const priorUser = [...before].reverse().find((m) => m.role === "user");

    setError("");
    try {
      await fetch(`/api/chats/${selectedChatId}/messages/${target.id}?after=1`, {
        method: "DELETE",
      });
    } catch {
      // fall through — local state is still corrected below
    }
    setMessages(before);

    if (priorUser) {
      const historyBeforeUser = before.slice(0, before.lastIndexOf(priorUser));
      await runTurn({
        chatId: selectedChatId,
        mode: "retry",
        input: priorUser.content,
        history: historyBeforeUser,
        attachments: priorUser.attachments,
        settings,
      });
    } else {
      // No prior action (e.g. the opening passage) — regenerate as a kickoff.
      await runTurn({
        chatId: selectedChatId,
        mode: "kickoff",
        input: KICKOFF_DIRECTIVE,
        history: [],
        settings,
      });
    }
  }

  // Erase the most recent exchange (the latest narrator passage and the player
  // action that prompted it), like AI Dungeon's Erase / Undo.
  async function eraseLastTurn() {
    if (busy || !selectedChatId || !messages.length) {
      return;
    }

    const lastAssistantIndex = messages.map((m) => m.role).lastIndexOf("assistant");
    const cutFrom =
      lastAssistantIndex >= 0
        ? // include the user action immediately before it, if present
          before(messages, lastAssistantIndex)
        : messages.length - 1;

    const target = messages[cutFrom];
    setError("");
    try {
      await fetch(`/api/chats/${selectedChatId}/messages/${target.id}?after=1`, {
        method: "DELETE",
      });
    } catch {
      // fall through
    }
    setMessages(messages.slice(0, cutFrom));
    void refreshChats();
  }

  function before(list: StoryMessage[], assistantIndex: number) {
    const prev = list[assistantIndex - 1];
    return prev && prev.role === "user" ? assistantIndex - 1 : assistantIndex;
  }

  function startEditing(message: StoryMessage) {
    setEditingId(message.id);
    setEditDraft(message.content);
  }

  async function saveEdit() {
    const id = editingId;
    const content = editDraft.trim();
    if (!id || !content) {
      setEditingId("");
      return;
    }

    setMessages((current) =>
      current.map((m) => (m.id === id ? { ...m, content } : m)),
    );
    setEditingId("");

    if (selectedChatId) {
      try {
        await fetch(`/api/chats/${selectedChatId}/messages/${id}`, {
          method: "PATCH",
          headers: { "Content-Type": "application/json" },
          body: JSON.stringify({ content }),
        });
        void refreshChats();
      } catch (editError) {
        setError(editError instanceof Error ? editError.message : "Не удалось сохранить изменения.");
      }
    }
  }

  async function beginStory(options: {
    title: string;
    world: string;
    opening: { mode: "narrator"; hint: string } | { mode: "self"; text: string };
  }) {
    setNewStoryOpen(false);
    setError("");

    try {
      const seedSettings: StorySettings = { ...settings, world: options.world };
      const response = await fetch("/api/chats", {
        method: "POST",
        headers: { "Content-Type": "application/json" },
        body: JSON.stringify({ settings: seedSettings, title: options.title }),
      });
      const payload = await readApi<ChatResponse>(response);
      setChats((current) => [
        payload.chat,
        ...current.filter((chat) => chat.id !== payload.chat.id),
      ]);
      applyChat(payload.chat, payload.heroRpg ?? null, payload.items ?? [], payload.events ?? []);
      void refreshChats();

      if (options.opening.mode === "self") {
        // The player wrote the opening; store it verbatim, no generation.
        await runTurn({
          chatId: payload.chat.id,
          mode: "opening",
          input: options.opening.text.trim(),
          history: [],
          settings: payload.chat.settings,
        });
      } else {
        await kickoffStory(payload.chat, options.opening.hint);
      }
    } catch (createError) {
      setError(createError instanceof Error ? createError.message : "Не удалось создать историю.");
    }
  }

  // Core turn submission, reused by the composer form, the one-button action
  // chips, and (later) voice input. Takes the raw player text directly.
  async function playInput(rawText: string) {
    const trimmed = rawText.trim();
    if (!trimmed || busy || !selectedChatId) {
      return;
    }

    const formatted = formatPlayerInput(inputMode, trimmed);
    const conversationBeforeTurn = messages;
    const turnAttachments = attachments;
    const userMessage: StoryMessage = {
      id: makeId(),
      role: "user",
      content: formatted,
      createdAt: new Date().toISOString(),
      attachments: turnAttachments,
    };

    setMessages((current) => [...current, userMessage]);
    setInput("");
    setAttachments([]);
    setSuggestedActions([]);

    await runTurn({
      chatId: selectedChatId,
      mode: "turn",
      userMessageId: userMessage.id,
      input: formatted,
      history: conversationBeforeTurn,
      attachments: turnAttachments,
      settings,
    });
  }

  async function submitTurn(event: FormEvent<HTMLFormElement>) {
    event.preventDefault();
    await playInput(input);
  }

  // One-button play: after a passage lands, ask the text server for a few quick
  // D&D-style action chips based on it. Best-effort — never blocks play.
  async function fetchSuggestedActions(passage: string) {
    if (!passage.trim()) {
      return;
    }
    try {
      const response = await fetch("/api/actions", {
        method: "POST",
        headers: { "Content-Type": "application/json" },
        body: JSON.stringify({ passage, settings }),
      });
      const data = (await response.json()) as {
        actions?: Array<{ emoji?: string; label: string }>;
      };
      if (Array.isArray(data.actions)) {
        setSuggestedActions(data.actions.slice(0, 4));
      }
    } catch {
      // chips are optional
    }
  }

  function desktopPanelControls(panel: DesktopPanel) {
    return {
      open: activeDesktopPanel === panel,
      onOpenChange: (open: boolean) => setActiveDesktopPanel(open ? panel : null),
      divided: orderedDesktopPanels[0] !== panel,
    };
  }

  function renderDesktopPanel(panel: DesktopPanel) {
    if (panel === "chats") {
      return (
        <ChatLibrary
          key={panel}
          chats={chats}
          selectedChatId={selectedChatId}
          loading={libraryLoading}
          onCreate={() => setNewStoryOpen(true)}
          onSelect={(chatId) => void loadChat(chatId)}
          onDelete={(chatId) => void deleteChatById(chatId)}
          fillAvailable={activeDesktopPanel === "chats"}
          {...desktopPanelControls(panel)}
        />
      );
    }

    if (panel === "characters") {
      return (
        <CharacterPanel
          key={panel}
          settings={settings}
          setSettings={setSettings}
          characters={characters}
          draft={characterDraft}
          creating={characterSaving}
          uploadingId={characterUploadingId}
          onDraftChange={setCharacterDraft}
          onDraftPortrait={(file) => void uploadCharacterPortrait(file)}
          onCreate={() => void createCharacterFromDraft()}
          onLocalChange={(characterId, updates) =>
            setCharacters((current) =>
              current.map((character) =>
                character.id === characterId ? { ...character, ...updates } : character,
              ),
            )
          }
          onSave={(character) =>
            void updateCharacterById(character.id, {
              name: character.name,
              details: character.details,
              inventory: character.inventory,
              skills: character.skills,
              spells: character.spells,
            })
          }
          onPortraitFile={(characterId, file) => void uploadCharacterPortrait(file, characterId)}
          onClearPortrait={(characterId) => void updateCharacterById(characterId, { portrait: null })}
          onDelete={(characterId) => void deleteCharacterById(characterId)}
          {...desktopPanelControls(panel)}
        />
      );
    }

    if (panel === "textModel") {
      return (
        <TextModelPanel
          key={panel}
          settings={settings}
          setSettings={setSettings}
          localTextStatus={localTextStatus}
          {...desktopPanelControls(panel)}
        />
      );
    }

    if (panel === "story") {
      return (
        <StorySettingsPanel
          key={panel}
          settings={settings}
          setSettings={setSettings}
          {...desktopPanelControls(panel)}
        />
      );
    }

    if (panel === "images") {
      return (
        <ImageSettingsPanel
          key={panel}
          settings={settings}
          setSettings={setSettings}
          onImageGenerationEnabledChange={setImageGenerationEnabled}
          imageWorkerStatus={imageWorkerStatus}
          imageWorkerBusy={imageWorkerBusy}
          imageWorkerMessage={imageWorkerMessage}
          onStartImageWorker={() => void startImageWorker()}
          onOpenImageModelFolder={() => void openImageModelFolder()}
          {...desktopPanelControls(panel)}
        />
      );
    }

    if (panel === "voice") {
      return (
        <VoicePanel
          key={panel}
          settings={settings}
          setSettings={setSettings}
          {...desktopPanelControls(panel)}
        />
      );
    }

    if (panel === "localData") {
      return (
        <LocalDataPanel
          key={panel}
          clearing={clearingLocalData}
          onClear={() => void clearAllLocalData()}
          {...desktopPanelControls(panel)}
        />
      );
    }

    return <SupportPanel key={panel} {...desktopPanelControls(panel)} />;
  }

  return (
    <main className="flex h-dvh min-h-dvh flex-1 overflow-hidden bg-[#130d09] text-stone-100">
      {settings.rpgEnabled && settings.diceEnabled && (
        <DiceStage
          job={diceQueue[0] ?? null}
          onDone={(id) => setDiceQueue((queue) => queue.filter((entry) => entry.id !== id))}
          sound={settings.diceSound}
          volume={settings.diceVolume}
        />
      )}
      <section className="mx-auto flex h-dvh min-h-0 w-full max-w-7xl flex-1 flex-col px-3 pt-3 sm:px-4 md:px-8 md:pt-4">
        <header className="flex shrink-0 items-center justify-between gap-3 border-b border-stone-800/80 pb-3">
          <div className="flex min-w-0 items-center gap-3">
            <div className="min-w-0">
              <h1 className="truncate text-balance text-base font-semibold text-stone-100">
                {activeChat?.title || "Open Dungeon"}
              </h1>
              <p className="truncate text-xs text-stone-500">
                {settings.textProvider === "local"
                  ? `${
                      LOCAL_TEXT_MODELS.find((model) => model.id === settings.localTextModel)
                        ?.label ?? "Локальная модель"
                    } · на устройстве`
                  : `${settings.customModel || "Подключённый сервер"} · ваш сервер`}
              </p>
            </div>
          </div>
          <div className="flex shrink-0 items-center gap-2">
            <button
              type="button"
              aria-label="Открыть инструменты истории"
              onClick={() => setMobileToolsOpen(true)}
              className="inline-flex size-10 items-center justify-center rounded border border-stone-700 text-stone-300 hover:bg-stone-900 lg:hidden"
            >
              <Settings2 className="size-4" aria-hidden="true" />
            </button>
            <button
              type="button"
              onClick={() => setNewStoryOpen(true)}
              className="inline-flex h-10 items-center gap-2 rounded border border-stone-700 px-3 text-sm text-stone-300 hover:bg-stone-900"
            >
              <Plus className="size-4" aria-hidden="true" />
              <span className="hidden sm:inline">Новая история</span>
              <span className="sm:hidden">Новая</span>
            </button>
          </div>
        </header>

        <MobileChatBar
          chats={chats}
          selectedChatId={selectedChatId}
          disabled={libraryLoading || loadingChat}
          onSelect={(chatId) => void loadChat(chatId)}
          onDelete={(chatId) => void deleteChatById(chatId)}
        />

        <MobileToolsSheet
          open={mobileToolsOpen}
          activeTool={mobileTool}
          onActiveToolChange={setMobileTool}
          onClose={() => setMobileToolsOpen(false)}
          characters={characters}
          draft={characterDraft}
          creating={characterSaving}
          uploadingId={characterUploadingId}
          onDraftChange={setCharacterDraft}
          onDraftPortrait={(file) => void uploadCharacterPortrait(file)}
          onCreateCharacter={() => void createCharacterFromDraft()}
          onLocalCharacterChange={(characterId, updates) =>
            setCharacters((current) =>
              current.map((character) =>
                character.id === characterId ? { ...character, ...updates } : character,
              ),
            )
          }
          onSaveCharacter={(character) =>
            void updateCharacterById(character.id, {
              name: character.name,
              details: character.details,
              inventory: character.inventory,
              skills: character.skills,
              spells: character.spells,
            })
          }
          onPortraitFile={(characterId, file) => void uploadCharacterPortrait(file, characterId)}
          onClearPortrait={(characterId) => void updateCharacterById(characterId, { portrait: null })}
          onDeleteCharacter={(characterId) => void deleteCharacterById(characterId)}
          settings={settings}
          setSettings={setSettings}
          onImageGenerationEnabledChange={setImageGenerationEnabled}
          imageWorkerStatus={imageWorkerStatus}
          imageWorkerBusy={imageWorkerBusy}
          imageWorkerMessage={imageWorkerMessage}
          onStartImageWorker={() => void startImageWorker()}
          onOpenImageModelFolder={() => void openImageModelFolder()}
          localTextStatus={localTextStatus}
          clearingLocalData={clearingLocalData}
          onClearLocalData={() => void clearAllLocalData()}
        />

        {newStoryOpen && (
          <NewStoryDialog
            onClose={() => setNewStoryOpen(false)}
            onBegin={(options) => void beginStory(options)}
          />
        )}

        <div
          className={cn(
            "grid min-h-0 flex-1 overflow-hidden gap-4 py-3 lg:gap-6 lg:py-6",
            settings.rpgEnabled && heroRpg
              ? "lg:grid-cols-[300px_minmax(0,1fr)_340px]"
              : "lg:grid-cols-[minmax(0,1fr)_340px]",
          )}
        >
          {settings.rpgEnabled && heroRpg && (
            <aside className="hidden min-h-0 flex-col gap-2 overflow-y-auto pr-1 lg:flex">
              <CharacterSheet
                hero={heroRpg}
                name={characters[0]?.name || "Герой"}
                portrait={characters[0]?.portrait}
                items={items}
                onToggle={equipItem}
                busy={busy}
              />
              {journal.length > 0 && (
                <div className="space-y-1 rounded border border-amber-900/40 bg-amber-950/10 px-3 py-2 text-xs text-amber-100/90">
                  <div className="mb-1 font-medium uppercase tracking-wide text-amber-300/70">
                    Журнал
                  </div>
                  {journal.slice(-40).map((event) => {
                    const rollResult =
                      event.kind === "roll"
                        ? (event.data as { result?: RollResult } | undefined)?.result
                        : undefined;
                    return (
                      <div key={event.id} className="journal-in flex items-center gap-2">
                        {rollResult ? <DiceRollBadge result={rollResult} /> : null}
                        <span>{event.text}</span>
                      </div>
                    );
                  })}
                </div>
              )}
            </aside>
          )}
          <section className="flex h-full min-h-0 flex-col">
            <div className="mx-auto flex h-full min-h-0 w-full max-w-3xl flex-1 flex-col">
              {messages.length > 0 && (
                <div className="mb-2 flex shrink-0 justify-end">
                  <button
                    type="button"
                    onClick={() => setBookMode((value) => !value)}
                    className="inline-flex items-center gap-1.5 rounded-full border border-stone-700 px-3 py-1 text-xs text-stone-300 transition hover:border-amber-300 hover:text-amber-200"
                  >
                    <BookOpen className="size-3.5" aria-hidden="true" />
                    {bookMode ? "Лента" : "Книга"}
                  </button>
                </div>
              )}
              {bookMode ? (
                <BookReader messages={messages} />
              ) : (
              <div className="min-h-0 flex-1 space-y-8 overflow-y-auto overscroll-contain pr-1 pb-3 sm:space-y-10">
                {libraryLoading || loadingChat ? (
                  <StorySkeleton />
                ) : messages.length === 0 && !busy ? (
                  <div className="flex h-full flex-col items-center justify-center gap-5 text-center">
                    <div className="flex size-12 items-center justify-center rounded-xl border border-amber-200/20 bg-amber-200/10">
                      <BookOpen className="size-5 text-amber-200" aria-hidden="true" />
                    </div>
                    <div className="max-w-sm">
                      <p className="text-balance font-serif text-2xl text-stone-200">
                        Каждая история начинается с одной строки.
                      </p>
                      <p className="mt-2 text-pretty text-sm text-stone-500">
                        Начни историю и опиши, что ты делаешь — дальше рассказчик
                        подхватит, со сценами и всем остальным.
                      </p>
                    </div>
                    <button
                      type="button"
                      onClick={() => setNewStoryOpen(true)}
                      className="inline-flex h-10 items-center gap-2 rounded-lg bg-amber-200 px-4 text-sm font-medium text-stone-950 hover:bg-amber-100"
                    >
                      <Plus className="size-4" aria-hidden="true" />
                      Начать новую историю
                    </button>
                  </div>
                ) : (
                  messages.map((message) => (
                    <article key={message.id} className="group">
                      {editingId === message.id ? (
                        <MessageEditor
                          message={message}
                          value={editDraft}
                          onChange={setEditDraft}
                          onSave={() => void saveEdit()}
                          onCancel={() => setEditingId("")}
                        />
                      ) : message.role === "user" ? (
                        <div className="ml-auto max-w-[92%] sm:max-w-2xl">
                          <div className="rounded-2xl rounded-br-md border border-stone-800/70 bg-stone-900/60 px-4 py-3 text-sm leading-6 text-stone-300">
                            <p className="text-pretty whitespace-pre-wrap">{message.content}</p>
                            {!!message.attachments?.length && (
                              <AttachmentStrip attachments={message.attachments} className="mt-3" />
                            )}
                          </div>
                          <MessageActions
                            align="end"
                            disabled={busy}
                            onEdit={() => startEditing(message)}
                          />
                        </div>
                      ) : (
                        <div
                          className={cn(
                            "font-serif text-stone-100",
                            storyProseClassName(settings.proseSize),
                          )}
                        >
                          <p className="text-pretty whitespace-pre-wrap">
                            {renderStoryEmphasis(stripModelMarkup(message.content))}
                          </p>
                          {(message.generatedImage ||
                            (settings.imageGenerationEnabled && message.imageRequest?.needed)) && (
                            <ImageBeat
                              message={message}
                              status={imageStatus[message.id]}
                              onRetry={() =>
                                message.imageRequest?.prompt &&
                                requestGeneratedImage(
                                  message.id,
                                  message.imageRequest.prompt,
                                  referencesForImage(
                                    message.imageRequest.characterIds,
                                    lastUserAttachments,
                                  ),
                                  message.imageRequest,
                                )
                              }
                            />
                          )}
                          <MessageActions
                            align="start"
                            disabled={busy}
                            onEdit={() => startEditing(message)}
                            onSpeak={() => void speakText(message.id, message.content)}
                            speaking={speakingId === message.id}
                          />
                        </div>
                      )}
                    </article>
                  ))
                )}

                {busy && (
                  <div className="flex items-center gap-3 font-serif text-base italic text-stone-500">
                    <Loader2 className="size-4 animate-spin" aria-hidden="true" />
                    Формируется следующий отрывок…
                    <button
                      type="button"
                      onClick={stopTurn}
                      className="ml-1 inline-flex items-center gap-1 rounded border border-stone-700 px-2 py-0.5 text-xs not-italic text-stone-300 transition hover:border-red-500/70 hover:text-red-300"
                    >
                      <X className="size-3.5" aria-hidden="true" />
                      Прервать
                    </button>
                  </div>
                )}
                <div ref={endRef} />
              </div>
              )}

              {settings.rpgEnabled && (heroRpg || items.length > 0) && (
                <div className="mb-2 shrink-0 space-y-2 lg:hidden">
                  {heroRpg && <HudBar hero={heroRpg} name={characters[0]?.name || "Герой"} />}
                  {items.length > 0 && (
                    <InventoryPanel items={items} onToggle={equipItem} disabled={busy} />
                  )}
                </div>
              )}
              {settings.rpgEnabled && journal.length > 0 && (
                <div className="mb-2 max-h-28 shrink-0 space-y-1 overflow-y-auto rounded border border-amber-900/40 bg-amber-950/10 px-3 py-2 text-xs text-amber-100/90 lg:hidden">
                  <div className="mb-1 font-medium uppercase tracking-wide text-amber-300/70">
                    Журнал
                  </div>
                  {journal.slice(-8).map((event) => {
                    const rollResult =
                      event.kind === "roll"
                        ? (event.data as { result?: RollResult } | undefined)?.result
                        : undefined;
                    return (
                      <div key={event.id} className="journal-in flex items-center gap-2">
                        {rollResult ? <DiceRollBadge result={rollResult} /> : null}
                        <span>{event.text}</span>
                      </div>
                    );
                  })}
                </div>
              )}
              {suggestedActions.length > 0 && !busy && (
                <div className="flex shrink-0 flex-wrap gap-2 px-1 pb-2">
                  {suggestedActions.map((action, index) => (
                    <button
                      key={`${index}-${action.label}`}
                      type="button"
                      onClick={() => void playInput(action.label)}
                      className="inline-flex max-w-[18rem] items-center gap-1.5 rounded-full border border-amber-900/60 bg-amber-950/20 px-3 py-1.5 text-sm text-amber-100 transition hover:border-amber-300 hover:bg-amber-900/30"
                    >
                      {action.emoji && <span aria-hidden="true">{action.emoji}</span>}
                      <span className="truncate">{action.label}</span>
                    </button>
                  ))}
                </div>
              )}

              <form
                onSubmit={submitTurn}
                className="shrink-0 border-t border-stone-800 bg-[#130d09] pb-[calc(0.75rem+env(safe-area-inset-bottom))] pt-3 sm:pt-4 lg:pb-0"
              >
                {error && (
                  <div className="mb-3 rounded border border-red-900/80 bg-red-950/30 px-3 py-2 text-sm text-red-200">
                    {error}
                  </div>
                )}

                {!!attachments.length && (
                  <AttachmentStrip
                    attachments={attachments}
                    className="mb-3"
                    onRemove={(id) =>
                      setAttachments((current) => current.filter((item) => item.id !== id))
                    }
                  />
                )}

                {messages.length > 0 && (
                  <div className="mb-2 flex flex-wrap items-center gap-2">
                    <StoryActionButton
                      icon={ChevronRight}
                      label="Продолжить"
                      title="Пусть рассказчик продолжит без тебя"
                      disabled={busy || !selectedChatId}
                      onClick={() => void continueStory()}
                    />
                    <StoryActionButton
                      icon={RotateCcw}
                      label="Повторить"
                      title="Перегенерировать последний отрывок"
                      disabled={busy || !selectedChatId}
                      onClick={() => void retryLastTurn()}
                    />
                    <StoryActionButton
                      icon={Eraser}
                      label="Стереть"
                      title="Убрать последний обмен"
                      disabled={busy || !selectedChatId}
                      onClick={() => void eraseLastTurn()}
                    />
                  </div>
                )}

                <div className="rounded-2xl border border-stone-700/80 bg-stone-950 focus-within:border-amber-300/60">
                  <input
                    id="reference-images"
                    name="reference-images"
                    ref={fileInputRef}
                    type="file"
                    accept="image/png,image/jpeg,image/webp"
                    multiple
                    className="hidden"
                    onChange={(event) => handleFiles(event.target.files)}
                  />
                  <textarea
                    id="story-input"
                    name="story-input"
                    value={input}
                    onChange={(event) => setInput(event.target.value)}
                    onKeyDown={(event) => {
                      if (event.key === "Enter" && (event.metaKey || event.ctrlKey)) {
                        event.currentTarget.form?.requestSubmit();
                      }
                    }}
                    rows={2}
                    placeholder={
                      INPUT_MODES.find((m) => m.value === inputMode)?.placeholder ?? "Что ты делаешь?"
                    }
                    className="max-h-40 min-h-16 w-full resize-none bg-transparent px-4 pb-1 pt-3.5 text-base text-stone-100 outline-none placeholder:text-stone-600 disabled:cursor-not-allowed disabled:text-stone-600 sm:min-h-20"
                    disabled={libraryLoading || loadingChat}
                  />
                  <div className="flex items-center justify-between gap-2 px-2.5 pb-2.5">
                    <div className="flex items-center gap-1">
                      <div className="flex rounded-lg border border-stone-800 bg-stone-950 p-0.5">
                        {INPUT_MODES.map((m) => (
                          <button
                            key={m.value}
                            type="button"
                            aria-pressed={inputMode === m.value}
                            onClick={() => setInputMode(m.value)}
                            className={cn(
                              "rounded-md px-2.5 py-1 text-xs font-medium text-stone-400 hover:text-stone-200",
                              inputMode === m.value && "bg-stone-800 text-stone-100",
                            )}
                          >
                            {m.label}
                          </button>
                        ))}
                      </div>
                      <button
                        type="button"
                        aria-label="Прикрепить референсы"
                        onClick={() => fileInputRef.current?.click()}
                        className="flex size-9 shrink-0 items-center justify-center rounded-lg text-stone-400 hover:bg-stone-900 hover:text-stone-200 disabled:cursor-not-allowed disabled:text-stone-600"
                        disabled={uploading || libraryLoading}
                      >
                        {uploading ? (
                          <Loader2 className="size-4 animate-spin" aria-hidden="true" />
                        ) : (
                          <Paperclip className="size-4" aria-hidden="true" />
                        )}
                      </button>
                      <MicButton
                        onTranscript={(text) =>
                          setInput((current) => (current ? `${current} ${text}` : text))
                        }
                        disabled={busy || libraryLoading}
                      />
                    </div>
                    <div className="flex items-center gap-3">
                      <span className="hidden text-xs text-stone-600 sm:inline">⌘↵ отправить</span>
                      <button
                        type="submit"
                        aria-label="Отправить"
                        disabled={busy || !input.trim() || !selectedChatId || libraryLoading}
                        className="inline-flex h-9 shrink-0 items-center justify-center gap-2 rounded-lg bg-amber-200 px-4 text-sm font-medium text-stone-950 hover:bg-amber-100 disabled:cursor-not-allowed disabled:bg-stone-800 disabled:text-stone-500"
                      >
                        <Send className="size-4" aria-hidden="true" />
                        <span className="hidden sm:inline">Отправить</span>
                      </button>
                    </div>
                  </div>
                </div>
              </form>
            </div>
          </section>

          <aside className="hidden min-h-0 border-l border-stone-800 pl-6 lg:block">
            <div
              className={cn(
                "sticky top-4 overflow-y-auto pr-1 pb-4",
                activeDesktopPanel
                  ? "flex h-[calc(100dvh-2rem)] min-h-0 flex-col"
                  : "max-h-[calc(100dvh-2rem)] space-y-2",
              )}
            >
              {orderedDesktopPanels.map((panel) => renderDesktopPanel(panel))}
              {!activeDesktopPanel && (
                <>
                  <FontSizeSlider
                    settings={settings}
                    setSettings={setSettings}
                    idPrefix="desktop-rail"
                  />
                  <ResponseLengthControl
                    settings={settings}
                    setSettings={setSettings}
                    idPrefix="desktop-rail"
                  />
                </>
              )}
            </div>
          </aside>
        </div>
      </section>
    </main>
  );
}

function NewStoryDialog({
  onClose,
  onBegin,
}: {
  onClose: () => void;
  onBegin: (options: {
    title: string;
    world: string;
    opening: { mode: "narrator"; hint: string } | { mode: "self"; text: string };
  }) => void;
}) {
  const [presetId, setPresetId] = useState<StoryPresetId>("fantasy");
  const [name, setName] = useState("");
  const [role, setRole] = useState("");
  const [customWorld, setCustomWorld] = useState("");
  const [openingMode, setOpeningMode] = useState<"narrator" | "self">("narrator");
  const [openingHint, setOpeningHint] = useState("");
  const [openingText, setOpeningText] = useState("");

  const isCustom = presetId === "custom";
  const preset = STORY_PRESETS.find((item) => item.id === presetId) ?? STORY_PRESETS[0];
  const settingReady = !isCustom || customWorld.trim().length > 0;
  const openingReady = openingMode === "narrator" || openingText.trim().length > 0;
  const canBegin = settingReady && openingReady;

  function begin() {
    const opening =
      openingMode === "self"
        ? ({ mode: "self", text: openingText.trim() } as const)
        : ({ mode: "narrator", hint: openingHint.trim() } as const);

    if (isCustom) {
      const world = customWorld.trim();
      onBegin({ world, title: titleFromInput(world), opening });
      return;
    }

    const persona = role.trim() || preset.rolePlaceholder;
    const protagonist = name.trim() ? `${name.trim()}, ${persona}` : persona;
    onBegin({
      world: `${preset.seed} You are ${protagonist}.`,
      title: titleFromInput(
        name.trim() ? `${name.trim()} · ${preset.label}` : `${preset.label} · ${persona}`,
      ),
      opening,
    });
  }

  return (
    <Dialog.Root open onOpenChange={(open) => !open && onClose()}>
      <Dialog.Portal>
        <Dialog.Overlay className="fixed inset-0 z-40 bg-stone-950/80" />
        <Dialog.Content
          className="fixed left-1/2 top-1/2 z-50 max-h-[90dvh] w-[min(calc(100vw-2rem),580px)] -translate-x-1/2 -translate-y-1/2 overflow-y-auto rounded-xl border border-stone-700 bg-[#130d09] p-5 shadow-xl"
          onOpenAutoFocus={(event) => event.preventDefault()}
        >
          <div className="flex items-start justify-between gap-3">
            <div>
              <Dialog.Title className="text-balance text-base font-semibold text-stone-100">
                Новая история
              </Dialog.Title>
              <Dialog.Description className="mt-1 text-pretty text-sm text-stone-500">
                Выбери сеттинг, скажи кто ты, и выбери как начнётся история.
              </Dialog.Description>
            </div>
            <Dialog.Close asChild>
              <button
                type="button"
                aria-label="Закрыть"
                className="flex size-8 shrink-0 items-center justify-center rounded-lg border border-stone-800 text-stone-400 hover:bg-stone-900 hover:text-stone-100"
              >
                <X className="size-4" aria-hidden="true" />
              </button>
            </Dialog.Close>
          </div>

          <div className="mt-4 grid grid-cols-2 gap-2 sm:grid-cols-3">
            {[
              ...STORY_PRESETS,
              { id: "custom" as const, label: "Свой мир", flavor: "Опиши своё начало" },
            ].map((item) => {
              const selected = item.id === presetId;
              return (
                <button
                  key={item.id}
                  type="button"
                  aria-pressed={selected}
                  onClick={() => setPresetId(item.id)}
                  className={cn(
                    "rounded-lg border px-3 py-2.5 text-left",
                    selected
                      ? "border-amber-200/70 bg-stone-900"
                      : "border-stone-800 bg-stone-950 hover:bg-stone-900",
                  )}
                >
                  <span className="block text-sm font-medium text-stone-200">{item.label}</span>
                  <span className="mt-0.5 block text-xs text-stone-500">{item.flavor}</span>
                </button>
              );
            })}
          </div>

          {isCustom ? (
            <label className="mt-4 block">
              <span className="mb-2 block text-xs font-medium uppercase text-stone-500">
                О чём эта история?
              </span>
              <textarea
                id="new-story-custom"
                name="new-story-custom"
                value={customWorld}
                onChange={(event) => setCustomWorld(event.target.value)}
                rows={4}
                placeholder="You are a lighthouse keeper on a coast where the fog has started whispering back. Last night the light went out on its own..."
                className="w-full resize-none rounded-lg border border-stone-800 bg-stone-950 px-3 py-2 text-sm text-stone-200 outline-none focus:border-amber-300"
              />
            </label>
          ) : (
            <div className="mt-4 grid gap-3 sm:grid-cols-2">
              <label className="block">
                <span className="mb-2 block text-xs font-medium uppercase text-stone-500">
                  Кто ты?
                </span>
                <input
                  id="new-story-role"
                  name="new-story-role"
                  value={role}
                  onChange={(event) => setRole(event.target.value)}
                  placeholder={preset.rolePlaceholder}
                  className="w-full rounded-lg border border-stone-800 bg-stone-950 px-3 py-2 text-sm text-stone-200 outline-none focus:border-amber-300"
                />
              </label>
              <label className="block">
                <span className="mb-2 block text-xs font-medium uppercase text-stone-500">
                  Имя <span className="normal-case text-stone-600">(необязательно)</span>
                </span>
                <input
                  id="new-story-name"
                  name="new-story-name"
                  value={name}
                  onChange={(event) => setName(event.target.value)}
                  placeholder="Alice Fordring"
                  className="w-full rounded-lg border border-stone-800 bg-stone-950 px-3 py-2 text-sm text-stone-200 outline-none focus:border-amber-300"
                />
              </label>
            </div>
          )}

          <div className="mt-5 border-t border-stone-800 pt-4">
            <span className="mb-2 block text-xs font-medium uppercase text-stone-500">
              Начало
            </span>
            <div className="grid grid-cols-2 gap-2">
              {(
                [
                  { value: "narrator", label: "Рассказчик задаёт сцену" },
                  { value: "self", label: "Написать начало самому" },
                ] as const
              ).map((option) => {
                const selected = option.value === openingMode;
                return (
                  <button
                    key={option.value}
                    type="button"
                    aria-pressed={selected}
                    onClick={() => setOpeningMode(option.value)}
                    className={cn(
                      "rounded-lg border px-3 py-2 text-sm",
                      selected
                        ? "border-amber-200/70 bg-stone-900 text-stone-100"
                        : "border-stone-800 bg-stone-950 text-stone-300 hover:bg-stone-900",
                    )}
                  >
                    {option.label}
                  </button>
                );
              })}
            </div>

            {openingMode === "narrator" ? (
              <label className="mt-3 block">
                <span className="mb-1.5 block text-xs font-medium uppercase text-stone-500">
                  Подсказка начала <span className="normal-case text-stone-600">(необязательно)</span>
                </span>
                <textarea
                  id="new-story-opening-hint"
                  name="new-story-opening-hint"
                  value={openingHint}
                  onChange={(event) => setOpeningHint(event.target.value)}
                  rows={2}
                  placeholder="напр. начни с моего пробуждения в камере без памяти о прошлой ночи"
                  className="w-full resize-none rounded-lg border border-stone-800 bg-stone-950 px-3 py-2 text-sm text-stone-200 outline-none focus:border-amber-300"
                />
              </label>
            ) : (
              <label className="mt-3 block">
                <span className="mb-1.5 block text-xs font-medium uppercase text-stone-500">
                  Твой вводный отрывок
                </span>
                <textarea
                  id="new-story-opening-text"
                  name="new-story-opening-text"
                  value={openingText}
                  onChange={(event) => setOpeningText(event.target.value)}
                  rows={4}
                  placeholder="Rain hammers the tin roof of the bus shelter. You pull your coat tighter and check the time again. The 11:40 is twenty minutes late, and the only other person here keeps watching you..."
                  className="w-full resize-none rounded-lg border border-stone-800 bg-stone-950 px-3 py-2 text-sm text-stone-200 outline-none focus:border-amber-300"
                />
                <span className="mt-1.5 block text-xs text-stone-600">
                  This becomes the story&apos;s first passage exactly as written. Take your
                  first action and the narrator continues from there.
                </span>
              </label>
            )}
          </div>

          <div className="mt-5 flex items-center justify-end gap-2">
            <Dialog.Close asChild>
              <button
                type="button"
                className="rounded-lg border border-stone-700 px-3 py-2 text-sm text-stone-300 hover:bg-stone-900"
              >
                Отмена
              </button>
            </Dialog.Close>
            <button
              type="button"
              disabled={!canBegin}
              onClick={begin}
              className="inline-flex items-center gap-2 rounded-lg bg-amber-200 px-4 py-2 text-sm font-medium text-stone-950 hover:bg-amber-100 disabled:cursor-not-allowed disabled:bg-stone-800 disabled:text-stone-500"
            >
              <Sparkles className="size-4" aria-hidden="true" />
              Начать историю
            </button>
          </div>
        </Dialog.Content>
      </Dialog.Portal>
    </Dialog.Root>
  );
}

function MobileToolsSheet({
  open,
  activeTool,
  onActiveToolChange,
  onClose,
  characters,
  draft,
  creating,
  uploadingId,
  onDraftChange,
  onDraftPortrait,
  onCreateCharacter,
  onLocalCharacterChange,
  onSaveCharacter,
  onPortraitFile,
  onClearPortrait,
  onDeleteCharacter,
  settings,
  setSettings,
  onImageGenerationEnabledChange,
  imageWorkerStatus,
  imageWorkerBusy,
  imageWorkerMessage,
  onStartImageWorker,
  onOpenImageModelFolder,
  localTextStatus,
  clearingLocalData,
  onClearLocalData,
}: {
  open: boolean;
  activeTool: MobileTool;
  onActiveToolChange: (tool: MobileTool) => void;
  onClose: () => void;
  characters: StoryCharacter[];
  draft: CharacterDraft;
  creating: boolean;
  uploadingId: string;
  onDraftChange: (updater: CharacterDraft | ((current: CharacterDraft) => CharacterDraft)) => void;
  onDraftPortrait: (file: File) => void;
  onCreateCharacter: () => void;
  onLocalCharacterChange: (
    characterId: string,
    updates: Partial<Pick<StoryCharacter, "name" | "details" | "inventory" | "skills" | "spells">>,
  ) => void;
  onSaveCharacter: (character: StoryCharacter) => void;
  onPortraitFile: (characterId: string, file: File) => void;
  onClearPortrait: (characterId: string) => void;
  onDeleteCharacter: (characterId: string) => void;
  settings: StorySettings;
  setSettings: Dispatch<SetStateAction<StorySettings>>;
  onImageGenerationEnabledChange: (enabled: boolean) => void;
  imageWorkerStatus: ImageWorkerStatus | null;
  imageWorkerBusy: boolean;
  imageWorkerMessage: string;
  onStartImageWorker: () => void;
  onOpenImageModelFolder: () => void;
  localTextStatus: LocalTextStatus | null;
  clearingLocalData: boolean;
  onClearLocalData: () => void;
}) {
  if (!open) {
    return null;
  }

  const tools: Array<{ value: MobileTool; label: string }> = [
    { value: "characters", label: "Chars" },
    { value: "story", label: "История" },
    { value: "images", label: "Изображения" },
    { value: "data", label: "Data" },
  ];

  return (
    <div className="fixed inset-0 z-30 lg:hidden">
      <button
        type="button"
        aria-label="Закрыть инструменты истории"
        onClick={onClose}
        className="absolute inset-0 bg-stone-950/70"
      />
      <section
        role="dialog"
        aria-modal="true"
        aria-label="Инструменты истории"
        className="absolute inset-x-0 bottom-0 flex max-h-[88dvh] flex-col rounded-t-2xl border border-stone-700 bg-[#130d09] shadow-2xl"
      >
        <div className="flex shrink-0 items-center justify-between gap-3 border-b border-stone-800 px-4 py-3">
          <PanelTitle icon={Settings2} title="Инструменты" />
          <button
            type="button"
            aria-label="Закрыть инструменты истории"
            onClick={onClose}
            className="flex size-9 items-center justify-center rounded border border-stone-800 text-stone-400 hover:bg-stone-900 hover:text-stone-100"
          >
            <X className="size-4" aria-hidden="true" />
          </button>
        </div>

        <div className="grid shrink-0 grid-cols-4 gap-1 border-b border-stone-800 bg-stone-950/40 p-2">
          {tools.map((tool) => {
            const selected = tool.value === activeTool;
            return (
              <button
                key={tool.value}
                type="button"
                aria-pressed={selected}
                onClick={() => onActiveToolChange(tool.value)}
                className={cn(
                  "h-10 rounded text-sm text-stone-400 hover:bg-stone-900",
                  selected && "bg-stone-800 text-stone-100",
                )}
              >
                {tool.label}
              </button>
            );
          })}
        </div>

        <div className="min-h-0 flex-1 overflow-y-auto px-4 py-4 pb-[calc(1rem+env(safe-area-inset-bottom))]">
          {activeTool === "characters" && (
            <CharacterPanel
              settings={settings}
              setSettings={setSettings}
              characters={characters}
              draft={draft}
              creating={creating}
              uploadingId={uploadingId}
              onDraftChange={onDraftChange}
              onDraftPortrait={onDraftPortrait}
              onCreate={onCreateCharacter}
              onLocalChange={onLocalCharacterChange}
              onSave={onSaveCharacter}
              onPortraitFile={onPortraitFile}
              onClearPortrait={onClearPortrait}
              onDelete={onDeleteCharacter}
              compact
            />
          )}

          {activeTool === "story" && (
            <div className="space-y-6">
              <TextModelPanel
                settings={settings}
                setSettings={setSettings}
                localTextStatus={localTextStatus}
                compact
              />
              <StorySettingsPanel settings={settings} setSettings={setSettings} compact />
            </div>
          )}

          {activeTool === "images" && (
            <ImageSettingsPanel
              settings={settings}
              setSettings={setSettings}
              onImageGenerationEnabledChange={onImageGenerationEnabledChange}
              imageWorkerStatus={imageWorkerStatus}
              imageWorkerBusy={imageWorkerBusy}
              imageWorkerMessage={imageWorkerMessage}
              onStartImageWorker={onStartImageWorker}
              onOpenImageModelFolder={onOpenImageModelFolder}
              compact
            />
          )}

          {activeTool === "data" && (
            <LocalDataPanel clearing={clearingLocalData} onClear={onClearLocalData} compact />
          )}
        </div>
      </section>
    </div>
  );
}

function MobileChatBar({
  chats,
  selectedChatId,
  disabled,
  onSelect,
  onDelete,
}: {
  chats: StoryChatSummary[];
  selectedChatId: string;
  disabled: boolean;
  onSelect: (chatId: string) => void;
  onDelete: (chatId: string) => void;
}) {
  if (!chats.length) {
    return null;
  }

  const selectedChat = chats.find((chat) => chat.id === selectedChatId);

  return (
    <div className="flex items-center gap-2 border-b border-stone-800 py-3 lg:hidden">
      <select
        id="mobile-chat-select"
        name="mobile-chat-select"
        value={selectedChatId}
        onChange={(event) => onSelect(event.target.value)}
        disabled={disabled}
        className="min-w-0 flex-1 rounded border border-stone-800 bg-stone-950 px-3 py-2 text-sm text-stone-200 outline-none focus:border-amber-300"
      >
        {chats.map((chat) => (
          <option key={chat.id} value={chat.id}>
            {chat.title}
          </option>
        ))}
      </select>
      {selectedChat && (
        <DeleteChatDialog chat={selectedChat} onConfirm={() => onDelete(selectedChat.id)}>
          <button
            type="button"
            aria-label="Удалить текущую историю"
            className="flex size-10 items-center justify-center rounded border border-stone-800 text-stone-400 hover:bg-stone-900 hover:text-red-200"
          >
            <Trash2 className="size-4" aria-hidden="true" />
          </button>
        </DeleteChatDialog>
      )}
    </div>
  );
}

function ChatLibrary({
  chats,
  selectedChatId,
  loading,
  onCreate,
  onSelect,
  onDelete,
  open,
  onOpenChange,
  divided,
  fillAvailable = false,
}: {
  chats: StoryChatSummary[];
  selectedChatId: string;
  loading: boolean;
  onCreate: () => void;
  onSelect: (chatId: string) => void;
  onDelete: (chatId: string) => void;
  fillAvailable?: boolean;
} & PanelControlProps) {
  const chatListClassName = cn(
    "space-y-2 overflow-y-auto pr-1",
    fillAvailable && "min-h-0 flex-1",
  );
  const chatListStyle = fillAvailable ? undefined : { maxHeight: "clamp(4rem, 10dvh, 9rem)" };

  return (
    <PanelSection
      icon={Library}
      iconSrc={SIDEBAR_ICONS.chats}
      title="Истории"
      defaultOpen
      open={open}
      onOpenChange={onOpenChange}
      divided={divided ?? false}
      fill={fillAvailable}
      action={
        <button
          type="button"
          aria-label="Создать новую историю"
          onClick={onCreate}
          className="flex size-10 shrink-0 items-center justify-center rounded border border-stone-800 text-stone-300 hover:bg-stone-900"
        >
          <Plus className="size-4" aria-hidden="true" />
        </button>
      }
    >

      {loading ? (
        <div className={chatListClassName} style={chatListStyle}>
          <div className="h-16 rounded border border-stone-800 bg-stone-950" />
          <div className="h-16 rounded border border-stone-800 bg-stone-950" />
        </div>
      ) : (
        <div className={chatListClassName} style={chatListStyle}>
          {chats.map((chat) => {
            const selected = chat.id === selectedChatId;
            return (
              <div
                key={chat.id}
                className={cn(
                  "grid grid-cols-[minmax(0,1fr)_auto] items-stretch rounded border border-stone-800 bg-stone-950/80",
                  selected && "border-amber-200/70 bg-stone-900/70",
                )}
              >
                <button
                  type="button"
                  onClick={() => onSelect(chat.id)}
                  className="min-w-0 px-3 py-2 text-left"
                >
                  <span className="block truncate text-sm font-medium text-stone-200">
                    {chat.title}
                  </span>
                  <span className="mt-1 block truncate text-xs tabular-nums text-stone-500">
                    {chat.messageCount} {chat.messageCount === 1 ? "message" : "messages"} ·{" "}
                    {formatChatDate(chat.updatedAt)}
                  </span>
                  {chat.lastMessagePreview && (
                    <span className="mt-1 block truncate text-xs text-stone-600">
                      {chat.lastMessagePreview}
                    </span>
                  )}
                </button>
                <DeleteChatDialog chat={chat} onConfirm={() => onDelete(chat.id)}>
                  <button
                    type="button"
                    aria-label={`Delete ${chat.title}`}
                    className="flex w-10 items-center justify-center rounded-r text-stone-500 hover:bg-stone-900 hover:text-red-200"
                  >
                    <Trash2 className="size-4" aria-hidden="true" />
                  </button>
                </DeleteChatDialog>
              </div>
            );
          })}

          {!chats.length && (
            <button
              type="button"
              onClick={onCreate}
              className="w-full rounded border border-stone-800 bg-stone-950 px-3 py-4 text-left text-sm text-stone-400 hover:bg-stone-900"
            >
              Начать историю
            </button>
          )}
        </div>
      )}
    </PanelSection>
  );
}

function CharacterPanel({
  settings,
  setSettings,
  characters,
  draft,
  creating,
  uploadingId,
  onDraftChange,
  onDraftPortrait,
  onCreate,
  onLocalChange,
  onSave,
  onPortraitFile,
  onClearPortrait,
  onDelete,
  compact = false,
  open,
  onOpenChange,
  divided,
}: {
  settings: StorySettings;
  setSettings: Dispatch<SetStateAction<StorySettings>>;
  characters: StoryCharacter[];
  draft: CharacterDraft;
  creating: boolean;
  uploadingId: string;
  onDraftChange: (updater: CharacterDraft | ((current: CharacterDraft) => CharacterDraft)) => void;
  onDraftPortrait: (file: File) => void;
  onCreate: () => void;
  onLocalChange: (
    characterId: string,
    updates: Partial<Pick<StoryCharacter, "name" | "details" | "inventory" | "skills" | "spells">>,
  ) => void;
  onSave: (character: StoryCharacter) => void;
  onPortraitFile: (characterId: string, file: File) => void;
  onClearPortrait: (characterId: string) => void;
  onDelete: (characterId: string) => void;
  compact?: boolean;
} & PanelControlProps) {
  return (
    <PanelSection
      icon={UserRound}
      iconSrc={SIDEBAR_ICONS.characters}
      title="Персонажи"
      compact={compact}
      defaultOpen={compact}
      open={open}
      onOpenChange={onOpenChange}
      divided={divided}
    >
      <label className="block">
        <span className="mb-2 block text-xs font-medium uppercase text-stone-500">
          Промпт рассказчика
        </span>
        <textarea
          id={`${compact ? "mobile" : "desktop"}-narrator-prompt`}
          name={`${compact ? "mobile" : "desktop"}-narrator-prompt`}
          value={settings.narratorPrompt}
          onChange={(event) =>
            setSettings((current) => ({ ...current, narratorPrompt: event.target.value }))
          }
          rows={compact ? 6 : 10}
          spellCheck={false}
          className="w-full resize-y rounded border border-stone-800 bg-stone-950 px-3 py-2 font-mono text-xs leading-relaxed text-stone-200 outline-none focus:border-amber-300"
        />
        <span className="mt-1 block text-[11px] text-stone-600">
          Системный промпт рассказчика. Пусто — встроенный по умолчанию.
        </span>
      </label>
      <div className="space-y-3 rounded border border-stone-800 bg-stone-950/70 p-3">
        <label className="block">
          <span className="mb-1 block text-xs font-medium uppercase text-stone-500">Имя</span>
          <input
            id="new-character-name"
            name="new-character-name"
            value={draft.name}
            onChange={(event) =>
              onDraftChange((current) => ({ ...current, name: event.target.value }))
            }
            className="w-full rounded border border-stone-800 bg-stone-950 px-3 py-2 text-sm text-stone-200 outline-none focus:border-amber-300"
            placeholder="Valerie Maroto"
          />
        </label>

        <label className="block">
          <span className="mb-1 block text-xs font-medium uppercase text-stone-500">Детали</span>
          <textarea
            id="new-character-details"
            name="new-character-details"
            value={draft.details}
            onChange={(event) =>
              onDraftChange((current) => ({ ...current, details: event.target.value }))
            }
            rows={3}
            className="w-full resize-none rounded border border-stone-800 bg-stone-950 px-3 py-2 text-sm text-stone-200 outline-none focus:border-amber-300"
            placeholder="Короткие чёрные волосы, пацанка, сухой юмор..."
          />
        </label>

        <div className="grid gap-2">
          <label className="block">
            <span className="mb-1 flex items-center gap-1.5 text-xs font-medium uppercase text-stone-500">
              <Backpack className="size-3.5" aria-hidden="true" />
              Инвентарь
            </span>
            <textarea
              id="new-character-inventory"
              name="new-character-inventory"
              value={draft.inventory}
              onChange={(event) =>
                onDraftChange((current) => ({ ...current, inventory: event.target.value }))
              }
              rows={2}
              className="w-full resize-none rounded border border-stone-800 bg-stone-950 px-3 py-2 text-sm text-stone-200 outline-none focus:border-amber-300"
              placeholder="Железный кинжал, фонарь, 12 серебра..."
            />
          </label>
          <div className="grid gap-2 sm:grid-cols-2">
            <label className="block">
              <span className="mb-1 flex items-center gap-1.5 text-xs font-medium uppercase text-stone-500">
                <Sparkles className="size-3.5" aria-hidden="true" />
                Навыки
              </span>
              <textarea
                id="new-character-skills"
                name="new-character-skills"
                value={draft.skills}
                onChange={(event) =>
                  onDraftChange((current) => ({ ...current, skills: event.target.value }))
                }
                rows={2}
                className="w-full resize-none rounded border border-stone-800 bg-stone-950 px-3 py-2 text-sm text-stone-200 outline-none focus:border-amber-300"
                placeholder="Взлом замков, травничество..."
              />
            </label>
            <label className="block">
              <span className="mb-1 flex items-center gap-1.5 text-xs font-medium uppercase text-stone-500">
                <WandSparkles className="size-3.5" aria-hidden="true" />
                Заклинания
              </span>
              <textarea
                id="new-character-spells"
                name="new-character-spells"
                value={draft.spells}
                onChange={(event) =>
                  onDraftChange((current) => ({ ...current, spells: event.target.value }))
                }
                rows={2}
                className="w-full resize-none rounded border border-stone-800 bg-stone-950 px-3 py-2 text-sm text-stone-200 outline-none focus:border-amber-300"
                placeholder="Починка, рука мага..."
              />
            </label>
          </div>
        </div>

        {draft.portrait && (
          <div className="flex items-center gap-2 rounded border border-stone-800 bg-stone-950 p-2">
            {/* eslint-disable-next-line @next/next/no-img-element */}
            <img src={draft.portrait.url} alt="" className="size-12 rounded object-cover" />
            <span className="min-w-0 flex-1 truncate text-xs text-stone-400">
              {draft.portrait.name}
            </span>
            <button
              type="button"
              aria-label="Убрать черновой портрет"
              onClick={() => onDraftChange((current) => ({ ...current, portrait: undefined }))}
              className="flex size-7 items-center justify-center rounded text-stone-500 hover:bg-stone-900 hover:text-stone-100"
            >
              <X className="size-4" aria-hidden="true" />
            </button>
          </div>
        )}

        <div className="flex items-center gap-2">
          <input
            id="new-character-picture"
            name="new-character-picture"
            type="file"
            accept="image/png,image/jpeg,image/webp"
            className="hidden"
            onChange={(event) => {
              const file = event.currentTarget.files?.[0];
              if (file) {
                onDraftPortrait(file);
              }
              event.currentTarget.value = "";
            }}
          />
          <label
            htmlFor="new-character-picture"
            className="inline-flex h-9 cursor-pointer items-center gap-2 rounded border border-stone-700 px-3 text-sm text-stone-300 hover:bg-stone-900"
          >
            {uploadingId === "draft" ? (
              <Loader2 className="size-4 animate-spin" aria-hidden="true" />
            ) : (
              <ImagePlus className="size-4" aria-hidden="true" />
            )}
            Picture
          </label>

          <button
            type="button"
            onClick={onCreate}
            disabled={!draft.name.trim() || creating || uploadingId === "draft"}
            className="ml-auto inline-flex h-9 items-center gap-2 rounded bg-amber-200 px-3 text-sm font-medium text-stone-950 hover:bg-amber-100 disabled:cursor-not-allowed disabled:bg-stone-800 disabled:text-stone-500"
          >
            {creating ? (
              <Loader2 className="size-4 animate-spin" aria-hidden="true" />
            ) : (
              <Plus className="size-4" aria-hidden="true" />
            )}
            Add
          </button>
        </div>
      </div>

      <div className="space-y-2">
        {characters.map((character) => {
          const pictureInputId = `character-picture-${character.id}`;

          return (
            <div
              key={character.id}
              className="space-y-2 rounded border border-stone-800 bg-stone-950/70 p-3"
            >
              <div className="grid grid-cols-[48px_minmax(0,1fr)_auto] gap-3">
                <div className="flex size-12 items-center justify-center overflow-hidden rounded border border-stone-800 bg-stone-900 text-stone-500">
                  {character.portrait ? (
                    // eslint-disable-next-line @next/next/no-img-element
                    <img
                      src={character.portrait.url}
                      alt=""
                      className="size-full object-cover"
                    />
                  ) : (
                    <UserRound className="size-5" aria-hidden="true" />
                  )}
                </div>

                <div className="min-w-0 space-y-2">
                  <input
                    value={character.name}
                    onChange={(event) =>
                      onLocalChange(character.id, { name: event.target.value })
                    }
                    onBlur={() => onSave(character)}
                    className="w-full rounded border border-stone-800 bg-stone-950 px-2 py-1.5 text-sm font-medium text-stone-200 outline-none focus:border-amber-300"
                  />
                  <textarea
                    value={character.details}
                    onChange={(event) =>
                      onLocalChange(character.id, { details: event.target.value })
                    }
                    onBlur={() => onSave(character)}
                    rows={3}
                    className="w-full resize-none rounded border border-stone-800 bg-stone-950 px-2 py-1.5 text-xs text-stone-300 outline-none focus:border-amber-300"
                    placeholder="Детали, которые рассказчик должен сохранить..."
                  />
                </div>

                <DeleteCharacterDialog characterName={character.name} onConfirm={() => onDelete(character.id)}>
                  <button
                    type="button"
                    aria-label={`Delete ${character.name}`}
                    className="flex size-8 items-center justify-center rounded text-stone-500 hover:bg-stone-900 hover:text-red-200"
                  >
                    <Trash2 className="size-4" aria-hidden="true" />
                  </button>
                </DeleteCharacterDialog>
              </div>

              <div className="grid gap-2">
                <label className="block">
                  <span className="mb-1 flex items-center gap-1.5 text-xs font-medium uppercase text-stone-500">
                    <Backpack className="size-3.5" aria-hidden="true" />
                    Инвентарь
                  </span>
                  <textarea
                    value={character.inventory || ""}
                    onChange={(event) =>
                      onLocalChange(character.id, { inventory: event.target.value })
                    }
                    onBlur={() => onSave(character)}
                    rows={2}
                    className="w-full resize-none rounded border border-stone-800 bg-stone-950 px-2 py-1.5 text-xs text-stone-300 outline-none focus:border-amber-300"
                    placeholder="Предметы, снаряжение, деньги, квестовые объекты..."
                  />
                </label>
                <div className="grid gap-2 sm:grid-cols-2">
                  <label className="block">
                    <span className="mb-1 flex items-center gap-1.5 text-xs font-medium uppercase text-stone-500">
                      <Sparkles className="size-3.5" aria-hidden="true" />
                      Навыки
                    </span>
                    <textarea
                      value={character.skills || ""}
                      onChange={(event) =>
                        onLocalChange(character.id, { skills: event.target.value })
                      }
                      onBlur={() => onSave(character)}
                      rows={2}
                      className="w-full resize-none rounded border border-stone-800 bg-stone-950 px-2 py-1.5 text-xs text-stone-300 outline-none focus:border-amber-300"
                      placeholder="Таланты, умения, классовые особенности..."
                    />
                  </label>
                  <label className="block">
                    <span className="mb-1 flex items-center gap-1.5 text-xs font-medium uppercase text-stone-500">
                      <WandSparkles className="size-3.5" aria-hidden="true" />
                      Заклинания
                    </span>
                    <textarea
                      value={character.spells || ""}
                      onChange={(event) =>
                        onLocalChange(character.id, { spells: event.target.value })
                      }
                      onBlur={() => onSave(character)}
                      rows={2}
                      className="w-full resize-none rounded border border-stone-800 bg-stone-950 px-2 py-1.5 text-xs text-stone-300 outline-none focus:border-amber-300"
                      placeholder="Подготовленные заклинания, способности, заметки о перезарядке..."
                    />
                  </label>
                </div>
              </div>

              <div className="flex items-center gap-2">
                <input
                  id={pictureInputId}
                  name={pictureInputId}
                  type="file"
                  accept="image/png,image/jpeg,image/webp"
                  className="hidden"
                  onChange={(event) => {
                    const file = event.currentTarget.files?.[0];
                    if (file) {
                      onPortraitFile(character.id, file);
                    }
                    event.currentTarget.value = "";
                  }}
                />
                <label
                  htmlFor={pictureInputId}
                  className="inline-flex h-8 cursor-pointer items-center gap-2 rounded border border-stone-800 px-2 text-xs text-stone-400 hover:bg-stone-900 hover:text-stone-200"
                >
                  {uploadingId === character.id ? (
                    <Loader2 className="size-3.5 animate-spin" aria-hidden="true" />
                  ) : (
                    <ImagePlus className="size-3.5" aria-hidden="true" />
                  )}
                  Photo
                </label>
                {character.portrait && (
                  <button
                    type="button"
                    onClick={() => onClearPortrait(character.id)}
                    className="h-8 rounded border border-stone-800 px-2 text-xs text-stone-500 hover:bg-stone-900 hover:text-stone-200"
                  >
                    Очистить
                  </button>
                )}
              </div>
            </div>
          );
        })}

        {!characters.length && (
          <p className="rounded border border-dashed border-stone-800 px-3 py-4 text-sm text-stone-500">
            Сохранённые персонажи появятся здесь.
          </p>
        )}
      </div>
    </PanelSection>
  );
}

function DeleteCharacterDialog({
  characterName,
  onConfirm,
  children,
}: {
  characterName: string;
  onConfirm: () => void;
  children: ReactNode;
}) {
  return (
    <AlertDialog.Root>
      <AlertDialog.Trigger asChild>{children}</AlertDialog.Trigger>
      <AlertDialog.Portal>
        <AlertDialog.Overlay className="fixed inset-0 z-40 bg-stone-950/80" />
        <AlertDialog.Content className="fixed left-1/2 top-1/2 z-50 w-[min(calc(100vw-2rem),420px)] -translate-x-1/2 -translate-y-1/2 rounded-xl border border-stone-700 bg-[#130d09] p-5 shadow-xl">
          <AlertDialog.Title className="text-balance text-base font-semibold text-stone-100">
            Удалить этого персонажа?
          </AlertDialog.Title>
          <AlertDialog.Description className="mt-2 text-pretty text-sm text-stone-400">
            {characterName || "Этот персонаж"} будет удалён из этой истории.
          </AlertDialog.Description>
          <div className="mt-5 flex justify-end gap-2">
            <AlertDialog.Cancel asChild>
              <button
                type="button"
                className="rounded border border-stone-700 px-3 py-2 text-sm text-stone-300 hover:bg-stone-900"
              >
                Отмена
              </button>
            </AlertDialog.Cancel>
            <AlertDialog.Action asChild>
              <button
                type="button"
                onClick={onConfirm}
                className="rounded bg-red-300 px-3 py-2 text-sm font-medium text-red-950 hover:bg-red-200"
              >
                Удалить
              </button>
            </AlertDialog.Action>
          </div>
        </AlertDialog.Content>
      </AlertDialog.Portal>
    </AlertDialog.Root>
  );
}

// One-tap URL fills for the most common OpenAI-compatible servers.
const SERVER_PRESETS: Array<{ label: string; url: string }> = [
  { label: "Этот компьютер", url: "http://127.0.0.1:8080/v1" },
  { label: "LM Studio", url: "http://localhost:1234/v1" },
  { label: "Ollama", url: "http://127.0.0.1:11434/v1" },
  { label: "OpenRouter", url: "https://openrouter.ai/api/v1" },
];

function TextModelPanel({
  settings,
  setSettings,
  localTextStatus,
  compact = false,
  open,
  onOpenChange,
  divided,
}: {
  settings: StorySettings;
  setSettings: Dispatch<SetStateAction<StorySettings>>;
  localTextStatus: LocalTextStatus | null;
  compact?: boolean;
} & PanelControlProps) {
  const idPrefix = compact ? "mobile" : "desktop";
  const selectedMissing =
    localTextStatus?.ok && !localTextStatus.installedModels.includes(settings.localTextModel);

  // Live model list from the custom OpenAI-compatible server's /models endpoint.
  // ok=true once a fetch returns >=1 model; lets us show a <select> instead of
  // only the free-text field. Refetched (debounced, abortable) when the URL or
  // key changes, but only while the custom provider is active.
  const [customModels, setCustomModels] = useState<Array<{ id: string; label: string }>>([]);
  const [customModelsError, setCustomModelsError] = useState(false);
  const customBaseUrl = settings.customBaseUrl;
  const customApiKey = settings.customApiKey;
  const isCustom = settings.textProvider === "custom";

  useEffect(() => {
    if (!isCustom) {
      setCustomModels([]);
      setCustomModelsError(false);
      return;
    }
    const trimmed = customBaseUrl.trim();
    if (!trimmed) {
      setCustomModels([]);
      setCustomModelsError(false);
      return;
    }
    const controller = new AbortController();
    const handle = setTimeout(() => {
      const url = `${trimmed.replace(/\/+$/, "")}/models`;
      fetch(url, {
        signal: controller.signal,
        headers: customApiKey.trim()
          ? { Authorization: `Bearer ${customApiKey.trim()}` }
          : undefined,
      })
        .then((res) => (res.ok ? res.json() : Promise.reject(new Error(`HTTP ${res.status}`))))
        .then((body: unknown) => {
          const rawList = Array.isArray(body)
            ? body
            : Array.isArray((body as { data?: unknown })?.data)
              ? (body as { data: unknown[] }).data
              : [];
          const parsed = rawList
            .map((item) => {
              const rec = item as { id?: unknown; label?: unknown; name?: unknown };
              const id = typeof rec.id === "string" ? rec.id : "";
              const label =
                typeof rec.label === "string" && rec.label
                  ? rec.label
                  : typeof rec.name === "string" && rec.name
                    ? rec.name
                    : id;
              return { id, label };
            })
            .filter((m) => m.id);
          setCustomModels(parsed);
          setCustomModelsError(false);
        })
        .catch((err: unknown) => {
          if ((err as { name?: string })?.name === "AbortError") return;
          setCustomModels([]);
          setCustomModelsError(true);
        });
    }, 400);
    return () => {
      clearTimeout(handle);
      controller.abort();
    };
  }, [isCustom, customBaseUrl, customApiKey]);

  const customModelInList =
    customModels.length > 0 && customModels.some((m) => m.id === settings.customModel);

  return (
    <PanelSection
      icon={Cpu}
      iconSrc={SIDEBAR_ICONS.textModel}
      title="Текстовая модель"
      compact={compact}
      defaultOpen={compact}
      open={open}
      onOpenChange={onOpenChange}
      divided={divided}
    >
      <div className="space-y-2">
        <span className="block text-xs font-medium uppercase text-stone-500">Провайдер</span>
        <Segmented<TextProvider>
          value={settings.textProvider}
          options={[
            { value: "custom", label: "Локальный сервер" },
            { value: "local", label: "Ollama" },
          ]}
          onChange={(textProvider) =>
            setSettings((current) => ({ ...current, textProvider }))
          }
        />
      </div>
      {settings.textProvider === "local" ? (
        <div className="space-y-2">
          <span className="block text-xs font-medium uppercase text-stone-500">
            Gemma 4 QAT (Q4)
          </span>
          <select
            id={`${idPrefix}-local-text-model`}
            name={`${idPrefix}-local-text-model`}
            value={settings.localTextModel}
            onChange={(event) =>
              setSettings((current) => ({
                ...current,
                localTextModel: event.target.value as LocalTextModelId,
              }))
            }
            className="w-full rounded border border-stone-800 bg-stone-950 px-3 py-2 text-sm text-stone-200 outline-none focus:border-amber-300"
          >
            {LOCAL_TEXT_MODELS.map((model) => {
              const missing =
                localTextStatus?.ok && !localTextStatus.installedModels.includes(model.id);
              return (
                <option key={model.id} value={model.id}>
                  {`${model.label} · ${model.ram} RAM${missing ? ` · needs ${model.size} download` : ""}`}
                </option>
              );
            })}
          </select>
          {localTextStatus && !localTextStatus.ok && (
            <p className="text-xs text-amber-200/80">
              Ollama недоступна. Это необязательный провайдер Ollama — основной
              путь «Локальный сервер» в ней не нуждается. Запусти Ollama, если
              хочешь её использовать, затем перезагрузи страницу.
            </p>
          )}
          {selectedMissing && (
            <p className="text-xs text-amber-200/80">
              Install with{" "}
              <code className="rounded bg-stone-900 px-1 py-0.5 text-amber-100">
                ollama pull {settings.localTextModel}
              </code>
            </p>
          )}
        </div>
      ) : (
        <div className="space-y-3">
          <div className="space-y-1.5">
            <span className="block text-xs font-medium uppercase text-stone-500">
              Быстрое заполнение
            </span>
            <div className="flex flex-wrap gap-1.5">
              {SERVER_PRESETS.map((preset) => (
                <button
                  key={preset.label}
                  type="button"
                  onClick={() =>
                    setSettings((current) => ({ ...current, customBaseUrl: preset.url }))
                  }
                  className="rounded border border-stone-700 bg-stone-900/40 px-2 py-1 text-xs text-stone-300 hover:border-amber-700/60 hover:bg-stone-900"
                >
                  {preset.label}
                </button>
              ))}
            </div>
          </div>
          <div className="space-y-1.5">
            <label
              htmlFor={`${idPrefix}-custom-base-url`}
              className="block text-xs font-medium uppercase text-stone-500"
            >
              URL сервера
            </label>
            <input
              id={`${idPrefix}-custom-base-url`}
              name={`${idPrefix}-custom-base-url`}
              type="text"
              inputMode="url"
              autoComplete="off"
              autoCorrect="off"
              autoCapitalize="off"
              spellCheck={false}
              placeholder="http://127.0.0.1:8080/v1"
              value={settings.customBaseUrl}
              onChange={(event) =>
                setSettings((current) => ({ ...current, customBaseUrl: event.target.value }))
              }
              className="w-full rounded border border-stone-800 bg-stone-950 px-3 py-2 text-sm text-stone-200 outline-none focus:border-amber-300"
            />
          </div>
          <div className="space-y-1.5">
            <label
              htmlFor={`${idPrefix}-custom-model`}
              className="block text-xs font-medium uppercase text-stone-500"
            >
              Модель
            </label>
            {customModels.length > 0 && (
              <select
                id={`${idPrefix}-custom-model-select`}
                name={`${idPrefix}-custom-model-select`}
                value={customModelInList ? settings.customModel : ""}
                onChange={(event) => {
                  const value = event.target.value;
                  if (!value) return;
                  setSettings((current) => ({ ...current, customModel: value }));
                }}
                className="w-full rounded border border-stone-800 bg-stone-950 px-3 py-2 text-sm text-stone-200 outline-none focus:border-amber-300"
              >
                {!customModelInList && (
                  <option value="">
                    {settings.customModel
                      ? `${settings.customModel} (вручную)`
                      : "— выбери модель —"}
                  </option>
                )}
                {customModels.map((model) => (
                  <option key={model.id} value={model.id}>
                    {model.label || model.id}
                  </option>
                ))}
              </select>
            )}
            <input
              id={`${idPrefix}-custom-model`}
              name={`${idPrefix}-custom-model`}
              type="text"
              autoComplete="off"
              autoCorrect="off"
              autoCapitalize="off"
              spellCheck={false}
              placeholder={
                customModels.length > 0 ? "или впиши id вручную" : "e.g. llama-3.3-70b-instruct"
              }
              value={settings.customModel}
              onChange={(event) =>
                setSettings((current) => ({ ...current, customModel: event.target.value }))
              }
              className="w-full rounded border border-stone-800 bg-stone-950 px-3 py-2 text-sm text-stone-200 outline-none focus:border-amber-300"
            />
            {customModelsError && (
              <p className="text-xs text-stone-500">
                Не удалось получить список моделей с сервера — впиши id вручную.
              </p>
            )}
          </div>
          <div className="space-y-1.5">
            <label
              htmlFor={`${idPrefix}-custom-api-key`}
              className="block text-xs font-medium uppercase text-stone-500"
            >
              API-ключ <span className="normal-case text-stone-600">(необязательно)</span>
            </label>
            <input
              id={`${idPrefix}-custom-api-key`}
              name={`${idPrefix}-custom-api-key`}
              type="password"
              autoComplete="off"
              autoCorrect="off"
              autoCapitalize="off"
              spellCheck={false}
              placeholder="только если сервер этого требует"
              value={settings.customApiKey}
              onChange={(event) =>
                setSettings((current) => ({ ...current, customApiKey: event.target.value }))
              }
              className="w-full rounded border border-stone-800 bg-stone-950 px-3 py-2 text-sm text-stone-200 outline-none focus:border-amber-300"
            />
          </div>
          <p className="text-xs leading-relaxed text-stone-500">
            Any OpenAI-compatible server: llama.cpp, LM Studio, vLLM, TabbyAPI, KoboldCpp,
            OpenRouter, or a remote Ollama. Everything stays on your machine, and most
            local servers need no key. For OpenRouter, paste a model id from{" "}
            <a
              href="https://openrouter.ai/models"
              target="_blank"
              rel="noreferrer noopener"
              className="text-amber-200/90 underline underline-offset-2 hover:text-amber-100"
            >
              openrouter.ai/models
            </a>
            .
          </p>
        </div>
      )}
    </PanelSection>
  );
}

function FontSizeSlider({
  settings,
  setSettings,
  idPrefix,
  className,
}: {
  settings: StorySettings;
  setSettings: Dispatch<SetStateAction<StorySettings>>;
  idPrefix: string;
  className?: string;
}) {
  const proseSizeValue = proseSizeSliderValue(settings.proseSize);
  const proseSizeLabel = PROSE_SIZE_OPTIONS[proseSizeValue]?.label ?? "18px";

  return (
    <div
      className={cn(
        "space-y-2 rounded border border-stone-800 bg-stone-950 px-3 py-2",
        className,
      )}
    >
      <div className="flex items-center justify-between gap-3">
        <span className="flex items-center gap-1.5 text-xs font-medium uppercase text-stone-500">
          <Type className="size-3.5" aria-hidden="true" />
          Размер шрифта
        </span>
        <span className="text-xs font-medium text-amber-100">{proseSizeLabel}</span>
      </div>
      <input
        id={`${idPrefix}-font-size`}
        name={`${idPrefix}-font-size`}
        type="range"
        min={0}
        max={PROSE_SIZE_OPTIONS.length - 1}
        step={1}
        value={proseSizeValue}
        aria-label="Размер шрифта"
        onChange={(event) => {
          const option =
            PROSE_SIZE_OPTIONS[Number(event.target.value)] ?? PROSE_SIZE_OPTIONS[3];
          setSettings((current) => ({ ...current, proseSize: option.value }));
        }}
        className="w-full accent-amber-200"
      />
      <div className="flex justify-between text-[0.65rem] font-medium uppercase text-stone-600">
        {PROSE_SIZE_OPTIONS.map((option) => (
          <span key={option.value}>{option.label}</span>
        ))}
      </div>
    </div>
  );
}

function ResponseLengthControl({
  settings,
  setSettings,
  idPrefix,
  className,
}: {
  settings: StorySettings;
  setSettings: Dispatch<SetStateAction<StorySettings>>;
  idPrefix: string;
  className?: string;
}) {
  const value = Math.max(
    0,
    RESPONSE_LENGTH_OPTIONS.findIndex((option) => option.value === settings.responseLength),
  );
  const label = RESPONSE_LENGTH_OPTIONS[value]?.label ?? "Средне";

  return (
    <div
      className={cn(
        "space-y-2 rounded border border-stone-800 bg-stone-950 px-3 py-2",
        className,
      )}
    >
      <div className="flex items-center justify-between gap-3">
        <span className="text-xs font-medium uppercase text-stone-500">Длина ответа</span>
        <span className="text-xs font-medium text-amber-100">{label}</span>
      </div>
      <input
        id={`${idPrefix}-response-length`}
        name={`${idPrefix}-response-length`}
        type="range"
        min={0}
        max={RESPONSE_LENGTH_OPTIONS.length - 1}
        step={1}
        value={value}
        aria-label="Длина ответа"
        onChange={(event) => {
          const option =
            RESPONSE_LENGTH_OPTIONS[Number(event.target.value)] ?? RESPONSE_LENGTH_OPTIONS[1];
          setSettings((current) => ({ ...current, responseLength: option.value }));
        }}
        className="w-full accent-amber-200"
      />
      <div className="flex justify-between text-[0.65rem] font-medium uppercase text-stone-600">
        {RESPONSE_LENGTH_OPTIONS.map((option) => (
          <span key={option.value}>{option.label}</span>
        ))}
      </div>
    </div>
  );
}

function VoiceControl({
  settings,
  setSettings,
  className,
}: {
  settings: StorySettings;
  setSettings: Dispatch<SetStateAction<StorySettings>>;
  className?: string;
}) {
  const [voices, setVoices] = useState<string[]>([]);
  const [uploaded, setUploaded] = useState<string[]>([]);
  const [uploadingVoice, setUploadingVoice] = useState(false);
  const [voiceError, setVoiceError] = useState<string | null>(null);
  const voiceFileRef = useRef<HTMLInputElement>(null);
  useEffect(() => {
    let active = true;
    fetch("/api/voices")
      .then((response) => (response.ok ? response.json() : { voices: [] }))
      .then((data: { voices?: string[] }) => {
        if (active && Array.isArray(data.voices)) {
          setVoices(data.voices);
        }
      })
      .catch(() => {});
    return () => {
      active = false;
    };
  }, []);

  async function uploadVoice(file: File) {
    setVoiceError(null);
    setUploadingVoice(true);
    try {
      const form = new FormData();
      form.append("file", file);
      const response = await fetch("/api/tts-voice", { method: "POST", body: form });
      const data = (await response.json().catch(() => ({}))) as {
        voice?: string;
        error?: string;
      };
      if (!response.ok || !data.voice) {
        setVoiceError(data.error || "Не удалось загрузить голос.");
        return;
      }
      const id = data.voice;
      setUploaded((current) => (current.includes(id) ? current : [...current, id]));
      setSettings((current) => ({ ...current, voice: id }));
    } catch {
      setVoiceError("Сервер озвучки не запущен (порт 8081).");
    } finally {
      setUploadingVoice(false);
    }
  }

  const merged = [...voices, ...uploaded.filter((v) => !voices.includes(v))];
  const list = merged.length ? merged : [settings.voice];

  return (
    <div
      className={cn(
        "space-y-2 rounded border border-stone-800 bg-stone-950 px-3 py-2",
        className,
      )}
    >
      <label className="flex items-center justify-between text-sm text-stone-300">
        Автоозвучка
        <input
          type="checkbox"
          checked={settings.autoplay}
          onChange={(event) =>
            setSettings((current) => ({ ...current, autoplay: event.target.checked }))
          }
          className="size-4 accent-amber-200"
        />
      </label>
      <div>
        <span className="mb-1 block text-xs font-medium uppercase text-stone-500">
          Голос озвучки
        </span>
        <select
          value={settings.voice}
          onChange={(event) =>
            setSettings((current) => ({ ...current, voice: event.target.value }))
          }
          className="w-full rounded border border-stone-800 bg-stone-950 px-2 py-1.5 text-sm text-stone-200"
        >
          {list.map((voice) => (
            <option key={voice} value={voice}>
              {voice}
            </option>
          ))}
        </select>
        <input
          ref={voiceFileRef}
          type="file"
          accept="audio/mpeg,.mp3"
          className="hidden"
          onChange={(event) => {
            const file = event.target.files?.[0];
            event.target.value = "";
            if (file) {
              void uploadVoice(file);
            }
          }}
        />
        <button
          type="button"
          disabled={uploadingVoice}
          onClick={() => voiceFileRef.current?.click()}
          className="mt-2 w-full rounded border border-stone-800 bg-stone-900 px-2 py-1.5 text-xs font-medium text-stone-200 hover:border-amber-300 disabled:opacity-50"
        >
          {uploadingVoice ? "Загрузка…" : "Загрузить свой голос (.mp3)"}
        </button>
        {voiceError && <p className="mt-1 text-xs text-red-400">{voiceError}</p>}
      </div>
      <div>
        <div className="mb-1 flex items-center justify-between">
          <span className="text-xs font-medium uppercase text-stone-500">Громкость</span>
          <span className="text-xs font-medium text-amber-100">{Math.round(settings.ttsVolume * 100)}%</span>
        </div>
        <input
          type="range"
          min={0}
          max={1}
          step={0.05}
          value={settings.ttsVolume}
          aria-label="Громкость"
          onChange={(event) =>
            setSettings((current) => ({ ...current, ttsVolume: Number(event.target.value) }))
          }
          className="w-full accent-amber-200"
        />
      </div>
      <div>
        <div className="mb-1 flex items-center justify-between">
          <span className="text-xs font-medium uppercase text-stone-500">Скорость</span>
          <span className="text-xs font-medium text-amber-100">{settings.ttsSpeed.toFixed(2)}×</span>
        </div>
        <input
          type="range"
          min={0.5}
          max={2}
          step={0.05}
          value={settings.ttsSpeed}
          aria-label="Скорость"
          onChange={(event) =>
            setSettings((current) => ({ ...current, ttsSpeed: Number(event.target.value) }))
          }
          className="w-full accent-amber-200"
        />
      </div>
    </div>
  );
}

function VoicePanel({
  settings,
  setSettings,
  compact = false,
  open,
  onOpenChange,
  divided,
}: {
  settings: StorySettings;
  setSettings: Dispatch<SetStateAction<StorySettings>>;
  compact?: boolean;
} & PanelControlProps) {
  return (
    <PanelSection
      icon={Volume2}
      title="Озвучка"
      compact={compact}
      defaultOpen={compact}
      open={open}
      onOpenChange={onOpenChange}
      divided={divided}
    >
      <VoiceControl settings={settings} setSettings={setSettings} />
    </PanelSection>
  );
}

// --- Voice input: record the mic to 16 kHz mono WAV in the browser, POST /api/asr ---
function mergeFloat32(chunks: Float32Array[]): Float32Array {
  let length = 0;
  for (const chunk of chunks) length += chunk.length;
  const out = new Float32Array(length);
  let offset = 0;
  for (const chunk of chunks) {
    out.set(chunk, offset);
    offset += chunk.length;
  }
  return out;
}

function downsampleBuffer(buffer: Float32Array, from: number, to: number): Float32Array {
  if (to >= from) return buffer;
  const ratio = from / to;
  const newLength = Math.round(buffer.length / ratio);
  const out = new Float32Array(newLength);
  for (let i = 0; i < newLength; i += 1) {
    const start = Math.floor(i * ratio);
    const end = Math.min(buffer.length, Math.floor((i + 1) * ratio));
    let sum = 0;
    let count = 0;
    for (let j = start; j < end; j += 1) {
      sum += buffer[j];
      count += 1;
    }
    out[i] = count ? sum / count : buffer[start] || 0;
  }
  return out;
}

function encodeWav(samples: Float32Array, sampleRate: number): ArrayBuffer {
  const buffer = new ArrayBuffer(44 + samples.length * 2);
  const view = new DataView(buffer);
  const writeString = (offset: number, value: string) => {
    for (let i = 0; i < value.length; i += 1) view.setUint8(offset + i, value.charCodeAt(i));
  };
  writeString(0, "RIFF");
  view.setUint32(4, 36 + samples.length * 2, true);
  writeString(8, "WAVE");
  writeString(12, "fmt ");
  view.setUint32(16, 16, true);
  view.setUint16(20, 1, true);
  view.setUint16(22, 1, true);
  view.setUint32(24, sampleRate, true);
  view.setUint32(28, sampleRate * 2, true);
  view.setUint16(32, 2, true);
  view.setUint16(34, 16, true);
  writeString(36, "data");
  view.setUint32(40, samples.length * 2, true);
  let offset = 44;
  for (let i = 0; i < samples.length; i += 1) {
    const clamped = Math.max(-1, Math.min(1, samples[i]));
    view.setInt16(offset, clamped < 0 ? clamped * 0x8000 : clamped * 0x7fff, true);
    offset += 2;
  }
  return buffer;
}

function MicButton({
  onTranscript,
  disabled,
}: {
  onTranscript: (text: string) => void;
  disabled?: boolean;
}) {
  const [recording, setRecording] = useState(false);
  const [busy, setBusy] = useState(false);
  const ctxRef = useRef<AudioContext | null>(null);
  const streamRef = useRef<MediaStream | null>(null);
  const procRef = useRef<ScriptProcessorNode | null>(null);
  const chunksRef = useRef<Float32Array[]>([]);

  const stop = async () => {
    setRecording(false);
    const ctx = ctxRef.current;
    const stream = streamRef.current;
    const proc = procRef.current;
    const sampleRate = ctx?.sampleRate ?? 48000;
    proc?.disconnect();
    stream?.getTracks().forEach((track) => track.stop());
    if (ctx) await ctx.close().catch(() => {});
    ctxRef.current = null;
    streamRef.current = null;
    procRef.current = null;

    const pcm = mergeFloat32(chunksRef.current);
    chunksRef.current = [];
    if (!pcm.length) return;

    const wav = encodeWav(downsampleBuffer(pcm, sampleRate, 16000), 16000);
    setBusy(true);
    try {
      const response = await fetch("/api/asr", {
        method: "POST",
        headers: { "Content-Type": "audio/wav" },
        body: wav,
      });
      const data = (await response.json()) as { text?: string };
      if (response.ok && data.text) onTranscript(data.text);
    } catch {
      // voice input is optional
    } finally {
      setBusy(false);
    }
  };

  const start = async () => {
    try {
      const stream = await navigator.mediaDevices.getUserMedia({ audio: true });
      const ctx = new AudioContext();
      const source = ctx.createMediaStreamSource(stream);
      const proc = ctx.createScriptProcessor(4096, 1, 1);
      chunksRef.current = [];
      proc.onaudioprocess = (event) => {
        chunksRef.current.push(new Float32Array(event.inputBuffer.getChannelData(0)));
      };
      source.connect(proc);
      proc.connect(ctx.destination);
      ctxRef.current = ctx;
      streamRef.current = stream;
      procRef.current = proc;
      setRecording(true);
    } catch {
      // mic permission denied or unavailable
    }
  };

  return (
    <button
      type="button"
      onClick={() => void (recording ? stop() : start())}
      disabled={disabled || busy}
      title={recording ? "Остановить запись" : "Голосовой ввод"}
      className="flex size-9 shrink-0 items-center justify-center rounded-lg text-stone-400 transition hover:bg-stone-900 hover:text-stone-200 disabled:cursor-not-allowed disabled:text-stone-600"
    >
      {busy ? (
        <Loader2 className="size-4 animate-spin" />
      ) : (
        <Mic size={18} className={recording ? "animate-pulse text-red-400" : ""} />
      )}
    </button>
  );
}

type RollResult = {
  d20: number;
  total: number;
  dc: number;
  success: boolean;
  crit: "success" | "fail" | null;
  modifier: number;
};

function hpTone(pct: number): { bar: string; text: string } {
  if (pct <= 25) return { bar: "bg-red-500", text: "text-red-300" };
  if (pct <= 50) return { bar: "bg-amber-400", text: "text-amber-200" };
  return { bar: "bg-emerald-500", text: "text-emerald-200" };
}

// Player HUD: name + level, coloured HP bar, AC, six abilities with 5e modifiers,
// active conditions. Read-only; mirrors the stone/amber journal strip styling.
function HudBar({ hero, name }: { hero: CharacterRpg; name: string }) {
  const max = Math.max(1, hero.hp.max);
  const cur = Math.max(0, Math.min(hero.hp.current, max));
  const pct = Math.round((cur / max) * 100);
  const tone = hpTone(pct);
  return (
    <div className="rounded border border-amber-900/40 bg-amber-950/10 px-3 py-2 text-amber-100/90">
      <div className="flex items-center justify-between gap-3">
        <div className="flex items-baseline gap-2">
          <span className="truncate font-serif text-sm text-stone-100">{name}</span>
          <span className="text-[10px] uppercase tracking-wide text-amber-300/70">Ур. {hero.level}</span>
          {hero.dead && (
            <span className="rounded border border-red-500/60 px-1.5 text-[10px] font-medium uppercase text-red-300">
              ☠️ погиб
            </span>
          )}
        </div>
        <span className="inline-flex items-center gap-1 text-xs text-stone-300">
          <Shield className="size-3.5 text-amber-300" aria-hidden="true" />
          <span className="tabular-nums">AC {hero.ac}</span>
        </span>
      </div>
      <div className="mt-1.5 flex items-center gap-2">
        <Heart className={cn("size-3.5 shrink-0", tone.text)} aria-hidden="true" />
        <div className="h-2 flex-1 overflow-hidden rounded-full bg-stone-800">
          <div className={cn("h-full rounded-full transition-all", tone.bar)} style={{ width: `${pct}%` }} />
        </div>
        <span className={cn("shrink-0 text-xs tabular-nums", tone.text)}>
          {cur}/{max}
        </span>
      </div>
      <div className="mt-2 grid grid-cols-6 gap-1">
        {ABILITIES.map((ability) => {
          const score = hero.stats[ability];
          const mod = abilityMod(score);
          return (
            <div
              key={ability}
              className="flex flex-col items-center rounded border border-stone-800 bg-stone-950/60 py-1"
              title={ABILITY_LABELS_RU[ability]}
            >
              <span className="text-[9px] uppercase tracking-wide text-stone-500">
                {ABILITY_LABELS_RU[ability].slice(0, 3)}
              </span>
              <span className="text-sm font-semibold tabular-nums text-stone-200">{score}</span>
              <span className="text-[10px] tabular-nums text-amber-300/80">
                {mod >= 0 ? `+${mod}` : mod}
              </span>
            </div>
          );
        })}
      </div>
      {hero.conditions.length > 0 && (
        <div className="mt-2 flex flex-wrap gap-1">
          {hero.conditions.map((condition) => (
            <span
              key={condition}
              className="rounded-full border border-amber-900/60 bg-amber-950/30 px-2 py-0.5 text-[10px] text-amber-200"
            >
              {condition}
            </span>
          ))}
        </div>
      )}
    </div>
  );
}

const RARITY_TONE: Record<Item["rarity"], string> = {
  common: "text-stone-200",
  uncommon: "text-emerald-300",
  rare: "text-sky-300",
  epic: "text-fuchsia-300",
  legendary: "text-amber-300",
};
const SLOT_RU: Record<Item["slot"], string> = {
  weapon: "оружие",
  armor: "броня",
  shield: "щит",
  trinket: "украшение",
  consumable: "расходник",
  misc: "прочее",
};
const SLOT_ICON: Record<Item["slot"], typeof Swords> = {
  weapon: Swords,
  armor: ShieldHalf,
  shield: Shield,
  trinket: Sparkles,
  consumable: Heart,
  misc: Backpack,
};

// Inventory list: rarity-tinted name, slot/damage line, optional generated
// thumbnail, equip/unequip toggle. Equippable = anything that isn't consumable/misc.
function InventoryPanel({
  items,
  onToggle,
  disabled,
}: {
  items: Item[];
  onToggle: (itemId: string, equipped: boolean) => void;
  disabled?: boolean;
}) {
  return (
    <div className="max-h-44 space-y-1 overflow-y-auto rounded border border-amber-900/40 bg-amber-950/10 px-3 py-2">
      <div className="mb-1 flex items-center gap-1.5 text-[10px] font-medium uppercase tracking-wide text-amber-300/70">
        <Backpack className="size-3.5" aria-hidden="true" />
        Инвентарь
      </div>
      {items.map((item) => {
        const Icon = SLOT_ICON[item.slot];
        const equippable = item.slot !== "consumable" && item.slot !== "misc";
        return (
          <div
            key={item.id}
            className="flex items-center gap-2 rounded border border-stone-800 bg-stone-950/60 px-2 py-1.5"
          >
            {item.imageUrl ? (
              // eslint-disable-next-line @next/next/no-img-element
              <img
                src={item.imageUrl}
                alt=""
                className="size-9 shrink-0 rounded object-cover"
                onError={(event) => {
                  event.currentTarget.style.display = "none";
                }}
              />
            ) : (
              <div className="flex size-9 shrink-0 items-center justify-center rounded bg-stone-900">
                <Icon className="size-4 text-stone-500" aria-hidden="true" />
              </div>
            )}
            <div className="min-w-0 flex-1">
              <div className="flex items-center gap-1.5">
                <span className={cn("truncate text-sm font-medium", RARITY_TONE[item.rarity])}>
                  {item.name}
                </span>
                {item.qty > 1 && (
                  <span className="shrink-0 text-[10px] tabular-nums text-stone-500">×{item.qty}</span>
                )}
              </div>
              <div className="truncate text-[10px] text-stone-500">
                {SLOT_RU[item.slot]}
                {item.damage ? ` · ${item.damage}` : ""}
              </div>
            </div>
            {equippable && (
              <button
                type="button"
                disabled={disabled}
                onClick={() => onToggle(item.id, !item.equipped)}
                className={cn(
                  "shrink-0 rounded border px-2 py-1 text-[10px] font-medium uppercase transition disabled:opacity-40",
                  item.equipped
                    ? "border-amber-300 bg-amber-300/10 text-amber-200 hover:bg-amber-300/20"
                    : "border-stone-700 text-stone-400 hover:border-amber-300/60 hover:text-amber-200",
                )}
              >
                {item.equipped ? "Снять" : "Надеть"}
              </button>
            )}
          </div>
        );
      })}
    </div>
  );
}

// Full character sheet for the left column: hero portrait, HP/AC, ability stones,
// worn equipment, and an inventory satchel grid (click a slot to equip/unequip).
function CharacterSheet({
  hero,
  name,
  portrait,
  items,
  onToggle,
  busy,
}: {
  hero: CharacterRpg;
  name: string;
  portrait?: Attachment | null;
  items: Item[];
  onToggle: (itemId: string, equipped: boolean) => void;
  busy?: boolean;
}) {
  const equipped = items.filter((item) => item.equipped);
  // Fold equipped-gear modifiers into the displayed stats / AC / max-HP, so loot
  // visibly changes the hero (a +2 STR blade reads as a higher, amber-tinted STR).
  const bonus: Record<string, number> = {};
  for (const item of equipped) {
    for (const [key, value] of Object.entries(item.modifiers ?? {})) {
      if (typeof value === "number") bonus[key] = (bonus[key] ?? 0) + value;
    }
  }
  const maxHp = Math.max(1, hero.hp.max + (bonus.maxHp ?? 0));
  const cur = Math.max(0, Math.min(hero.hp.current, maxHp));
  const pct = Math.round((cur / maxHp) * 100);
  const tone = hpTone(pct);
  const effAc = hero.ac + (bonus.ac ?? 0);
  return (
    <div className="flex flex-col gap-2">
      <div className="relative overflow-hidden rounded-lg border border-amber-900/40 bg-amber-950/10">
        <div className="aspect-[4/5] w-full bg-stone-950">
          {portrait?.url ? (
            // eslint-disable-next-line @next/next/no-img-element
            <img src={portrait.url} alt={name} className="size-full object-cover" />
          ) : (
            <div className="flex size-full items-center justify-center">
              <UserRound className="size-14 text-stone-700" aria-hidden="true" />
            </div>
          )}
          <div className="absolute inset-x-0 bottom-0 bg-gradient-to-t from-stone-950/95 to-transparent px-3 pb-2 pt-8">
            <div className="flex items-end justify-between gap-2">
              <span className="truncate font-serif text-base text-stone-50">{name}</span>
              <span className="shrink-0 text-[10px] uppercase tracking-wide text-amber-300/80">
                ур. {hero.level}
              </span>
            </div>
          </div>
          {hero.dead && (
            <div className="absolute inset-0 flex items-center justify-center bg-red-950/55 text-sm font-semibold uppercase tracking-wider text-red-200">
              ☠️ погиб
            </div>
          )}
        </div>
      </div>

      <div className="rounded-lg border border-amber-900/40 bg-amber-950/10 px-3 py-2">
        <div className="flex items-center gap-2">
          <Heart className={cn("size-4 shrink-0", tone.text)} aria-hidden="true" />
          <div className="h-2.5 flex-1 overflow-hidden rounded-full bg-stone-800">
            <div
              className={cn("h-full rounded-full transition-all", tone.bar)}
              style={{ width: `${pct}%` }}
            />
          </div>
          <span className={cn("shrink-0 text-xs tabular-nums", tone.text)}>
            {cur}/{maxHp}
          </span>
        </div>
        <div className="mt-2 flex items-center justify-between gap-2 text-xs text-stone-300">
          <span className="inline-flex items-center gap-1">
            <Shield className="size-3.5 text-amber-300" aria-hidden="true" />
            Класс защиты{" "}
            <span className={cn("tabular-nums", bonus.ac ? "text-amber-300" : undefined)}>{effAc}</span>
          </span>
          {hero.conditions.length > 0 && (
            <span className="truncate text-amber-200/80">{hero.conditions.join(", ")}</span>
          )}
        </div>
      </div>

      <div className="grid grid-cols-3 gap-1.5 rounded-lg border border-amber-900/40 bg-amber-950/10 p-2">
        {ABILITIES.map((ability) => {
          const score = hero.stats[ability] + (bonus[ability] ?? 0);
          const mod = abilityMod(score);
          const buffed = Boolean(bonus[ability]);
          return (
            <div
              key={ability}
              className="flex flex-col items-center rounded border border-stone-800 bg-stone-950/60 py-1.5"
              title={ABILITY_LABELS_RU[ability]}
            >
              <span className="text-[9px] uppercase tracking-wide text-stone-500">
                {ABILITY_LABELS_RU[ability].slice(0, 3)}
              </span>
              <span
                className={cn(
                  "text-base font-semibold tabular-nums",
                  buffed ? "text-amber-300" : "text-stone-100",
                )}
              >
                {score}
              </span>
              <span className="text-[10px] tabular-nums text-amber-300/80">
                {mod >= 0 ? `+${mod}` : mod}
              </span>
            </div>
          );
        })}
      </div>

      {equipped.length > 0 && (
        <div className="rounded-lg border border-amber-900/40 bg-amber-950/10 px-3 py-2">
          <div className="mb-1.5 text-[10px] font-medium uppercase tracking-wide text-amber-300/70">
            Снаряжение
          </div>
          <div className="flex flex-wrap gap-1.5">
            {equipped.map((item) => {
              const Icon = SLOT_ICON[item.slot];
              return (
                <span
                  key={item.id}
                  className={cn(
                    "inline-flex items-center gap-1 rounded border border-amber-300/40 bg-amber-300/5 px-2 py-1 text-[11px]",
                    RARITY_TONE[item.rarity],
                  )}
                >
                  <Icon className="size-3" aria-hidden="true" />
                  {item.name}
                </span>
              );
            })}
          </div>
        </div>
      )}

      <div className="rounded-lg border border-amber-900/40 bg-amber-950/10 px-3 py-2">
        <div className="mb-1.5 flex items-center gap-1.5 text-[10px] font-medium uppercase tracking-wide text-amber-300/70">
          <Backpack className="size-3.5" aria-hidden="true" />
          Инвентарь
        </div>
        {items.length === 0 ? (
          <p className="py-2 text-center text-[11px] text-stone-600">Пусто — добыча появится в бою</p>
        ) : (
          <div className="grid grid-cols-4 gap-1.5">
            {items.map((item) => {
              const Icon = SLOT_ICON[item.slot];
              const equippable = item.slot !== "consumable" && item.slot !== "misc";
              return (
                <button
                  key={item.id}
                  type="button"
                  disabled={busy || !equippable}
                  onClick={() => equippable && onToggle(item.id, !item.equipped)}
                  title={`${item.name}${item.damage ? ` · ${item.damage}` : ""}${
                    equippable ? (item.equipped ? " · надето (снять)" : " · надеть") : ""
                  }`}
                  className={cn(
                    "relative flex aspect-square items-center justify-center rounded border bg-stone-950/60 transition disabled:cursor-default",
                    item.equipped
                      ? "border-amber-300/70 shadow-[0_0_10px_rgba(251,191,36,0.2)]"
                      : "border-stone-800 hover:border-amber-300/40",
                  )}
                >
                  {item.imageUrl ? (
                    // eslint-disable-next-line @next/next/no-img-element
                    <img
                      src={item.imageUrl}
                      alt=""
                      className="size-full rounded object-cover"
                      onError={(event) => {
                        event.currentTarget.style.display = "none";
                      }}
                    />
                  ) : (
                    <Icon className={cn("size-5", RARITY_TONE[item.rarity])} aria-hidden="true" />
                  )}
                  {item.qty > 1 && (
                    <span className="absolute bottom-0 right-0.5 text-[9px] tabular-nums text-stone-300">
                      ×{item.qty}
                    </span>
                  )}
                  {item.equipped && (
                    <span className="absolute left-0.5 top-0.5 size-1.5 rounded-full bg-amber-300" />
                  )}
                </button>
              );
            })}
          </div>
        )}
      </div>
    </div>
  );
}

// Compact journal chip showing the settled d20, tinted by outcome (gold crit,
// green success, red fail). The motion now lives in the 3D DiceStage below; this
// is the persistent record the player scrolls back through.
function DiceRollBadge({ result }: { result: RollResult }) {
  const tone =
    result.crit === "success"
      ? "border-amber-300 text-amber-300"
      : result.crit === "fail"
        ? "border-red-500 text-red-400"
        : result.success
          ? "border-emerald-600 text-emerald-300"
          : "border-stone-600 text-stone-300";
  return (
    <span
      className={cn(
        "inline-flex size-7 shrink-0 items-center justify-center rounded-md border bg-stone-950 text-sm font-bold tabular-nums",
        tone,
      )}
    >
      {result.d20}
    </span>
  );
}

type DiceOutcome = "critSuccess" | "success" | "fail" | "critFail";
type DiceJob = { id: string; d20: number; outcome: DiceOutcome; caption: string };

// Per-outcome die colour (valid dice-box-threejs colorsets), backdrop glow and
// caption tint — a nat-20 blazes gold, a nat-1 turns to blood and skulls.
const DICE_COLORSET: Record<DiceOutcome, string> = {
  critSuccess: "radiant",
  success: "acid",
  fail: "fire",
  critFail: "necrotic",
};
// RGB triplets for a soft radial spotlight behind the die (smooth falloff to
// transparent — no hard-edged blob).
const DICE_GLOW: Record<DiceOutcome, string> = {
  critSuccess: "250, 204, 21",
  success: "52, 211, 153",
  fail: "234, 88, 12",
  critFail: "220, 38, 38",
};
const DICE_CAPTION_TONE: Record<DiceOutcome, string> = {
  critSuccess: "border-amber-300/60 text-amber-200",
  success: "border-emerald-400/50 text-emerald-200",
  fail: "border-orange-500/50 text-orange-200",
  critFail: "border-red-500/60 text-red-200",
};

function outcomeOf(result: RollResult): DiceOutcome {
  if (result.crit === "success") return "critSuccess";
  if (result.crit === "fail") return "critFail";
  return result.success ? "success" : "fail";
}

// Pull this turn's rolls out of the event stream so the 3D die can be forced to
// land on exactly what the engine already rolled, tinted by the outcome.
function rollJobsFromEvents(events: GameEvent[]): DiceJob[] {
  const jobs: DiceJob[] = [];
  for (const event of events) {
    if (event.kind !== "roll") continue;
    const result = (event.data as { result?: RollResult } | undefined)?.result;
    if (result && Number.isInteger(result.d20)) {
      jobs.push({
        id: event.id,
        d20: result.d20,
        outcome: outcomeOf(result),
        caption: event.text.replace(/^🎲\s*/, ""),
      });
    }
  }
  return jobs;
}

// Real 3D physics d20 (@3d-dice/dice-box-threejs, three.js + cannon-es). The cube
// is forced via `1d20@N` to land on the value the engine already rolled, recoloured
// per outcome and clattering on felt. WebGL loads lazily, only in D&D mode.
function DiceStage({
  job,
  onDone,
  sound,
  volume,
}: {
  job: DiceJob | null;
  onDone: (id: string) => void;
  sound: boolean;
  volume: number;
}) {
  const mountRef = useRef<HTMLDivElement>(null);
  const boxRef = useRef<DiceBox | null>(null);
  const [ready, setReady] = useState(false);
  const [active, setActive] = useState(false);
  const [landed, setLanded] = useState(false);
  const lastIdRef = useRef<string | null>(null);
  const onDoneRef = useRef(onDone);
  onDoneRef.current = onDone;

  useEffect(() => {
    let cancelled = false;
    // dice-box samples the container size at construction; wait until layout has
    // given the mount real dimensions, otherwise the world builds at 1×1 (invisible).
    const waitForSize = () =>
      new Promise<void>((resolve) => {
        const check = () => {
          if (cancelled) return resolve();
          const el = mountRef.current;
          if (el && el.clientWidth > 16 && el.clientHeight > 16) return resolve();
          requestAnimationFrame(check);
        };
        check();
      });
    void (async () => {
      try {
        const { default: DiceBoxCtor } = await import("@3d-dice/dice-box-threejs");
        await waitForSize();
        if (cancelled || !mountRef.current) return;
        const box = new DiceBoxCtor("#od-dice-stage", {
          assetPath: "/dice/",
          theme_colorset: "fire",
          theme_texture: "fire",
          theme_material: "metal",
          theme_surface: "green-felt",
          sound_dieMaterial: "metal",
          sounds: true,
          volume: sound ? volume : 0,
          shadows: true,
          gravity_multiplier: 260,
          light_intensity: 0.95,
          baseScale: 175,
          strength: 2.6,
        });
        await box.initialize();
        if (cancelled) return;
        boxRef.current = box;
        if (process.env.NODE_ENV !== "production") {
          (window as unknown as { __odDiceBox?: unknown }).__odDiceBox = box;
        }
        setReady(true);
      } catch (error) {
        console.error("[dice] init failed", error);
      }
    })();
    return () => {
      cancelled = true;
      try {
        boxRef.current?.clearDice();
      } catch {
        // ignore teardown races
      }
      boxRef.current = null;
    };
  }, []);

  useEffect(() => {
    const box = boxRef.current;
    if (!ready || !box || !job || lastIdRef.current === job.id) return;
    lastIdRef.current = job.id;
    setActive(true);
    setLanded(false);
    let settled = false;
    const finish = () => {
      if (settled) return;
      settled = true;
      setLanded(true);
      // hold the landed die + verdict a beat so the player reads it, then fade
      window.setTimeout(() => {
        setActive(false);
        onDoneRef.current(job.id);
      }, 2600);
    };
    void (async () => {
      try {
        await box.loadTheme({ colorset: DICE_COLORSET[job.outcome], texture: "", material: "metal" });
      } catch {
        // keep the current theme if the recolour fails
      }
      box.roll(`1d20@${job.d20}`).then(finish).catch(finish);
    })();
  }, [ready, job]);

  // apply sound/volume changes live (sounds stay loaded; 0 = muted)
  useEffect(() => {
    if (boxRef.current) boxRef.current.volume = sound ? volume : 0;
  }, [sound, volume]);

  const outcome = job?.outcome ?? "fail";
  const critical = outcome === "critSuccess" || outcome === "critFail";
  return (
    <div
      aria-hidden
      className={cn(
        "pointer-events-none fixed inset-0 z-[60] flex items-center justify-center transition-opacity duration-300",
        active ? "opacity-100" : "opacity-0",
      )}
    >
      <div className="absolute inset-0 bg-black/65" />
      <div
        className={cn(
          "pointer-events-none absolute inset-0 transition-opacity duration-700",
          landed ? (critical ? "opacity-100" : "opacity-70") : "opacity-0",
        )}
        style={{
          background: `radial-gradient(circle at 50% 45%, rgba(${DICE_GLOW[outcome]}, ${
            critical ? 0.32 : 0.22
          }) 0%, transparent 58%)`,
        }}
      />
      <div
        ref={mountRef}
        id="od-dice-stage"
        className="relative"
        style={{ width: "min(94vw, 1120px)", height: "min(62vh, 520px)" }}
      />
      {job && (
        <div
          className={cn(
            "absolute bottom-[12%] max-w-[86vw] rounded-lg border bg-stone-950/80 px-4 py-2 text-center font-serif text-sm shadow-lg backdrop-blur-sm transition-all duration-500 sm:text-base",
            DICE_CAPTION_TONE[outcome],
            landed ? "translate-y-0 opacity-100" : "translate-y-3 opacity-0",
          )}
        >
          {job.caption}
        </div>
      )}
    </div>
  );
}

function DiceButton({
  field,
  settings,
  onValue,
  context,
}: {
  field: "world" | "style" | "character" | "opening";
  settings: StorySettings;
  onValue: (value: string) => void;
  context?: string;
}) {
  const [loading, setLoading] = useState(false);
  const roll = async () => {
    setLoading(true);
    try {
      const response = await fetch("/api/suggest", {
        method: "POST",
        headers: { "Content-Type": "application/json" },
        body: JSON.stringify({ field, context, settings }),
      });
      const data = (await response.json()) as { value?: string };
      if (response.ok && data.value) {
        onValue(data.value);
      }
    } catch {
      // best-effort: leave the field untouched on failure
    } finally {
      setLoading(false);
    }
  };

  return (
    <button
      type="button"
      onClick={roll}
      disabled={loading}
      title="Придумать за меня"
      className="inline-flex items-center gap-1 rounded border border-stone-700 px-1.5 py-0.5 text-[10px] font-normal normal-case text-stone-400 transition hover:border-amber-300 hover:text-amber-300 disabled:opacity-50"
    >
      <Dices size={13} className={loading ? "animate-spin" : ""} />
      {loading ? "…" : "Идея"}
    </button>
  );
}

function StorySettingsPanel({
  settings,
  setSettings,
  compact = false,
  open,
  onOpenChange,
  divided,
}: {
  settings: StorySettings;
  setSettings: Dispatch<SetStateAction<StorySettings>>;
  compact?: boolean;
} & PanelControlProps) {
  const idPrefix = compact ? "mobile" : "desktop";

  return (
    <PanelSection
      icon={Settings2}
      iconSrc={SIDEBAR_ICONS.story}
      title="История"
      compact={compact}
      defaultOpen={compact}
      open={open}
      onOpenChange={onOpenChange}
      divided={divided}
    >
      {compact && (
        <>
          <FontSizeSlider settings={settings} setSettings={setSettings} idPrefix={idPrefix} />
          <ResponseLengthControl settings={settings} setSettings={setSettings} idPrefix={idPrefix} />
          <VoiceControl settings={settings} setSettings={setSettings} />
        </>
      )}
      <label className="mb-1 flex cursor-pointer items-center justify-between gap-3 rounded border border-stone-800 bg-stone-950 px-3 py-2">
        <span className="text-xs font-medium uppercase text-stone-400">⚔️ Режим D&D (статы, кубик, журнал)</span>
        <input
          type="checkbox"
          checked={settings.rpgEnabled}
          onChange={(event) =>
            setSettings((current) => ({ ...current, rpgEnabled: event.target.checked }))
          }
          className="size-4 accent-amber-300"
        />
      </label>
      {settings.rpgEnabled && (
        <div className="mb-1 space-y-2 rounded border border-stone-800 bg-stone-950/60 px-3 py-2">
          <div className="text-[10px] font-medium uppercase tracking-wide text-stone-500">🎲 Кубики</div>
          <label className="flex cursor-pointer items-center justify-between gap-3">
            <span className="text-xs text-stone-300">3D-бросок</span>
            <input
              type="checkbox"
              checked={settings.diceEnabled}
              onChange={(event) =>
                setSettings((current) => ({ ...current, diceEnabled: event.target.checked }))
              }
              className="size-4 accent-amber-300"
            />
          </label>
          <label className="flex cursor-pointer items-center justify-between gap-3">
            <span className={cn("text-xs", settings.diceEnabled ? "text-stone-300" : "text-stone-600")}>
              Звук броска
            </span>
            <input
              type="checkbox"
              checked={settings.diceSound}
              disabled={!settings.diceEnabled}
              onChange={(event) =>
                setSettings((current) => ({ ...current, diceSound: event.target.checked }))
              }
              className="size-4 accent-amber-300 disabled:opacity-40"
            />
          </label>
          <label className="block">
            <span className="mb-1 flex items-center justify-between text-xs">
              <span className={settings.diceEnabled && settings.diceSound ? "text-stone-300" : "text-stone-600"}>
                Громкость
              </span>
              <span className="tabular-nums text-stone-500">{settings.diceVolume}%</span>
            </span>
            <input
              type="range"
              min={0}
              max={100}
              step={5}
              value={settings.diceVolume}
              disabled={!settings.diceEnabled || !settings.diceSound}
              onChange={(event) =>
                setSettings((current) => ({ ...current, diceVolume: Number(event.target.value) }))
              }
              className="w-full accent-amber-300 disabled:opacity-40"
            />
          </label>
        </div>
      )}
      <div className="mb-1 space-y-2 rounded border border-stone-800 bg-stone-950/60 px-3 py-2">
        <div className="text-[10px] font-medium uppercase tracking-wide text-stone-500">
          ✨ Повествование
        </div>
        <label className="flex cursor-pointer items-center justify-between gap-3">
          <span className="text-xs text-stone-300">Избегать повторов сцен</span>
          <input
            type="checkbox"
            checked={settings.antiRepetition}
            onChange={(event) =>
              setSettings((current) => ({ ...current, antiRepetition: event.target.checked }))
            }
            className="size-4 accent-amber-300"
          />
        </label>
        <label className="flex cursor-pointer items-center justify-between gap-3">
          <span className="text-xs text-stone-300">Осмысленная концовка</span>
          <input
            type="checkbox"
            checked={settings.causeAwareEnding}
            onChange={(event) =>
              setSettings((current) => ({ ...current, causeAwareEnding: event.target.checked }))
            }
            className="size-4 accent-amber-300"
          />
        </label>
        <label className="flex cursor-pointer items-center justify-between gap-3">
          <span className="text-xs text-stone-300">Разные голоса персонажей</span>
          <input
            type="checkbox"
            checked={settings.multiVoice}
            onChange={(event) =>
              setSettings((current) => ({ ...current, multiVoice: event.target.checked }))
            }
            className="size-4 accent-amber-300"
          />
        </label>
        <label className="flex cursor-pointer items-center justify-between gap-3">
          <span className="text-xs text-stone-300">Спутник-комментатор</span>
          <input
            type="checkbox"
            checked={settings.companion}
            onChange={(event) =>
              setSettings((current) => ({ ...current, companion: event.target.checked }))
            }
            className="size-4 accent-amber-300"
          />
        </label>
        <label className="block">
          <span className="mb-1 block text-xs text-stone-300">Стиль картинок (префикс)</span>
          <input
            type="text"
            value={settings.imageStylePrefix}
            onChange={(event) =>
              setSettings((current) => ({ ...current, imageStylePrefix: event.target.value }))
            }
            placeholder="напр. dark fantasy, масло, приглушённая палитра"
            className="w-full rounded border border-stone-700 bg-stone-950 px-2 py-1 text-xs text-stone-200 placeholder:text-stone-600"
          />
        </label>
      </div>
      <label className="block">
        <span className="mb-2 flex items-center justify-between text-xs font-medium uppercase text-stone-500">
          Мир
          <DiceButton
            field="world"
            settings={settings}
            context={settings.style}
            onValue={(value) => setSettings((current) => ({ ...current, world: value }))}
          />
        </span>
        <textarea
          id={`${idPrefix}-story-world`}
          name={`${idPrefix}-story-world`}
          value={settings.world}
          onChange={(event) =>
            setSettings((current) => ({ ...current, world: event.target.value }))
          }
          rows={compact ? 4 : 5}
          className="w-full resize-none rounded border border-stone-800 bg-stone-950 px-3 py-2 text-sm text-stone-200 outline-none focus:border-amber-300"
        />
      </label>
      <label className="block">
        <span className="mb-2 flex items-center justify-between text-xs font-medium uppercase text-stone-500">
          Стиль
          <DiceButton
            field="style"
            settings={settings}
            context={settings.world}
            onValue={(value) => setSettings((current) => ({ ...current, style: value }))}
          />
        </span>
        <textarea
          id={`${idPrefix}-story-style`}
          name={`${idPrefix}-story-style`}
          value={settings.style}
          onChange={(event) =>
            setSettings((current) => ({ ...current, style: event.target.value }))
          }
          rows={compact ? 3 : 4}
          className="w-full resize-none rounded border border-stone-800 bg-stone-950 px-3 py-2 text-sm text-stone-200 outline-none focus:border-amber-300"
        />
      </label>
    </PanelSection>
  );
}

function ImageSettingsPanel({
  settings,
  setSettings,
  onImageGenerationEnabledChange,
  imageWorkerStatus,
  imageWorkerBusy,
  imageWorkerMessage,
  onStartImageWorker,
  onOpenImageModelFolder,
  compact = false,
  open,
  onOpenChange,
  divided,
}: {
  settings: StorySettings;
  setSettings: Dispatch<SetStateAction<StorySettings>>;
  onImageGenerationEnabledChange: (enabled: boolean) => void;
  imageWorkerStatus: ImageWorkerStatus | null;
  imageWorkerBusy: boolean;
  imageWorkerMessage: string;
  onStartImageWorker: () => void;
  onOpenImageModelFolder: () => void;
  compact?: boolean;
} & PanelControlProps) {
  const idPrefix = compact ? "mobile" : "desktop";
  const imageControlsDisabled = !settings.imageGenerationEnabled;
  const workerRunning = Boolean(imageWorkerStatus?.ok);
  const workerStatusLabel = workerRunning ? "Сервер работает" : "Сервер остановлен";
  const workerDetail = workerRunning
    ? imageWorkerStatus?.defaultBackend
      ? `Бэкенд по умолчанию: ${imageWorkerStatus.defaultBackend}`
      : "Готов к локальной генерации"
    : "Запусти отсюда, если картинки не сгенерировались.";

  return (
    <PanelSection
      icon={Aperture}
      iconSrc={SIDEBAR_ICONS.images}
      title="Изображения"
      compact={compact}
      defaultOpen={compact}
      open={open}
      onOpenChange={onOpenChange}
      divided={divided}
    >
      <label className="flex items-center justify-between rounded border border-stone-800 bg-stone-950 px-3 py-2 text-sm text-stone-300">
        Генерация изображений
        <input
          id={`${idPrefix}-image-generation-enabled`}
          name={`${idPrefix}-image-generation-enabled`}
          type="checkbox"
          checked={settings.imageGenerationEnabled}
          onChange={(event) => onImageGenerationEnabledChange(event.target.checked)}
          className="size-4 accent-amber-200"
        />
      </label>
      <div className="space-y-3 rounded border border-stone-800 bg-stone-950 px-3 py-3">
        <div>
          <p className="text-sm font-medium text-stone-200">{workerStatusLabel}</p>
          <p className="mt-1 text-xs leading-relaxed text-stone-500">{workerDetail}</p>
        </div>
        <div className="grid grid-cols-2 gap-2">
          <button
            type="button"
            disabled={imageControlsDisabled || imageWorkerBusy}
            onClick={onStartImageWorker}
            className="inline-flex min-h-10 items-center justify-center gap-2 rounded border border-stone-700 bg-stone-900/50 px-2 text-sm text-stone-200 hover:border-amber-700/60 hover:bg-stone-900 disabled:cursor-not-allowed disabled:border-stone-800 disabled:text-stone-600"
          >
            {imageWorkerBusy ? (
              <Loader2 className="size-4 animate-spin" aria-hidden="true" />
            ) : (
              <Play className="size-4" aria-hidden="true" />
            )}
            Start
          </button>
          <button
            type="button"
            disabled={imageWorkerBusy}
            onClick={onOpenImageModelFolder}
            className="inline-flex min-h-10 items-center justify-center gap-2 rounded border border-stone-700 bg-stone-900/50 px-2 text-sm text-stone-200 hover:border-amber-700/60 hover:bg-stone-900 disabled:cursor-not-allowed disabled:border-stone-800 disabled:text-stone-600"
          >
            <FolderOpen className="size-4" aria-hidden="true" />
            Модели
          </button>
        </div>
        {imageWorkerMessage && (
          <p className="text-xs leading-relaxed text-stone-500">{imageWorkerMessage}</p>
        )}
      </div>
      <div className="space-y-2">
        <span className="block text-xs font-medium uppercase text-stone-500">Бэкенд</span>
        <Segmented<ImageBackend>
          value={settings.imageBackend}
          options={[
            { value: "mflux-hs", label: "MFLUX Mac" },
            { value: "sdnq-hs", label: "SDNQ CUDA/CPU" },
          ]}
          disabled={imageControlsDisabled}
          onChange={(imageBackend) =>
            setSettings((current) => ({
              ...current,
              imageBackend,
            }))
          }
        />
      </div>
      <div className="space-y-2">
        <span className="block text-xs font-medium uppercase text-stone-500">Размер</span>
        <Segmented<ImageMode>
          value={settings.imageMode}
          options={[
            { value: "fast", label: "1024" },
            { value: "slow", label: "2048" },
          ]}
          disabled={imageControlsDisabled}
          onChange={(imageMode) => setSettings((current) => ({ ...current, imageMode }))}
        />
      </div>
      <div className="space-y-2">
        <span className="block text-xs font-medium uppercase text-stone-500">Соотношение</span>
        <Segmented<AspectPreset>
          value={settings.aspect}
          options={[
            { value: "square", label: "Square" },
            { value: "portrait", label: "Portrait" },
            { value: "landscape", label: "Landscape" },
          ]}
          disabled={imageControlsDisabled}
          onChange={(aspect) => setSettings((current) => ({ ...current, aspect }))}
        />
      </div>
      <label
        className={cn(
          "flex items-center justify-between rounded border border-stone-800 bg-stone-950 px-3 py-2 text-sm text-stone-300",
          imageControlsDisabled && "text-stone-600",
        )}
      >
        Авто-картинки
        <input
          id={`${idPrefix}-auto-images`}
          name={`${idPrefix}-auto-images`}
          type="checkbox"
          checked={settings.autoImages}
          disabled={imageControlsDisabled}
          onChange={(event) =>
            setSettings((current) => ({ ...current, autoImages: event.target.checked }))
          }
          className="size-4 accent-amber-200 disabled:accent-stone-700"
        />
      </label>
      <label className="block">
        <span className="mb-2 block text-xs font-medium uppercase text-stone-500">
          Промпт изображений
        </span>
        <textarea
          id={`${idPrefix}-image-prompt`}
          name={`${idPrefix}-image-prompt`}
          value={settings.imagePrompt}
          disabled={imageControlsDisabled}
          onChange={(event) =>
            setSettings((current) => ({ ...current, imagePrompt: event.target.value }))
          }
          rows={compact ? 6 : 10}
          spellCheck={false}
          className="w-full resize-y rounded border border-stone-800 bg-stone-950 px-3 py-2 font-mono text-xs leading-relaxed text-stone-200 outline-none focus:border-amber-300 disabled:cursor-not-allowed disabled:text-stone-600"
        />
        <span className="mt-1 block text-[11px] text-stone-600">
          Инструкция модели по генерации изображений (FLUX-промпт остаётся на английском). Пусто — встроенный по умолчанию.
        </span>
      </label>
    </PanelSection>
  );
}

function LocalDataPanel({
  clearing,
  onClear,
  compact = false,
  open,
  onOpenChange,
  divided,
}: {
  clearing: boolean;
  onClear: () => void;
  compact?: boolean;
} & PanelControlProps) {
  return (
    <PanelSection
      icon={Trash2}
      iconSrc={SIDEBAR_ICONS.localData}
      title="Локальные данные"
      compact={compact}
      defaultOpen={compact}
      open={open}
      onOpenChange={onOpenChange}
      divided={divided}
    >
      <ClearLocalDataDialog onConfirm={onClear}>
        <button
          type="button"
          disabled={clearing}
          className="inline-flex w-full items-center justify-center gap-2 rounded border border-red-900/80 bg-red-950/20 px-3 py-2 text-sm font-medium text-red-200 hover:bg-red-950/40 disabled:cursor-not-allowed disabled:border-stone-800 disabled:text-stone-600"
        >
          {clearing ? (
            <Loader2 className="size-4 animate-spin" aria-hidden="true" />
          ) : (
            <Trash2 className="size-4" aria-hidden="true" />
          )}
          Удалить все локальные данные
        </button>
      </ClearLocalDataDialog>
    </PanelSection>
  );
}

function SupportPanel({ open, onOpenChange, divided }: PanelControlProps) {
  return (
    <PanelSection
      icon={Heart}
      iconSrc={SIDEBAR_ICONS.support}
      title="Поддержка"
      open={open}
      onOpenChange={onOpenChange}
      divided={divided}
    >
      <p className="text-pretty text-xs leading-relaxed text-stone-500">
        Open Dungeon is free and open source. If it earns a spot on your machine, a tip
        keeps development going.
      </p>
      <div className="flex flex-col gap-2">
        <a
          href="https://github.com/sponsors/newideas99"
          target="_blank"
          rel="noreferrer noopener"
          className="inline-flex w-full items-center justify-center gap-2 rounded border border-stone-700 bg-stone-900/40 px-3 py-2 text-sm font-medium text-stone-200 hover:border-amber-700/60 hover:bg-stone-900"
        >
          <Heart className="size-4 text-amber-200" aria-hidden="true" />
          Поддержать на GitHub
        </a>
        <a
          href="https://ko-fi.com/opendungeon"
          target="_blank"
          rel="noreferrer noopener"
          className="inline-flex w-full items-center justify-center gap-2 rounded border border-stone-700 bg-stone-900/40 px-3 py-2 text-sm font-medium text-stone-200 hover:border-amber-700/60 hover:bg-stone-900"
        >
          Поддержать на Ko-fi
        </a>
      </div>
    </PanelSection>
  );
}

function ClearLocalDataDialog({
  onConfirm,
  children,
}: {
  onConfirm: () => void;
  children: ReactNode;
}) {
  return (
    <AlertDialog.Root>
      <AlertDialog.Trigger asChild>{children}</AlertDialog.Trigger>
      <AlertDialog.Portal>
        <AlertDialog.Overlay className="fixed inset-0 z-40 bg-stone-950/80" />
        <AlertDialog.Content className="fixed left-1/2 top-1/2 z-50 w-[min(calc(100vw-2rem),440px)] -translate-x-1/2 -translate-y-1/2 rounded-xl border border-red-900/80 bg-[#130d09] p-5 shadow-xl">
          <AlertDialog.Title className="text-balance text-base font-semibold text-red-100">
            Полностью очистить приложение?
          </AlertDialog.Title>
          <AlertDialog.Description className="mt-2 text-pretty text-sm text-stone-400">
            Удалит все локальные истории, сообщения, персонажей, загруженные фото, сгенерированные
            картинки и временные файлы-референсы с этого компьютера.
          </AlertDialog.Description>
          <div className="mt-5 flex justify-end gap-2">
            <AlertDialog.Cancel asChild>
              <button
                type="button"
                className="rounded border border-stone-700 px-3 py-2 text-sm text-stone-300 hover:bg-stone-900"
              >
                Отмена
              </button>
            </AlertDialog.Cancel>
            <AlertDialog.Action asChild>
              <button
                type="button"
                onClick={onConfirm}
                className="rounded bg-red-300 px-3 py-2 text-sm font-medium text-red-950 hover:bg-red-200"
              >
                Удалить всё
              </button>
            </AlertDialog.Action>
          </div>
        </AlertDialog.Content>
      </AlertDialog.Portal>
    </AlertDialog.Root>
  );
}

function DeleteChatDialog({
  chat,
  onConfirm,
  children,
}: {
  chat: StoryChatSummary;
  onConfirm: () => void;
  children: ReactNode;
}) {
  return (
    <AlertDialog.Root>
      <AlertDialog.Trigger asChild>{children}</AlertDialog.Trigger>
      <AlertDialog.Portal>
        <AlertDialog.Overlay className="fixed inset-0 z-40 bg-stone-950/80" />
        <AlertDialog.Content className="fixed left-1/2 top-1/2 z-50 w-[min(calc(100vw-2rem),420px)] -translate-x-1/2 -translate-y-1/2 rounded-xl border border-stone-700 bg-[#130d09] p-5 shadow-xl">
          <AlertDialog.Title className="text-balance text-base font-semibold text-stone-100">
            Удалить эту историю?
          </AlertDialog.Title>
          <AlertDialog.Description className="mt-2 text-pretty text-sm text-stone-400">
            {chat.title} и его сохранённые сообщения будут удалены из локальной базы.
          </AlertDialog.Description>
          <div className="mt-5 flex justify-end gap-2">
            <AlertDialog.Cancel asChild>
              <button
                type="button"
                className="rounded border border-stone-700 px-3 py-2 text-sm text-stone-300 hover:bg-stone-900"
              >
                Отмена
              </button>
            </AlertDialog.Cancel>
            <AlertDialog.Action asChild>
              <button
                type="button"
                onClick={onConfirm}
                className="rounded bg-red-300 px-3 py-2 text-sm font-medium text-red-950 hover:bg-red-200"
              >
                Удалить
              </button>
            </AlertDialog.Action>
          </div>
        </AlertDialog.Content>
      </AlertDialog.Portal>
    </AlertDialog.Root>
  );
}

function StoryActionButton({
  icon: Icon,
  label,
  title,
  disabled,
  onClick,
}: {
  icon: typeof RotateCcw;
  label: string;
  title: string;
  disabled?: boolean;
  onClick: () => void;
}) {
  return (
    <button
      type="button"
      title={title}
      onClick={onClick}
      disabled={disabled}
      className="inline-flex items-center gap-1.5 rounded-lg border border-stone-800 bg-stone-950 px-3 py-1.5 text-xs font-medium text-stone-300 hover:bg-stone-900 hover:text-stone-100 disabled:cursor-not-allowed disabled:text-stone-600"
    >
      <Icon className="size-3.5" aria-hidden="true" />
      {label}
    </button>
  );
}

function MessageActions({
  align,
  disabled,
  onEdit,
  onSpeak,
  speaking,
}: {
  align: "start" | "end";
  disabled?: boolean;
  onEdit: () => void;
  onSpeak?: () => void;
  speaking?: boolean;
}) {
  return (
    <div
      className={cn(
        "mt-1 flex gap-1 opacity-0 transition-opacity group-hover:opacity-100 focus-within:opacity-100",
        align === "end" ? "justify-end" : "justify-start",
      )}
    >
      {onSpeak && (
        <button
          type="button"
          onClick={onSpeak}
          aria-label={speaking ? "Остановить озвучку" : "Озвучить"}
          title={speaking ? "Остановить озвучку" : "Озвучить"}
          className="inline-flex items-center gap-1 rounded-md px-2 py-1 text-xs text-stone-500 hover:bg-stone-900 hover:text-stone-200"
        >
          {speaking ? (
            <X className="size-3.5" aria-hidden="true" />
          ) : (
            <Play className="size-3.5" aria-hidden="true" />
          )}
          {speaking ? "Стоп" : "Озвучить"}
        </button>
      )}
      <button
        type="button"
        onClick={onEdit}
        disabled={disabled}
        aria-label="Изменить"
        title="Изменить"
        className="inline-flex items-center gap-1 rounded-md px-2 py-1 text-xs text-stone-500 hover:bg-stone-900 hover:text-stone-200 disabled:cursor-not-allowed"
      >
        <Pencil className="size-3.5" aria-hidden="true" />
        Изменить
      </button>
    </div>
  );
}

function MessageEditor({
  message,
  value,
  onChange,
  onSave,
  onCancel,
}: {
  message: StoryMessage;
  value: string;
  onChange: (value: string) => void;
  onSave: () => void;
  onCancel: () => void;
}) {
  return (
    <div className={cn("flex flex-col gap-2", message.role === "user" && "items-end")}>
      <textarea
        autoFocus
        value={value}
        onChange={(event) => onChange(event.target.value)}
        onKeyDown={(event) => {
          if (event.key === "Enter" && (event.metaKey || event.ctrlKey)) {
            onSave();
          }
          if (event.key === "Escape") {
            onCancel();
          }
        }}
        rows={Math.min(12, Math.max(3, value.split("\n").length + 1))}
        className="w-full resize-none rounded-xl border border-amber-300/50 bg-stone-950 px-4 py-3 text-base text-stone-100 outline-none"
      />
      <div className="flex gap-2">
        <button
          type="button"
          onClick={onCancel}
          className="rounded-lg border border-stone-700 px-3 py-1.5 text-xs text-stone-300 hover:bg-stone-900"
        >
          Отмена
        </button>
        <button
          type="button"
          onClick={onSave}
          className="inline-flex items-center gap-1.5 rounded-lg bg-amber-200 px-3 py-1.5 text-xs font-medium text-stone-950 hover:bg-amber-100"
        >
          <Check className="size-3.5" aria-hidden="true" />
          Сохранить
        </button>
      </div>
    </div>
  );
}

function StorySkeleton() {
  return (
    <div className="space-y-6">
      <div className="space-y-3">
        <div className="h-4 w-11/12 rounded bg-stone-900" />
        <div className="h-4 w-10/12 rounded bg-stone-900" />
        <div className="h-4 w-2/3 rounded bg-stone-900" />
      </div>
      <div className="ml-auto h-20 w-2/3 rounded border border-stone-800 bg-stone-950" />
    </div>
  );
}

function AttachmentStrip({
  attachments,
  className,
  onRemove,
}: {
  attachments: Attachment[];
  className?: string;
  onRemove?: (id: string) => void;
}) {
  return (
    <div className={cn("flex flex-wrap gap-2", className)}>
      {attachments.map((attachment) => (
        <div
          key={attachment.id}
          className="relative flex items-center gap-2 rounded border border-stone-700 bg-stone-950/80 p-1 pr-2 text-xs text-stone-400"
        >
          {/* eslint-disable-next-line @next/next/no-img-element */}
          <img src={attachment.url} alt="" className="size-12 rounded object-cover" />
          <span className="max-w-36 truncate">{attachment.name}</span>
          {onRemove && (
            <button
              type="button"
              aria-label={`Remove ${attachment.name}`}
              onClick={() => onRemove(attachment.id)}
              className="flex size-6 items-center justify-center rounded text-stone-400 hover:bg-stone-800 hover:text-stone-100"
            >
              <X className="size-3.5" aria-hidden="true" />
            </button>
          )}
        </div>
      ))}
    </div>
  );
}

function ImageBeat({
  message,
  status,
  onRetry,
}: {
  message: StoryMessage;
  status?: "loading" | "error";
  onRetry: () => void;
}) {
  const isLoading = status === "loading";
  const isError = status === "error";
  const [promptExpanded, setPromptExpanded] = useState(false);

  if (message.generatedImage) {
    return (
      <figure className="mt-6 overflow-hidden rounded-xl border border-stone-800 bg-stone-950">
        {/* eslint-disable-next-line @next/next/no-img-element */}
        <img
          src={message.generatedImage.url}
          alt={message.generatedImage.prompt}
          className="ken-burns max-h-[720px] w-full object-contain"
        />
        <figcaption className="border-t border-stone-800 px-3 py-2 font-sans text-xs text-stone-500">
          <div className="flex items-start justify-between gap-3">
            <p className={cn("min-w-0 leading-5", promptExpanded ? "" : "line-clamp-2")}>
              {message.generatedImage.prompt}
            </p>
            <span className="shrink-0 pt-0.5 tabular-nums">
              {message.generatedImage.backend ? `${message.generatedImage.backend} · ` : ""}
              {message.generatedImage.width}×{message.generatedImage.height}
            </span>
          </div>
          {message.generatedImage.prompt.length > 120 && (
            <button
              type="button"
              onClick={() => setPromptExpanded((value) => !value)}
              className="mt-1 text-xs text-amber-200 hover:text-amber-100"
            >
              {promptExpanded ? "Свернуть" : "Показать больше"}
            </button>
          )}
        </figcaption>
      </figure>
    );
  }

  const pendingPrompt = message.imageRequest?.prompt;

  return (
    <div className="mt-6 rounded border border-stone-800 bg-stone-950 px-4 py-3 font-sans text-sm text-stone-400">
      <div className="flex items-center gap-3">
        {isError ? (
          <ImagePlus className="size-4 text-red-300" aria-hidden="true" />
        ) : isLoading ? (
          <Loader2 className="size-4 animate-spin text-amber-200" aria-hidden="true" />
        ) : (
          <ImagePlus className="size-4 text-amber-200" aria-hidden="true" />
        )}
        <span>
          {isError
            ? "Ошибка инструмента изображений."
            : isLoading
              ? "Генерирую сцену..."
              : "Запрошен инструмент изображений."}
        </span>
        {!isLoading && (
          <button
            type="button"
            onClick={onRetry}
            className="ml-auto rounded border border-stone-700 px-2 py-1 text-xs text-stone-300 hover:bg-stone-900"
          >
            {isError ? "Повторить" : "Сгенерировать"}
          </button>
        )}
      </div>
      {pendingPrompt && (
        <div className="mt-2">
          <p className={cn("text-xs leading-5 text-stone-600", promptExpanded ? "" : "line-clamp-2")}>
            {pendingPrompt}
          </p>
          {pendingPrompt.length > 120 && (
            <button
              type="button"
              onClick={() => setPromptExpanded((value) => !value)}
              className="mt-1 text-xs text-amber-200 hover:text-amber-100"
            >
              {promptExpanded ? "Свернуть" : "Показать больше"}
            </button>
          )}
        </div>
      )}
    </div>
  );
}

function PanelSection({
  icon,
  iconSrc,
  title,
  children,
  action,
  compact = false,
  defaultOpen = false,
  open: controlledOpen,
  onOpenChange,
  divided = true,
  fill = false,
}: {
  icon: typeof Sparkles;
  iconSrc?: string;
  title: string;
  children: ReactNode;
  action?: ReactNode;
  compact?: boolean;
  defaultOpen?: boolean;
  open?: boolean;
  onOpenChange?: (open: boolean) => void;
  divided?: boolean;
  fill?: boolean;
}) {
  const [uncontrolledOpen, setUncontrolledOpen] = useState(defaultOpen);
  const open = controlledOpen ?? uncontrolledOpen;

  function toggleOpen() {
    const nextOpen = !open;
    if (onOpenChange) {
      onOpenChange(nextOpen);
      return;
    }
    setUncontrolledOpen(nextOpen);
  }

  return (
    <section
      className={cn(
        "space-y-2.5",
        fill && "flex min-h-0 flex-1 flex-col",
        divided && !compact && "border-t border-stone-800 pt-2.5",
      )}
    >
      <div className="flex items-center gap-2">
        <button
          type="button"
          aria-expanded={open}
          onClick={toggleOpen}
          className={cn(
            "group flex min-w-0 flex-1 items-center gap-3 rounded border border-stone-800 bg-stone-950/70 p-2 text-left transition hover:border-amber-800/60 hover:bg-stone-900/70",
            open && "border-amber-900/70 bg-stone-900/60",
          )}
        >
          <PanelTitle icon={icon} iconSrc={iconSrc} title={title} />
          <ChevronRight
            className={cn(
              "ml-auto size-4 shrink-0 text-stone-500 transition group-hover:text-amber-200/80",
              open && "rotate-90 text-amber-200",
            )}
            aria-hidden="true"
          />
        </button>
        {action}
      </div>
      {open && (
        <div className={cn("space-y-3", fill && "flex min-h-0 flex-1 flex-col")}>
          {children}
        </div>
      )}
    </section>
  );
}

function PanelTitle({
  icon: Icon,
  iconSrc,
  title,
}: {
  icon: typeof Sparkles;
  iconSrc?: string;
  title: string;
}) {
  return (
    <div className="flex min-w-0 items-center gap-3 text-sm font-medium text-stone-300">
      {iconSrc ? (
        <span className="flex size-12 shrink-0 items-center justify-center overflow-hidden rounded-lg border border-amber-200/15 bg-stone-950 shadow-[0_0_16px_rgba(251,191,36,0.1)]">
          {/* eslint-disable-next-line @next/next/no-img-element */}
          <img src={iconSrc} alt="" className="size-full object-cover" />
        </span>
      ) : (
        <span className="flex size-7 shrink-0 items-center justify-center rounded border border-amber-200/15 bg-amber-200/10">
          <Icon className="size-4 text-amber-200" aria-hidden="true" />
        </span>
      )}
      <span className="truncate">{title}</span>
    </div>
  );
}

function Segmented<T extends string>({
  value,
  options,
  onChange,
  disabled = false,
}: {
  value: T;
  options: Array<{ value: T; label: string }>;
  onChange: (value: T) => void;
  disabled?: boolean;
}) {
  return (
    <div
      className={cn(
        "grid gap-1 rounded border border-stone-800 bg-stone-950 p-1",
        options.length === 2 ? "grid-cols-2" : "grid-cols-3",
      )}
    >
      {options.map((option) => {
        const selected = option.value === value;
        return (
          <button
            key={option.value}
            type="button"
            disabled={disabled}
            onClick={() => onChange(option.value)}
            className={cn(
              "inline-flex items-center justify-center gap-1 whitespace-nowrap rounded-md px-2 py-1.5 text-xs text-stone-400 hover:bg-stone-900",
              selected && "bg-stone-800 text-stone-100",
              disabled && "cursor-not-allowed text-stone-700 hover:bg-transparent",
            )}
          >
            {selected && <Check className="size-3" aria-hidden="true" />}
            {option.label}
          </button>
        );
      })}
    </div>
  );
}
