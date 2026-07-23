import {
  AlertCircle,
  BadgeAlert,
  BadgeCheck,
  BarChart3,
  BookOpenText,
  ChevronDown,
  Clock3,
  Eye,
  EyeOff,
  FolderOpen,
  FolderPlus,
  GitBranch,
  Hash,
  Keyboard,
  Loader2,
  MessagesSquare,
  Moon,
  Pin,
  RotateCw,
  Search,
  SlidersHorizontal,
  SquarePen,
  SquareTerminal,
  Star,
  StickyNote,
  StickyNotePlus,
  Sun,
  TagPlus,
  Tags,
  TextSearch,
  Trash,
  X,
  type LucideIcon,
} from "lucide-react";
import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import { Badge } from "./components/ui/badge";
import { Button } from "./components/ui/button";
import { Card } from "./components/ui/card";
import { Input } from "./components/ui/input";
import { api } from "./lib/api";
import appIcon from "../src-tauri/icons/icon.png";
import type {
  AppSettings,
  CollectionColorName,
  ProviderStatus,
  SessionHistory,
  SessionMessage,
  SessionRecord,
  SessionSearchResult,
} from "./lib/types";
import { cn, formatModifiedTime } from "./lib/utils";

const defaultSettings: AppSettings = {
  theme: "dark",
  terminalExecutable: null,
  providerFilter: "all",
  showHiddenSessions: false,
};

type ToastState = {
  id: number;
  title: string;
  description: string;
  tone: "success" | "error";
};

type SessionView = "all" | "pinned";
type ActivityFilter =
  | "all"
  | "today"
  | "yesterday"
  | "last7"
  | "last30"
  | "resumed";

type SessionStatistics = {
  totalSessions: number;
  providerCounts: Array<{
    id: string;
    label: string;
    count: number;
  }>;
  pinnedSessions: number;
  hiddenSessions: number;
  collectionCount: number;
  tagCount: number;
};

type CollectionColorChoice = CollectionColorName | "none";

type CollectionColorStyle = {
  dotClassName: string;
  topBarClassName: string;
  swatchClassName: string;
};

type AdvancedSearch = {
  text: string;
  providers: string[];
  collections: string[];
  tags: string[];
  pinned: boolean | null;
  hidden: boolean | null;
  folder: string | null;
  before: number | null;
  after: number | null;
};

type CommandPaletteCommand = {
  id: string;
  label: string;
  detail?: string;
  keywords?: string[];
  icon: LucideIcon;
  disabled?: boolean;
  perform: () => void | Promise<void>;
};

const allCollectionsFilter = "__sessiondex_all_collections__";
const unassignedCollectionFilter = "__sessiondex_unassigned_collection__";
const secondsPerDay = 24 * 60 * 60;
const autoRefreshIntervalMs = 30_000;
const customSessionNameMaxLength = 100;
const collectionNameMaxLength = 48;
const tagNameMaxLength = 32;
const tagNamePattern = /^[a-z0-9][a-z0-9._-]*$/;
const activityFilterOptions: Array<{
  value: ActivityFilter;
  label: string;
  title: string;
}> = [
  { value: "all", label: "Any time", title: "Show any activity" },
  { value: "today", label: "Today", title: "Show sessions modified today" },
  {
    value: "yesterday",
    label: "Yesterday",
    title: "Show sessions modified yesterday",
  },
  {
    value: "last7",
    label: "Last 7 days",
    title: "Show sessions modified in the last 7 days",
  },
  {
    value: "last30",
    label: "Last month",
    title: "Show sessions modified in the last month",
  },
  {
    value: "resumed",
    label: "Recently resumed",
    title: "Show recently resumed sessions",
  },
];
const collectionColorChoices: Array<{
  value: CollectionColorChoice;
  label: string;
}> = [
  { value: "none", label: "No color" },
  { value: "gray", label: "Gray" },
  { value: "red", label: "Red" },
  { value: "orange", label: "Orange" },
  { value: "yellow", label: "Yellow" },
  { value: "green", label: "Green" },
  { value: "blue", label: "Blue" },
  { value: "purple", label: "Purple" },
];

const collectionColorStyles: Record<CollectionColorName, CollectionColorStyle> =
  {
    gray: {
      dotClassName: "bg-slate-400 dark:bg-slate-500",
      topBarClassName: "bg-slate-300 dark:bg-slate-600",
      swatchClassName: "bg-slate-400 dark:bg-slate-500",
    },
    red: {
      dotClassName: "bg-red-500 dark:bg-red-400",
      topBarClassName: "bg-red-300 dark:bg-red-500/70",
      swatchClassName: "bg-red-500 dark:bg-red-400",
    },
    orange: {
      dotClassName: "bg-orange-500 dark:bg-orange-400",
      topBarClassName: "bg-orange-300 dark:bg-orange-500/70",
      swatchClassName: "bg-orange-500 dark:bg-orange-400",
    },
    yellow: {
      dotClassName: "bg-yellow-400 dark:bg-yellow-300",
      topBarClassName: "bg-yellow-300 dark:bg-yellow-400/70",
      swatchClassName: "bg-yellow-400 dark:bg-yellow-300",
    },
    green: {
      dotClassName: "bg-emerald-500 dark:bg-emerald-400",
      topBarClassName: "bg-emerald-300 dark:bg-emerald-500/70",
      swatchClassName: "bg-emerald-500 dark:bg-emerald-400",
    },
    blue: {
      dotClassName: "bg-sky-500 dark:bg-sky-400",
      topBarClassName: "bg-sky-300 dark:bg-sky-500/70",
      swatchClassName: "bg-sky-500 dark:bg-sky-400",
    },
    purple: {
      dotClassName: "bg-violet-500 dark:bg-violet-400",
      topBarClassName: "bg-violet-300 dark:bg-violet-500/70",
      swatchClassName: "bg-violet-500 dark:bg-violet-400",
    },
  };

function collectionFilterValue(collectionName: string) {
  return `collection:${encodeURIComponent(collectionName)}`;
}

function collectionNameFromFilter(collectionFilter: string) {
  if (!collectionFilter.startsWith("collection:")) {
    return null;
  }

  try {
    return decodeURIComponent(collectionFilter.slice("collection:".length));
  } catch {
    return null;
  }
}

function isCollectionColorName(
  colorName: string | null | undefined,
): colorName is CollectionColorName {
  return Boolean(
    colorName &&
      Object.prototype.hasOwnProperty.call(collectionColorStyles, colorName),
  );
}

function normalizeCollectionColor(
  colorName: string | null | undefined,
): CollectionColorName | null {
  return isCollectionColorName(colorName) ? colorName : null;
}

function collectionColorStyle(
  colorName: string | null | undefined,
): CollectionColorStyle | null {
  const normalizedColor = normalizeCollectionColor(colorName);

  return normalizedColor ? collectionColorStyles[normalizedColor] : null;
}

function characterCount(value: string) {
  return Array.from(value).length;
}

function characterLimitError(label: string, value: string, maxLength: number) {
  if (characterCount(value.trim()) <= maxLength) {
    return null;
  }

  return `${label} must be ${maxLength} characters or fewer.`;
}

function normalizeTagName(tag: string) {
  return tag.trim().replace(/^#+/, "").toLowerCase();
}

function isValidTagName(tag: string) {
  return characterCount(tag) <= tagNameMaxLength && tagNamePattern.test(tag);
}

function tagNameInputTokens(value: string) {
  return value
    .split(/[,\s]+/)
    .map(normalizeTagName)
    .filter(Boolean);
}

function tagInputValidationError(value: string) {
  const tags = tagNameInputTokens(value);

  if (tags.length === 0) {
    return null;
  }

  if (tags.some((tag) => characterCount(tag) > tagNameMaxLength)) {
    return `Tags must be ${tagNameMaxLength} characters or fewer.`;
  }

  if (tags.some((tag) => !tagNamePattern.test(tag))) {
    return "Tags must start with a letter or number and can use letters, numbers, dashes, underscores, and dots.";
  }

  return null;
}

function tagNamesFromInput(value: string) {
  if (tagInputValidationError(value)) {
    return [];
  }

  return tagNameInputTokens(value).filter(isValidTagName);
}

function mergeTagNames(currentTags: string[], nextTags: string[]) {
  const mergedTags = new Set(currentTags.map(normalizeTagName));

  for (const tag of nextTags) {
    if (isValidTagName(tag)) {
      mergedTags.add(tag);
    }
  }

  return Array.from(mergedTags).sort((left, right) =>
    left.localeCompare(right),
  );
}

function decodeSearchFilterValue(value: string) {
  const trimmedValue = value.trim();

  try {
    return decodeURIComponent(trimmedValue);
  } catch {
    return trimmedValue;
  }
}

function uniqueValues(values: string[]) {
  return Array.from(new Set(values));
}

function parseBooleanSearchFilter(value: string) {
  const normalizedValue = value.trim().toLowerCase();

  if (["true", "yes", "1", "on"].includes(normalizedValue)) {
    return true;
  }

  if (["false", "no", "0", "off"].includes(normalizedValue)) {
    return false;
  }

  return null;
}

function parseSearchDateBound(value: string) {
  const match = /^(\d{4})(?:-(\d{1,2})(?:-(\d{1,2}))?)?$/.exec(
    value.trim(),
  );

  if (!match) {
    return null;
  }

  const year = Number(match[1]);
  const month = match[2] ? Number(match[2]) : 1;
  const day = match[3] ? Number(match[3]) : 1;

  if (year < 1970 || month < 1 || month > 12 || day < 1 || day > 31) {
    return null;
  }

  const date = new Date(year, month - 1, day);

  if (
    date.getFullYear() !== year ||
    date.getMonth() !== month - 1 ||
    date.getDate() !== day
  ) {
    return null;
  }

  return Math.floor(date.getTime() / 1000);
}

function tokenizeSearchQuery(query: string) {
  const tokens: string[] = [];
  const tokenPattern =
    /([a-zA-Z]+):"([^"]+)"|([a-zA-Z]+):'([^']+)'|"([^"]+)"|'([^']+)'|(\S+)/g;

  for (const match of query.matchAll(tokenPattern)) {
    tokens.push(
      match[1]
        ? `${match[1]}:${match[2]}`
        : match[3]
          ? `${match[3]}:${match[4]}`
          : (match[5] ?? match[6] ?? match[7] ?? ""),
    );
  }

  return tokens.filter(Boolean);
}

function parseAdvancedSearch(query: string): AdvancedSearch {
  const providers: string[] = [];
  const collections: string[] = [];
  const tags: string[] = [];
  const textTokens: string[] = [];
  let pinned: boolean | null = null;
  let hidden: boolean | null = null;
  let folder: string | null = null;
  let before: number | null = null;
  let after: number | null = null;

  for (const token of tokenizeSearchQuery(query)) {
    const separatorIndex = token.indexOf(":");

    if (separatorIndex <= 0) {
      textTokens.push(token);
      continue;
    }

    const key = token.slice(0, separatorIndex).toLowerCase();
    const value = decodeSearchFilterValue(token.slice(separatorIndex + 1));
    const normalizedValue = value.trim().toLowerCase();

    if (!normalizedValue) {
      textTokens.push(token);
      continue;
    }

    if (key === "provider") {
      providers.push(normalizedValue);
      continue;
    }

    if (key === "collection") {
      collections.push(normalizedValue);
      continue;
    }

    if (key === "tag") {
      const tag = normalizeTagName(normalizedValue);

      if (isValidTagName(tag)) {
        tags.push(tag);
        continue;
      }
    }

    if (key === "pinned") {
      const booleanValue = parseBooleanSearchFilter(normalizedValue);

      if (booleanValue !== null) {
        pinned = booleanValue;
        continue;
      }
    }

    if (key === "hidden") {
      const booleanValue = parseBooleanSearchFilter(normalizedValue);

      if (booleanValue !== null) {
        hidden = booleanValue;
        continue;
      }
    }

    if (key === "folder") {
      folder = normalizedValue;
      continue;
    }

    if (key === "before") {
      const dateBound = parseSearchDateBound(normalizedValue);

      if (dateBound !== null) {
        before = dateBound;
        continue;
      }
    }

    if (key === "after") {
      const dateBound = parseSearchDateBound(normalizedValue);

      if (dateBound !== null) {
        after = dateBound;
        continue;
      }
    }

    textTokens.push(token);
  }

  return {
    text: textTokens.join(" ").trim(),
    providers: uniqueValues(providers),
    collections: uniqueValues(collections),
    tags: uniqueValues(tags),
    pinned,
    hidden,
    folder,
    before,
    after,
  };
}

function activityTimeBounds(now = new Date()) {
  const todayStart = Math.floor(
    new Date(now.getFullYear(), now.getMonth(), now.getDate()).getTime() /
      1000,
  );

  return {
    todayStart,
    yesterdayStart: todayStart - secondsPerDay,
    last7Start: todayStart - secondsPerDay * 6,
    last30Start: todayStart - secondsPerDay * 29,
  };
}

function sessionMatchesActivityFilter(
  session: SessionRecord,
  activityFilter: ActivityFilter,
  bounds: ReturnType<typeof activityTimeBounds>,
) {
  if (activityFilter === "all") {
    return true;
  }

  if (activityFilter === "resumed") {
    return session.lastResumed !== null;
  }

  const lastModified = session.lastModified;

  if (lastModified === null) {
    return false;
  }

  if (activityFilter === "today") {
    return lastModified >= bounds.todayStart;
  }

  if (activityFilter === "yesterday") {
    return (
      lastModified >= bounds.yesterdayStart && lastModified < bounds.todayStart
    );
  }

  if (activityFilter === "last7") {
    return lastModified >= bounds.last7Start;
  }

  return lastModified >= bounds.last30Start;
}

function sessionKey(value: { provider: string; sessionId: string }) {
  return `${value.provider}:${value.sessionId}`;
}

function sessionDisplayName(
  session: Pick<SessionRecord, "sessionId" | "title">,
  friendlyName: string | null,
) {
  return friendlyName?.trim() || session.title?.trim() || session.sessionId;
}

function commandMatchesQuery(command: CommandPaletteCommand, query: string) {
  const tokens = query
    .trim()
    .toLowerCase()
    .split(/\s+/)
    .filter(Boolean);

  if (tokens.length === 0) {
    return true;
  }

  const searchableText = [
    command.label,
    command.detail,
    ...(command.keywords ?? []),
  ]
    .filter(Boolean)
    .join(" ")
    .toLowerCase();

  return tokens.every((token) => searchableText.includes(token));
}

function sessionCommandKeywords(session: SessionRecord) {
  return [
    session.displayName,
    session.sessionId,
    session.providerDisplayName,
    session.collection,
    session.note,
    ...session.tags,
    folderNameFromPath(session.workingDirectory),
    session.workingDirectory,
    session.discoveredBranch,
    session.discoveredRepository,
  ].filter((value): value is string => Boolean(value?.trim()));
}

function sessionCommandDetail(session: SessionRecord) {
  return [
    session.providerDisplayName,
    session.collection,
    folderNameFromPath(session.workingDirectory),
  ]
    .filter((value): value is string => Boolean(value?.trim()))
    .join(" / ");
}

function sessionMatchesMetadata(session: SessionRecord, query: string) {
  return (
    session.displayName.toLowerCase().includes(query) ||
    session.sessionId.toLowerCase().includes(query) ||
    session.providerDisplayName.toLowerCase().includes(query) ||
    (session.collection?.toLowerCase().includes(query) ?? false) ||
    (session.note?.toLowerCase().includes(query) ?? false) ||
    (session.workingDirectory?.toLowerCase().includes(query) ?? false) ||
    (folderNameFromPath(session.workingDirectory)
      ?.toLowerCase()
      .includes(query) ??
      false) ||
    (session.discoveredBranch?.toLowerCase().includes(query) ?? false) ||
    (session.discoveredRepository?.toLowerCase().includes(query) ?? false) ||
    session.tags.some((tag) => tag.toLowerCase().includes(query))
  );
}

function folderNameFromPath(path: string | null) {
  const trimmedPath = path?.trim();

  if (!trimmedPath) {
    return null;
  }

  const normalizedPath = trimmedPath.replace(/[\\/]+$/, "");
  const segments = normalizedPath.split(/[\\/]/).filter(Boolean);

  return segments.at(-1) ?? normalizedPath;
}

function projectKeyFromPath(path: string | null) {
  const trimmedPath = path?.trim();

  return trimmedPath ? trimmedPath : null;
}

function branchRepositoryPath(session: SessionRecord) {
  return (
    projectKeyFromPath(session.discoveredRepository) ??
    projectKeyFromPath(session.workingDirectory)
  );
}

function sessionMatchesAdvancedFilters(
  session: SessionRecord,
  advancedSearch: AdvancedSearch,
) {
  if (
    advancedSearch.providers.length > 0 &&
    !advancedSearch.providers.includes(session.provider.toLowerCase())
  ) {
    return false;
  }

  if (advancedSearch.collections.length > 0) {
    const collection = session.collection?.trim().toLowerCase();

    if (
      !collection ||
      !advancedSearch.collections.some((searchCollection) =>
        collection.includes(searchCollection),
      )
    ) {
      return false;
    }
  }

  if (advancedSearch.tags.length > 0) {
    const sessionTags = new Set(session.tags.map(normalizeTagName));

    if (!advancedSearch.tags.every((tag) => sessionTags.has(tag))) {
      return false;
    }
  }

  if (
    advancedSearch.pinned !== null &&
    session.isPinned !== advancedSearch.pinned
  ) {
    return false;
  }

  if (
    advancedSearch.hidden !== null &&
    session.isHidden !== advancedSearch.hidden
  ) {
    return false;
  }

  if (advancedSearch.folder) {
    const folderName = folderNameFromPath(session.workingDirectory)
      ?.toLowerCase()
      .trim();
    const workingDirectory = session.workingDirectory?.toLowerCase() ?? "";

    if (
      !folderName?.includes(advancedSearch.folder) &&
      !workingDirectory.includes(advancedSearch.folder)
    ) {
      return false;
    }
  }

  if (
    advancedSearch.before !== null &&
    (session.lastModified === null ||
      session.lastModified >= advancedSearch.before)
  ) {
    return false;
  }

  if (
    advancedSearch.after !== null &&
    (session.lastModified === null ||
      session.lastModified < advancedSearch.after)
  ) {
    return false;
  }

  return true;
}

async function copyTextToClipboard(text: string) {
  try {
    await navigator.clipboard.writeText(text);
    return;
  } catch {
    const textarea = document.createElement("textarea");
    textarea.value = text;
    textarea.setAttribute("readonly", "");
    textarea.style.position = "fixed";
    textarea.style.opacity = "0";
    document.body.appendChild(textarea);
    textarea.select();

    const copied = document.execCommand("copy");
    document.body.removeChild(textarea);

    if (!copied) {
      throw new Error("Clipboard copy failed.");
    }
  }
}

function useBodyScrollLock() {
  useEffect(() => {
    const previousOverflow = document.body.style.overflow;
    document.body.style.overflow = "hidden";

    return () => {
      document.body.style.overflow = previousOverflow;
    };
  }, []);
}

function isEditableKeyboardTarget(target: EventTarget | null) {
  if (!(target instanceof HTMLElement)) {
    return false;
  }

  return Boolean(
    target.closest("input, textarea, select, [contenteditable='true']"),
  );
}

function isInteractiveKeyboardTarget(target: EventTarget | null) {
  if (!(target instanceof HTMLElement)) {
    return false;
  }

  return Boolean(
    target.closest(
      "a, button, input, textarea, select, [contenteditable='true'], [role='button'], [role='menuitem'], [role='tab']",
    ),
  );
}

function appWindowIsActive() {
  return document.visibilityState === "visible" && document.hasFocus();
}

type LoadDataOptions = {
  background?: boolean;
  skipIfBusy?: boolean;
};

function App() {
  const [sessions, setSessions] = useState<SessionRecord[]>([]);
  const [searchMatches, setSearchMatches] = useState<SessionSearchResult[]>([]);
  const [providers, setProviders] = useState<ProviderStatus[]>([]);
  const [settings, setSettings] = useState<AppSettings>(defaultSettings);
  const [search, setSearch] = useState("");
  const [sessionView, setSessionView] = useState<SessionView>("all");
  const [activityFilter, setActivityFilter] =
    useState<ActivityFilter>("all");
  const [statsOpen, setStatsOpen] = useState(false);
  const [commandPaletteOpen, setCommandPaletteOpen] = useState(false);
  const [commandPaletteQuery, setCommandPaletteQuery] = useState("");
  const [collectionFilter, setCollectionFilter] =
    useState(allCollectionsFilter);
  const searchInputRef = useRef<HTMLInputElement>(null);
  const statsPopoverRef = useRef<HTMLDivElement | null>(null);
  const sessionsGridRef = useRef<HTMLElement | null>(null);
  const sessionCardRefs = useRef(new Map<string, HTMLDivElement>());
  const [selectedSessionKey, setSelectedSessionKey] = useState<string | null>(
    null,
  );
  const [loading, setLoading] = useState(true);
  const [refreshing, setRefreshing] = useState(false);
  const [searching, setSearching] = useState(false);
  const [appActive, setAppActive] = useState(() => appWindowIsActive());
  const [message, setMessage] = useState<string | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [toast, setToast] = useState<ToastState | null>(null);
  const [renameTarget, setRenameTarget] = useState<SessionRecord | null>(null);
  const [renameValue, setRenameValue] = useState("");
  const [collectionTarget, setCollectionTarget] =
    useState<SessionRecord | null>(null);
  const [collectionValue, setCollectionValue] = useState("");
  const [collectionColorValue, setCollectionColorValue] =
    useState<CollectionColorChoice>("none");
  const [collectionColorTouched, setCollectionColorTouched] = useState(false);
  const [branchTarget, setBranchTarget] = useState<SessionRecord | null>(null);
  const [branchValue, setBranchValue] = useState("");
  const [branchOptions, setBranchOptions] = useState<string[]>([]);
  const [branchOptionsLoading, setBranchOptionsLoading] = useState(false);
  const [branchError, setBranchError] = useState<string | null>(null);
  const [noteTarget, setNoteTarget] = useState<SessionRecord | null>(null);
  const [noteValue, setNoteValue] = useState("");
  const [tagsTarget, setTagsTarget] = useState<SessionRecord | null>(null);
  const [tagDraft, setTagDraft] = useState<string[]>([]);
  const [tagValue, setTagValue] = useState("");
  const [deleteTarget, setDeleteTarget] = useState<SessionRecord | null>(null);
  const [settingsOpen, setSettingsOpen] = useState(false);
  const [historyTarget, setHistoryTarget] = useState<SessionRecord | null>(null);
  const [sessionHistory, setSessionHistory] = useState<SessionHistory | null>(
    null,
  );
  const [historyLoading, setHistoryLoading] = useState(false);
  const [historyError, setHistoryError] = useState<string | null>(null);
  const loadDataPromiseRef = useRef<Promise<void> | null>(null);
  const searchPromiseRef = useRef<Promise<SessionSearchResult[]> | null>(null);
  const lastRefreshStartedAtRef = useRef(0);

  useEffect(() => {
    document.documentElement.classList.toggle("dark", settings.theme === "dark");
  }, [settings.theme]);

  useEffect(() => {
    if (!toast) {
      return;
    }

    const timeoutId = window.setTimeout(() => {
      setToast((currentToast) =>
        currentToast?.id === toast.id ? null : currentToast,
      );
    }, 4500);

    return () => window.clearTimeout(timeoutId);
  }, [toast]);

  useEffect(() => {
    if (!statsOpen) {
      return;
    }

    function handlePointerDown(event: MouseEvent) {
      const popoverElement = statsPopoverRef.current;

      if (
        popoverElement &&
        event.target instanceof Node &&
        popoverElement.contains(event.target)
      ) {
        return;
      }

      setStatsOpen(false);
    }

    document.addEventListener("mousedown", handlePointerDown);

    return () => document.removeEventListener("mousedown", handlePointerDown);
  }, [statsOpen]);

  function showToast(
    title: string,
    description: string,
    tone: ToastState["tone"] = "success",
  ) {
    setToast({
      id: Date.now(),
      title,
      description,
      tone,
    });
  }

  const loadData = useCallback(
    async ({ background = false, skipIfBusy = false }: LoadDataOptions = {}) => {
      if (loadDataPromiseRef.current) {
        if (skipIfBusy) {
          return;
        }

        await loadDataPromiseRef.current;
      }

      const refreshPromise = (async () => {
        lastRefreshStartedAtRef.current = Date.now();

        if (background) {
          setRefreshing(true);
        } else {
          setLoading(true);
          setError(null);
        }

        try {
          const [nextSettings, nextProviders, nextSessions] =
            await Promise.all([
              api.getSettings(),
              api.listProviders(),
              api.listSessions(),
            ]);

          setSettings(nextSettings);
          setProviders(nextProviders);
          setSessions(nextSessions);
        } catch (err) {
          if (!background) {
            setError(err instanceof Error ? err.message : String(err));
          } else {
            console.warn("Automatic session refresh failed.", err);
          }
        } finally {
          if (background) {
            setRefreshing(false);
          } else {
            setLoading(false);
          }
        }
      })();

      loadDataPromiseRef.current = refreshPromise;

      try {
        await refreshPromise;
      } finally {
        if (loadDataPromiseRef.current === refreshPromise) {
          loadDataPromiseRef.current = null;
        }
      }
    },
    [],
  );

  useEffect(() => {
    void loadData();
  }, [loadData]);

  useEffect(() => {
    function updateAppActive() {
      setAppActive(appWindowIsActive());
    }

    updateAppActive();
    window.addEventListener("focus", updateAppActive);
    window.addEventListener("blur", updateAppActive);
    document.addEventListener("visibilitychange", updateAppActive);

    return () => {
      window.removeEventListener("focus", updateAppActive);
      window.removeEventListener("blur", updateAppActive);
      document.removeEventListener("visibilitychange", updateAppActive);
    };
  }, []);

  useEffect(() => {
    if (!appActive) {
      return;
    }

    function refreshIfStale() {
      if (
        Date.now() - lastRefreshStartedAtRef.current <
        autoRefreshIntervalMs
      ) {
        return;
      }

      void loadData({ background: true, skipIfBusy: true });
    }

    refreshIfStale();
    const intervalId = window.setInterval(
      refreshIfStale,
      autoRefreshIntervalMs,
    );

    return () => window.clearInterval(intervalId);
  }, [appActive, loadData]);

  const advancedSearch = useMemo(() => parseAdvancedSearch(search), [search]);

  useEffect(() => {
    const query = advancedSearch.text.trim();

    if (!query) {
      setSearchMatches([]);
      setSearching(false);
      return;
    }

    const providerIds = new Set(providers.map((provider) => provider.id));

    if (
      settings.providerFilter !== "all" &&
      advancedSearch.providers.length > 0 &&
      !advancedSearch.providers.includes(settings.providerFilter)
    ) {
      setSearchMatches([]);
      setSearching(false);
      return;
    }

    if (
      advancedSearch.providers.length > 0 &&
      !advancedSearch.providers.some((providerId) => providerIds.has(providerId))
    ) {
      setSearchMatches([]);
      setSearching(false);
      return;
    }

    let cancelled = false;
    const providerFilter =
      settings.providerFilter !== "all"
        ? settings.providerFilter
        : advancedSearch.providers.length === 1 &&
            providerIds.has(advancedSearch.providers[0])
          ? advancedSearch.providers[0]
          : null;

    setError(null);
    setSearchMatches([]);
    setSearching(true);

    async function runSearch() {
      const activeSearch = searchPromiseRef.current;

      if (activeSearch) {
        try {
          await activeSearch;
        } catch {
          // The active request reports its own failure.
        }
      }

      if (cancelled) {
        return;
      }

      const searchPromise = api.searchSessions(query, providerFilter);
      searchPromiseRef.current = searchPromise;

      try {
        const results = await searchPromise;

        if (!cancelled) {
          setSearchMatches(results);
        }
      } catch (err) {
        if (!cancelled) {
          setSearchMatches([]);
          setError(err instanceof Error ? err.message : String(err));
        }
      } finally {
        if (searchPromiseRef.current === searchPromise) {
          searchPromiseRef.current = null;
        }

        if (!cancelled) {
          setSearching(false);
        }
      }
    }

    const timeoutId = window.setTimeout(() => {
      void runSearch();
    }, 250);

    return () => {
      cancelled = true;
      window.clearTimeout(timeoutId);
    };
  }, [advancedSearch, providers, settings.providerFilter]);

  const searchMatchesBySession = useMemo(() => {
    return new Map(
      searchMatches.map((searchMatch) => [
        sessionKey(searchMatch),
        searchMatch,
      ]),
    );
  }, [searchMatches]);

  const sessionStatistics = useMemo<SessionStatistics>(() => {
    const providerCountsById = new Map<
      string,
      { id: string; label: string; count: number; order: number }
    >();
    const collections = new Set<string>();
    const tags = new Set<string>();
    let pinnedSessions = 0;
    let hiddenSessions = 0;

    providers.forEach((provider, index) => {
      providerCountsById.set(provider.id, {
        id: provider.id,
        label: provider.displayName,
        count: 0,
        order: index,
      });
    });

    for (const session of sessions) {
      const providerCount = providerCountsById.get(session.provider);

      if (providerCount) {
        providerCount.count += 1;
      } else {
        providerCountsById.set(session.provider, {
          id: session.provider,
          label: session.providerDisplayName,
          count: 1,
          order: providerCountsById.size,
        });
      }

      if (session.isPinned) {
        pinnedSessions += 1;
      }

      if (session.isHidden) {
        hiddenSessions += 1;
      }

      const collectionName = session.collection?.trim();

      if (collectionName) {
        collections.add(collectionName);
      }

      for (const tag of session.tags) {
        tags.add(normalizeTagName(tag));
      }
    }

    return {
      totalSessions: sessions.length,
      providerCounts: Array.from(providerCountsById.values())
        .sort((left, right) => left.order - right.order)
        .map(({ id, label, count }) => ({ id, label, count })),
      pinnedSessions,
      hiddenSessions,
      collectionCount: collections.size,
      tagCount: tags.size,
    };
  }, [providers, sessions]);

  const providerFilteredSessions = useMemo(() => {
    return sessions.filter((session) => {
      if (
        settings.providerFilter !== "all" &&
        session.provider !== settings.providerFilter
      ) {
        return false;
      }

      if (
        session.isHidden &&
        !settings.showHiddenSessions &&
        advancedSearch.hidden !== true
      ) {
        return false;
      }

      return true;
    });
  }, [
    advancedSearch.hidden,
    sessions,
    settings.providerFilter,
    settings.showHiddenSessions,
  ]);

  const collectionOptions = useMemo(() => {
    const collectionsByName = new Map<
      string,
      { name: string; count: number; color: CollectionColorName | null }
    >();

    for (const session of providerFilteredSessions) {
      const collectionName = session.collection?.trim();

      if (collectionName) {
        const existingCollection = collectionsByName.get(collectionName);
        const collectionColor = normalizeCollectionColor(
          session.collectionColor,
        );

        if (existingCollection) {
          existingCollection.count += 1;

          if (!existingCollection.color && collectionColor) {
            existingCollection.color = collectionColor;
          }
        } else {
          collectionsByName.set(collectionName, {
            name: collectionName,
            count: 1,
            color: collectionColor,
          });
        }
      }
    }

    return Array.from(collectionsByName.values()).sort((left, right) =>
      left.name.localeCompare(right.name),
    );
  }, [providerFilteredSessions]);

  const collectionNames = useMemo(
    () => collectionOptions.map((collection) => collection.name),
    [collectionOptions],
  );

  const collectionColorByName = useMemo(() => {
    return new Map(
      collectionOptions.map((collection) => [
        collection.name,
        collection.color,
      ]),
    );
  }, [collectionOptions]);

  const allTagNames = useMemo(() => {
    const tags = new Set<string>();

    for (const session of sessions) {
      for (const tag of session.tags) {
        const normalizedTag = normalizeTagName(tag);

        if (normalizedTag) {
          tags.add(normalizedTag);
        }
      }
    }

    return Array.from(tags).sort((left, right) => left.localeCompare(right));
  }, [sessions]);

  const unassignedCollectionCount = useMemo(() => {
    return providerFilteredSessions.filter(
      (session) => !session.collection?.trim(),
    ).length;
  }, [providerFilteredSessions]);

  useEffect(() => {
    if (collectionFilter === allCollectionsFilter) {
      return;
    }

    const selectedCollectionName = collectionNameFromFilter(collectionFilter);
    const filterStillExists =
      collectionFilter === unassignedCollectionFilter
        ? unassignedCollectionCount > 0
        : selectedCollectionName !== null &&
          collectionOptions.some(
            (collection) => collection.name === selectedCollectionName,
          );

    if (!filterStillExists) {
      setCollectionFilter(allCollectionsFilter);
    }
  }, [collectionFilter, collectionOptions, unassignedCollectionCount]);

  useEffect(() => {
    if (!collectionTarget || collectionColorTouched) {
      return;
    }

    const collectionName = collectionValue.trim();
    const collectionColor = collectionName
      ? collectionColorByName.get(collectionName)
      : null;

    setCollectionColorValue(collectionColor ?? "none");
  }, [
    collectionColorByName,
    collectionColorTouched,
    collectionTarget,
    collectionValue,
  ]);

  const activityBounds = useMemo(() => activityTimeBounds(), [sessions]);

  const collectionFilteredSessions = useMemo(() => {
    const selectedCollectionName = collectionNameFromFilter(collectionFilter);

    return providerFilteredSessions.filter((session) => {
      if (
        collectionFilter === unassignedCollectionFilter &&
        session.collection?.trim()
      ) {
        return false;
      }

      if (
        collectionFilter !== allCollectionsFilter &&
        collectionFilter !== unassignedCollectionFilter &&
        session.collection !== selectedCollectionName
      ) {
        return false;
      }

      return true;
    });
  }, [collectionFilter, providerFilteredSessions]);

  const activityCounts = useMemo(() => {
    return Object.fromEntries(
      activityFilterOptions.map((activityOption) => [
        activityOption.value,
        collectionFilteredSessions.filter((session) =>
          sessionMatchesActivityFilter(
            session,
            activityOption.value,
            activityBounds,
          ),
        ).length,
      ]),
    ) as Record<ActivityFilter, number>;
  }, [activityBounds, collectionFilteredSessions]);

  const activityFilteredSessions = useMemo(() => {
    return collectionFilteredSessions.filter((session) =>
      sessionMatchesActivityFilter(session, activityFilter, activityBounds),
    );
  }, [activityBounds, activityFilter, collectionFilteredSessions]);

  const filteredSessions = useMemo(() => {
    const query = advancedSearch.text.trim().toLowerCase();

    return activityFilteredSessions.filter((session) => {
      if (!sessionMatchesAdvancedFilters(session, advancedSearch)) {
        return false;
      }

      if (!query) {
        return true;
      }

      return (
        sessionMatchesMetadata(session, query) ||
        searchMatchesBySession.has(sessionKey(session))
      );
    });
  }, [activityFilteredSessions, advancedSearch, searchMatchesBySession]);

  const pinnedFilteredCount = useMemo(() => {
    return filteredSessions.filter((session) => session.isPinned).length;
  }, [filteredSessions]);

  const displayedSessions = useMemo(() => {
    return filteredSessions
      .map((session, index) => ({ session, index }))
      .filter(({ session }) => sessionView === "all" || session.isPinned)
      .sort((left, right) => {
        if (activityFilter === "resumed") {
          return (
            (right.session.lastResumed ?? 0) -
              (left.session.lastResumed ?? 0) || left.index - right.index
          );
        }

        if (
          sessionView === "all" &&
          left.session.isPinned !== right.session.isPinned
        ) {
          return left.session.isPinned ? -1 : 1;
        }

        return left.index - right.index;
      })
      .map(({ session }) => session);
  }, [activityFilter, filteredSessions, sessionView]);

  const selectedSessionIndex = useMemo(() => {
    return displayedSessions.findIndex(
      (session) => sessionKey(session) === selectedSessionKey,
    );
  }, [displayedSessions, selectedSessionKey]);

  const selectedSession =
    selectedSessionIndex >= 0 ? displayedSessions[selectedSessionIndex] : null;
  const activeSession = selectedSession ?? displayedSessions[0] ?? null;

  useEffect(() => {
    setSelectedSessionKey((currentKey) => {
      if (displayedSessions.length === 0) {
        return null;
      }

      if (
        currentKey &&
        displayedSessions.some((session) => sessionKey(session) === currentKey)
      ) {
        return currentKey;
      }

      return sessionKey(displayedSessions[0]);
    });
  }, [displayedSessions]);

  function focusSessionCard(nextSessionKey: string) {
    window.requestAnimationFrame(() => {
      const element = sessionCardRefs.current.get(nextSessionKey);

      element?.scrollIntoView({ block: "nearest", inline: "nearest" });
      element?.focus({ preventScroll: true });
    });
  }

  function selectSessionAtIndex(index: number) {
    if (displayedSessions.length === 0) {
      return;
    }

    const nextIndex = Math.max(
      0,
      Math.min(index, displayedSessions.length - 1),
    );
    const nextSessionKey = sessionKey(displayedSessions[nextIndex]);

    setSelectedSessionKey(nextSessionKey);
    focusSessionCard(nextSessionKey);
  }

  function selectRelativeSession(offset: number) {
    const fallbackIndex = offset > 0 ? -1 : displayedSessions.length;
    const currentIndex =
      selectedSessionIndex >= 0 ? selectedSessionIndex : fallbackIndex;

    selectSessionAtIndex(currentIndex + offset);
  }

  function sessionGridColumnCount() {
    const gridElement = sessionsGridRef.current;

    if (!gridElement) {
      return 1;
    }

    const gridTemplateColumns =
      window.getComputedStyle(gridElement).gridTemplateColumns;

    if (!gridTemplateColumns || gridTemplateColumns === "none") {
      return 1;
    }

    return Math.max(1, gridTemplateColumns.split(" ").filter(Boolean).length);
  }

  function openCommandPalette() {
    setStatsOpen(false);
    setCommandPaletteOpen(true);
    setCommandPaletteQuery("");
  }

  function closeCommandPalette() {
    setCommandPaletteOpen(false);
    setCommandPaletteQuery("");
  }

  async function handleProviderFilterChange(providerFilter: string) {
    const nextSettings = { ...settings, providerFilter };
    setSettings(nextSettings);
    setError(null);

    try {
      await api.saveSettings(nextSettings);
    } catch (err) {
      setError(err instanceof Error ? err.message : String(err));
    }
  }

  async function handleResume(session: SessionRecord) {
    setMessage(null);
    setError(null);

    try {
      await api.resumeSession(
        session.provider,
        session.sessionId,
        session.workingDirectory,
      );
      const resumedAt = Math.floor(Date.now() / 1000);

      setSessions((currentSessions) =>
        currentSessions.map((currentSession) =>
          sessionKey(currentSession) === sessionKey(session)
            ? { ...currentSession, lastResumed: resumedAt }
            : currentSession,
        ),
      );
      setMessage(`Opened ${session.displayName} in your terminal.`);
    } catch (err) {
      setError(err instanceof Error ? err.message : String(err));
    }
  }

  async function copyResumeCommand(session: SessionRecord) {
    setMessage(null);
    setError(null);

    try {
      await copyTextToClipboard(session.resumeCommand);
      showToast("Resume command copied", session.resumeCommand);
    } catch (err) {
      const description = err instanceof Error ? err.message : String(err);
      showToast("Copy failed", description, "error");
      setError(description);
    }
  }

  async function copySessionId(session: SessionRecord) {
    setMessage(null);
    setError(null);

    try {
      await copyTextToClipboard(session.sessionId);
      showToast("Session ID copied", session.sessionId);
    } catch (err) {
      const description = err instanceof Error ? err.message : String(err);
      showToast("Copy failed", description, "error");
      setError(description);
    }
  }

  async function openSessionFolder(session: SessionRecord) {
    const workingDirectory = session.workingDirectory;

    if (!workingDirectory) {
      return;
    }

    setMessage(null);
    setError(null);

    try {
      await api.openWorkingDirectory(workingDirectory);
      setMessage(
        `Opened ${folderNameFromPath(workingDirectory) ?? workingDirectory}.`,
      );
    } catch (err) {
      setError(err instanceof Error ? err.message : String(err));
    }
  }

  async function toggleSessionPinned(session: SessionRecord) {
    const nextIsPinned = !session.isPinned;

    setMessage(null);
    setError(null);
    setSessions((currentSessions) =>
      currentSessions.map((currentSession) =>
        sessionKey(currentSession) === sessionKey(session)
          ? { ...currentSession, isPinned: nextIsPinned }
          : currentSession,
      ),
    );

    try {
      await api.setSessionPinned(
        session.provider,
        session.sessionId,
        nextIsPinned,
      );
    } catch (err) {
      const description = err instanceof Error ? err.message : String(err);

      setSessions((currentSessions) =>
        currentSessions.map((currentSession) =>
          sessionKey(currentSession) === sessionKey(session)
            ? { ...currentSession, isPinned: session.isPinned }
            : currentSession,
        ),
      );
      setError(description);
      showToast("Pin update failed", description, "error");
    }
  }

  async function toggleProjectFavorite(session: SessionRecord) {
    const workingDirectory = projectKeyFromPath(session.workingDirectory);

    if (!workingDirectory) {
      return;
    }

    const nextIsFavoriteProject = !session.isFavoriteProject;

    setMessage(null);
    setError(null);
    setSessions((currentSessions) =>
      currentSessions.map((currentSession) =>
        projectKeyFromPath(currentSession.workingDirectory) === workingDirectory
          ? { ...currentSession, isFavoriteProject: nextIsFavoriteProject }
          : currentSession,
      ),
    );

    try {
      await api.setProjectFavorite(workingDirectory, nextIsFavoriteProject);
    } catch (err) {
      const description = err instanceof Error ? err.message : String(err);

      setSessions((currentSessions) =>
        currentSessions.map((currentSession) =>
          projectKeyFromPath(currentSession.workingDirectory) ===
          workingDirectory
            ? { ...currentSession, isFavoriteProject: session.isFavoriteProject }
            : currentSession,
        ),
      );
      setError(description);
      showToast("Project favorite update failed", description, "error");
    }
  }

  function openRename(session: SessionRecord) {
    setRenameTarget(session);
    setRenameValue(session.friendlyName ?? "");
  }

  function openCollection(session: SessionRecord) {
    setCollectionTarget(session);
    setCollectionValue(session.collection ?? "");
    setCollectionColorValue(
      normalizeCollectionColor(session.collectionColor) ?? "none",
    );
    setCollectionColorTouched(false);
  }

  function openBranchCorrection(session: SessionRecord) {
    const repositoryPath = branchRepositoryPath(session);

    setBranchTarget(session);
    setBranchValue(session.discoveredBranch ?? "");
    setBranchOptions([]);
    setBranchError(null);

    if (!repositoryPath) {
      return;
    }

    setBranchOptionsLoading(true);
    void api
      .listRepositoryBranches(repositoryPath)
      .then((branches) => setBranchOptions(branches))
      .catch((err) =>
        setBranchError(err instanceof Error ? err.message : String(err)),
      )
      .finally(() => setBranchOptionsLoading(false));
  }

  function openNote(session: SessionRecord) {
    setNoteTarget(session);
    setNoteValue(session.note ?? "");
  }

  function openTags(session: SessionRecord) {
    setTagsTarget(session);
    setTagDraft(mergeTagNames([], session.tags));
    setTagValue("");
  }

  function addTagsFromValue(value: string) {
    if (tagInputValidationError(value)) {
      return;
    }

    const nextTags = tagNamesFromInput(value);

    if (nextTags.length === 0) {
      return;
    }

    setTagDraft((currentTags) => mergeTagNames(currentTags, nextTags));
    setTagValue("");
  }

  function removeTag(tag: string) {
    setTagDraft((currentTags) =>
      currentTags.filter((currentTag) => currentTag !== tag),
    );
  }

  async function saveRename() {
    if (!renameTarget) {
      return;
    }

    const nextName = renameValue.trim();
    const validationError = characterLimitError(
      "Custom session name",
      nextName,
      customSessionNameMaxLength,
    );

    if (validationError) {
      return;
    }

    const nextFriendlyName = nextName || null;

    try {
      await api.renameSession(
        renameTarget.provider,
        renameTarget.sessionId,
        nextName,
      );
      updateSession(renameTarget, (currentSession) => ({
        ...currentSession,
        friendlyName: nextFriendlyName,
        displayName: sessionDisplayName(currentSession, nextFriendlyName),
      }));
      setRenameTarget(null);
      setMessage(null);
      setError(null);
      showToast(
        nextName ? "Session name saved" : "Session name removed",
        nextName || "Using provider title.",
      );
    } catch (err) {
      setError(err instanceof Error ? err.message : String(err));
    }
  }

  async function saveCollection() {
    if (!collectionTarget) {
      return;
    }

    const nextCollection = collectionValue.trim();
    const validationError = characterLimitError(
      "Collection name",
      nextCollection,
      collectionNameMaxLength,
    );

    if (validationError) {
      return;
    }

    const nextCollectionColor =
      nextCollection && collectionColorValue !== "none"
        ? collectionColorValue
        : null;
    const targetKey = sessionKey(collectionTarget);

    try {
      await api.setSessionCollection(
        collectionTarget.provider,
        collectionTarget.sessionId,
        nextCollection,
      );

      if (nextCollection) {
        await api.setCollectionColor(nextCollection, collectionColorValue);
      }

      setSessions((currentSessions) =>
        currentSessions.map((currentSession) => {
          if (sessionKey(currentSession) === targetKey) {
            return {
              ...currentSession,
              collection: nextCollection || null,
              collectionColor: nextCollection ? nextCollectionColor : null,
            };
          }

          if (nextCollection && currentSession.collection === nextCollection) {
            return {
              ...currentSession,
              collectionColor: nextCollectionColor,
            };
          }

          return currentSession;
        }),
      );
      setCollectionTarget(null);
      setMessage(null);
      setError(null);
      showToast(
        nextCollection
          ? "Session collection saved"
          : "Session collection removed",
        nextCollection || collectionTarget.displayName,
      );
    } catch (err) {
      setError(err instanceof Error ? err.message : String(err));
    }
  }

  async function saveBranchCorrection() {
    if (!branchTarget) {
      return;
    }

    const nextBranch = branchValue.trim();

    if (!nextBranch) {
      setBranchError("Branch is required.");
      return;
    }

    setBranchError(null);
    setError(null);

    try {
      await api.setSessionDiscoveredBranch(
        branchTarget.provider,
        branchTarget.sessionId,
        nextBranch,
      );

      const targetKey = sessionKey(branchTarget);
      const fallbackDiscoveredAt =
        branchTarget.discoveredAt ?? Math.floor(Date.now() / 1000);

      setSessions((currentSessions) =>
        currentSessions.map((currentSession) =>
          sessionKey(currentSession) === targetKey
            ? {
                ...currentSession,
                discoveredBranch: nextBranch,
                discoveredAt: currentSession.discoveredAt ?? fallbackDiscoveredAt,
              }
            : currentSession,
        ),
      );
      closeBranchCorrection();
      setMessage(null);
      setError(null);
      showToast("Session branch saved", nextBranch);
    } catch (err) {
      const description = err instanceof Error ? err.message : String(err);

      setBranchError(description);
      setError(description);
    }
  }

  async function saveNote() {
    if (!noteTarget) {
      return;
    }

    const nextNote = noteValue.trim();

    try {
      await api.setSessionNote(
        noteTarget.provider,
        noteTarget.sessionId,
        nextNote,
      );
      updateSession(noteTarget, (currentSession) => ({
        ...currentSession,
        note: nextNote || null,
      }));
      setNoteTarget(null);
      setMessage(null);
      setError(null);
      showToast(
        nextNote ? "Session note saved" : "Session note removed",
        noteTarget.displayName,
      );
    } catch (err) {
      setError(err instanceof Error ? err.message : String(err));
    }
  }

  async function saveTags() {
    if (!tagsTarget) {
      return;
    }

    const validationError = tagInputValidationError(tagValue);

    if (validationError) {
      return;
    }

    const nextTags = mergeTagNames(tagDraft, tagNamesFromInput(tagValue));

    try {
      await api.setSessionTags(
        tagsTarget.provider,
        tagsTarget.sessionId,
        nextTags,
      );
      updateSession(tagsTarget, (currentSession) => ({
        ...currentSession,
        tags: nextTags,
      }));
      closeTags();
      setMessage(null);
      setError(null);
      showToast(
        nextTags.length > 0 ? "Session tags saved" : "Session tags removed",
        nextTags.length > 0 ? nextTags.join(", ") : tagsTarget.displayName,
      );
    } catch (err) {
      setError(err instanceof Error ? err.message : String(err));
    }
  }

  async function confirmDeleteOrHide() {
    if (!deleteTarget) {
      return;
    }

    try {
      const result = await api.deleteOrHideSession(
        deleteTarget.provider,
        deleteTarget.sessionId,
      );
      setDeleteTarget(null);
      setMessage(result.message);
      await loadData();
    } catch (err) {
      setError(err instanceof Error ? err.message : String(err));
    }
  }

  async function handleUnhide(session: SessionRecord) {
    setMessage(null);
    setError(null);

    try {
      await api.unhideSession(session.provider, session.sessionId);
      setMessage("Session restored to the dashboard.");
      await loadData();
    } catch (err) {
      setError(err instanceof Error ? err.message : String(err));
    }
  }

  async function openSessionHistory(session: SessionRecord) {
    setHistoryTarget(session);
    setSessionHistory(null);
    setHistoryError(null);
    setHistoryLoading(true);

    try {
      const history = await api.getSessionHistory(
        session.provider,
        session.sessionId,
      );
      setSessionHistory(history);
    } catch (err) {
      setHistoryError(err instanceof Error ? err.message : String(err));
    } finally {
      setHistoryLoading(false);
    }
  }

  function closeSessionHistory() {
    setHistoryTarget(null);
    setSessionHistory(null);
    setHistoryError(null);
    setHistoryLoading(false);
  }

  function closeTags() {
    setTagsTarget(null);
    setTagDraft([]);
    setTagValue("");
  }

  function closeBranchCorrection() {
    setBranchTarget(null);
    setBranchValue("");
    setBranchOptions([]);
    setBranchOptionsLoading(false);
    setBranchError(null);
  }

  function updateSession(
    targetSession: SessionRecord,
    updater: (session: SessionRecord) => SessionRecord,
  ) {
    const targetKey = sessionKey(targetSession);

    setSessions((currentSessions) =>
      currentSessions.map((currentSession) =>
        sessionKey(currentSession) === targetKey
          ? updater(currentSession)
          : currentSession,
      ),
    );
  }

  const commandPaletteSessionCandidates = commandPaletteQuery.trim()
    ? filteredSessions.slice(0, 200)
    : [];
  const activeSessionName = activeSession?.displayName ?? "No session selected";
  const renameNameLength = characterCount(renameValue.trim());
  const renameNameError = characterLimitError(
    "Custom session name",
    renameValue,
    customSessionNameMaxLength,
  );
  const collectionNameLength = characterCount(collectionValue.trim());
  const collectionNameError = characterLimitError(
    "Collection name",
    collectionValue,
    collectionNameMaxLength,
  );
  const tagInputError = tagInputValidationError(tagValue);
  const tagInputTokens = tagNameInputTokens(tagValue);
  const tagInputLongestLength = Math.max(
    0,
    ...tagInputTokens.map(characterCount),
  );
  const canAddTagInput =
    tagInputTokens.length > 0 &&
    !tagInputError &&
    tagNamesFromInput(tagValue).length > 0;
  const filteredBranchOptions = useMemo(() => {
    const query = branchValue.trim().toLowerCase();

    return branchOptions.filter((branchName) => {
      if (!query) {
        return true;
      }

      return branchName.toLowerCase().includes(query);
    });
  }, [branchOptions, branchValue]);
  const sessionBranchValue = branchValue.trim();
  const openedSessionBranch = branchTarget?.discoveredBranch?.trim() ?? "";
  const sessionBranchChanged =
    sessionBranchValue.length > 0 && sessionBranchValue !== openedSessionBranch;
  const sessionBranchMatchesSuggestion = branchOptions.some(
    (branchName) => branchName === sessionBranchValue,
  );
  const sessionBranchIsTyped =
    sessionBranchValue.length > 0 &&
    branchOptions.length > 0 &&
    !sessionBranchMatchesSuggestion;
  const isRefreshingSessions = loading || refreshing;
  const isInitialSessionLoad = loading && sessions.length === 0;
  const commandPaletteCommands: CommandPaletteCommand[] = [
    {
      id: "global.focus-search",
      label: "Focus search",
      icon: Search,
      keywords: ["find", "filter", "sessions"],
      perform: () => {
        window.requestAnimationFrame(() => {
          searchInputRef.current?.focus();
          searchInputRef.current?.select();
        });
      },
    },
    {
      id: "global.refresh",
      label: "Refresh sessions",
      icon: RotateCw,
      keywords: ["reload", "sync"],
      disabled: isRefreshingSessions,
      perform: () => {
        void loadData();
      },
    },
    {
      id: "global.settings",
      label: "Open settings",
      icon: SlidersHorizontal,
      keywords: ["preferences", "configure"],
      perform: () => setSettingsOpen(true),
    },
    {
      id: "global.statistics",
      label: "Open session statistics",
      icon: BarChart3,
      keywords: ["stats", "counts", "summary", "providers"],
      perform: () => setStatsOpen(true),
    },
    {
      id: "global.show-all",
      label: "Show all sessions",
      icon: MessagesSquare,
      keywords: ["view"],
      perform: () => setSessionView("all"),
    },
    {
      id: "global.show-pinned",
      label: "Show pinned sessions",
      icon: Pin,
      keywords: ["view", "favorites"],
      perform: () => setSessionView("pinned"),
    },
    ...activityFilterOptions.map((activityOption) => ({
      id: `global.activity.${activityOption.value}`,
      label:
        activityOption.value === "all"
          ? "Show any activity"
          : `Show ${activityOption.label}`,
      detail: `${activityCounts[activityOption.value]} sessions`,
      icon: Clock3,
      keywords: ["recent", "activity", "modified", activityOption.label],
      disabled: activityFilter === activityOption.value,
      perform: () => setActivityFilter(activityOption.value),
    })),
    {
      id: "global.clear-filters",
      label: "Clear search and filters",
      icon: X,
      keywords: ["reset", "all sessions"],
      disabled:
        !search &&
        settings.providerFilter === "all" &&
        collectionFilter === allCollectionsFilter &&
        activityFilter === "all" &&
        sessionView === "all",
      perform: () => {
        setSearch("");
        setCollectionFilter(allCollectionsFilter);
        setActivityFilter("all");
        setSessionView("all");
        if (settings.providerFilter !== "all") {
          void handleProviderFilterChange("all");
        }
      },
    },
    {
      id: "selected.resume",
      label: "Resume selected session",
      detail: activeSessionName,
      icon: SquareTerminal,
      keywords: activeSession ? sessionCommandKeywords(activeSession) : [],
      disabled:
        !activeSession || activeSession.isHidden || !activeSession.canResume,
      perform: () => {
        if (activeSession) {
          void handleResume(activeSession);
        }
      },
    },
    {
      id: "selected.history",
      label: "Open selected chat history",
      detail: activeSessionName,
      icon: BookOpenText,
      keywords: activeSession ? sessionCommandKeywords(activeSession) : [],
      disabled: !activeSession,
      perform: () => {
        if (activeSession) {
          void openSessionHistory(activeSession);
        }
      },
    },
    {
      id: "selected.rename",
      label: "Rename selected session",
      detail: activeSessionName,
      icon: SquarePen,
      keywords: activeSession ? sessionCommandKeywords(activeSession) : [],
      disabled: !activeSession || activeSession.isHidden,
      perform: () => {
        if (activeSession) {
          openRename(activeSession);
        }
      },
    },
    {
      id: "selected.pin",
      label: activeSession?.isPinned
        ? "Unpin selected session"
        : "Pin selected session",
      detail: activeSessionName,
      icon: Pin,
      keywords: activeSession ? sessionCommandKeywords(activeSession) : [],
      disabled: !activeSession,
      perform: () => {
        if (activeSession) {
          void toggleSessionPinned(activeSession);
        }
      },
    },
    {
      id: "selected.collection",
      label: activeSession?.collection
        ? "Change selected session collection"
        : "Add selected session collection",
      detail: activeSessionName,
      icon: activeSession?.collection ? FolderOpen : FolderPlus,
      keywords: activeSession ? sessionCommandKeywords(activeSession) : [],
      disabled: !activeSession,
      perform: () => {
        if (activeSession) {
          openCollection(activeSession);
        }
      },
    },
    {
      id: "selected.note",
      label: activeSession?.note
        ? "Edit selected session note"
        : "Add selected session note",
      detail: activeSessionName,
      icon: activeSession?.note ? StickyNote : StickyNotePlus,
      keywords: activeSession ? sessionCommandKeywords(activeSession) : [],
      disabled: !activeSession,
      perform: () => {
        if (activeSession) {
          openNote(activeSession);
        }
      },
    },
    {
      id: "selected.tags",
      label:
        activeSession && activeSession.tags.length > 0
          ? "Edit selected session tags"
          : "Add selected session tags",
      detail: activeSessionName,
      icon:
        activeSession && activeSession.tags.length > 0
          ? Tags
          : TagPlus,
      keywords: activeSession ? sessionCommandKeywords(activeSession) : [],
      disabled: !activeSession,
      perform: () => {
        if (activeSession) {
          openTags(activeSession);
        }
      },
    },
    {
      id: "selected.folder",
      label: "Open selected project folder",
      detail: activeSession?.workingDirectory ?? activeSessionName,
      icon: FolderOpen,
      keywords: activeSession ? sessionCommandKeywords(activeSession) : [],
      disabled: !activeSession?.workingDirectory,
      perform: () => {
        if (activeSession) {
          void openSessionFolder(activeSession);
        }
      },
    },
    {
      id: "selected.copy-resume-command",
      label: "Copy selected resume command",
      detail: activeSessionName,
      icon: SquareTerminal,
      keywords: activeSession ? sessionCommandKeywords(activeSession) : [],
      disabled: !activeSession,
      perform: () => {
        if (activeSession) {
          void copyResumeCommand(activeSession);
        }
      },
    },
    {
      id: "selected.copy-id",
      label: "Copy selected session ID",
      detail: activeSession?.sessionId ?? activeSessionName,
      icon: Hash,
      keywords: activeSession ? sessionCommandKeywords(activeSession) : [],
      disabled: !activeSession,
      perform: () => {
        if (activeSession) {
          void copySessionId(activeSession);
        }
      },
    },
    {
      id: "selected.visibility",
      label: activeSession?.isHidden
        ? "Unhide selected session"
        : activeSession?.canDelete
          ? "Delete selected session"
          : "Hide selected session",
      detail: activeSessionName,
      icon: activeSession?.isHidden
        ? Eye
        : activeSession?.canDelete
          ? Trash
          : EyeOff,
      keywords: activeSession ? sessionCommandKeywords(activeSession) : [],
      disabled: !activeSession,
      perform: () => {
        if (!activeSession) {
          return;
        }

        if (activeSession.isHidden) {
          void handleUnhide(activeSession);
        } else {
          setDeleteTarget(activeSession);
        }
      },
    },
    ...commandPaletteSessionCandidates.flatMap((session) => {
      const key = sessionKey(session);
      const keywords = sessionCommandKeywords(session);
      const detail = sessionCommandDetail(session);

      return [
        {
          id: `session.${key}.resume`,
          label: `Resume ${session.displayName}`,
          detail,
          icon: SquareTerminal,
          keywords: ["session", "resume", ...keywords],
          disabled: session.isHidden || !session.canResume,
          perform: () => {
            setSelectedSessionKey(key);
            void handleResume(session);
          },
        },
        {
          id: `session.${key}.history`,
          label: `Open history for ${session.displayName}`,
          detail,
          icon: BookOpenText,
          keywords: ["session", "history", "open", ...keywords],
          perform: () => {
            setSelectedSessionKey(key);
            void openSessionHistory(session);
          },
        },
        {
          id: `session.${key}.select`,
          label: `Select ${session.displayName}`,
          detail,
          icon: Search,
          keywords: ["session", "select", ...keywords],
          perform: () => {
            setSelectedSessionKey(key);
            focusSessionCard(key);
          },
        },
      ];
    }),
  ];

  function runCommandPaletteCommand(command: CommandPaletteCommand) {
    if (command.disabled) {
      return;
    }

    closeCommandPalette();
    void command.perform();
  }

  useEffect(() => {
    function focusSearch() {
      searchInputRef.current?.focus();
      searchInputRef.current?.select();
    }

    function handleKeyDown(event: KeyboardEvent) {
      const key = event.key;
      const keyLower = key.toLowerCase();
      const hasPrimaryModifier = event.metaKey || event.ctrlKey;
      const hasOnlyPrimaryModifier =
        hasPrimaryModifier && !event.altKey && !event.shiftKey;
      const hasCommandPaletteModifier =
        hasPrimaryModifier && event.shiftKey && !event.altKey;
      const hasNoModifiers =
        !event.metaKey && !event.ctrlKey && !event.altKey && !event.shiftKey;
      const blockingSurfaceOpen = Boolean(
        renameTarget ||
          branchTarget ||
          collectionTarget ||
          noteTarget ||
          tagsTarget ||
          deleteTarget ||
          settingsOpen ||
          historyTarget,
      );

      if (hasCommandPaletteModifier && keyLower === "p") {
        event.preventDefault();
        if (commandPaletteOpen || !blockingSurfaceOpen) {
          setStatsOpen(false);
          setCommandPaletteOpen((currentOpen) => {
            if (currentOpen) {
              setCommandPaletteQuery("");
            }

            return !currentOpen;
          });
        }
        return;
      }

      if (commandPaletteOpen) {
        return;
      }

      if (key === "Escape") {
        if (statsOpen) {
          event.preventDefault();
          setStatsOpen(false);
          return;
        }

        if (renameTarget) {
          event.preventDefault();
          setRenameTarget(null);
          return;
        }

        if (collectionTarget) {
          event.preventDefault();
          setCollectionTarget(null);
          return;
        }

        if (branchTarget) {
          event.preventDefault();
          closeBranchCorrection();
          return;
        }

        if (noteTarget) {
          event.preventDefault();
          setNoteTarget(null);
          return;
        }

        if (tagsTarget) {
          event.preventDefault();
          closeTags();
          return;
        }

        if (deleteTarget) {
          event.preventDefault();
          setDeleteTarget(null);
          return;
        }

        if (historyTarget) {
          event.preventDefault();
          closeSessionHistory();
          return;
        }

        if (settingsOpen) {
          event.preventDefault();
          setSettingsOpen(false);
          return;
        }

        if (search) {
          event.preventDefault();
          setSearch("");
          focusSearch();
          return;
        }

        if (document.activeElement === searchInputRef.current) {
          event.preventDefault();
          searchInputRef.current?.blur();
        }

        return;
      }

      if (hasOnlyPrimaryModifier && keyLower === "f") {
        event.preventDefault();
        return;
      }

      if (
        renameTarget ||
        branchTarget ||
        collectionTarget ||
        noteTarget ||
        tagsTarget ||
        deleteTarget ||
        settingsOpen ||
        historyTarget
      ) {
        return;
      }

      if (hasOnlyPrimaryModifier && keyLower === "k") {
        event.preventDefault();
        focusSearch();
        return;
      }

      if (hasOnlyPrimaryModifier && key === ",") {
        event.preventDefault();
        setStatsOpen(false);
        setSettingsOpen(true);
        return;
      }

      if (
        (hasOnlyPrimaryModifier && keyLower === "r") ||
        (hasNoModifiers && key === "F5")
      ) {
        event.preventDefault();
        void loadData();
        return;
      }

      if (hasOnlyPrimaryModifier && (key === "1" || key === "2")) {
        event.preventDefault();
        setSessionView(key === "1" ? "all" : "pinned");
        return;
      }

      const targetIsSearchInput = event.target === searchInputRef.current;
      const targetIsEditable = isEditableKeyboardTarget(event.target);
      const targetIsInteractive = isInteractiveKeyboardTarget(event.target);
      const canNavigateSessions = !targetIsInteractive || targetIsSearchInput;
      const verticalSessionOffset = targetIsSearchInput
        ? 1
        : sessionGridColumnCount();

      if (canNavigateSessions && key === "ArrowDown") {
        event.preventDefault();
        selectRelativeSession(verticalSessionOffset);
        return;
      }

      if (canNavigateSessions && key === "ArrowUp") {
        event.preventDefault();
        selectRelativeSession(-verticalSessionOffset);
        return;
      }

      if (!targetIsInteractive && key === "ArrowRight") {
        event.preventDefault();
        selectRelativeSession(1);
        return;
      }

      if (!targetIsInteractive && key === "ArrowLeft") {
        event.preventDefault();
        selectRelativeSession(-1);
        return;
      }

      if (!targetIsInteractive && key === "Home") {
        event.preventDefault();
        selectSessionAtIndex(0);
        return;
      }

      if (!targetIsInteractive && key === "End") {
        event.preventDefault();
        selectSessionAtIndex(displayedSessions.length - 1);
        return;
      }

      const activeSession = selectedSession ?? displayedSessions[0] ?? null;

      if (!activeSession) {
        return;
      }

      if (key === "F2" && !targetIsInteractive) {
        if (!activeSession.isHidden) {
          event.preventDefault();
          openRename(activeSession);
        }

        return;
      }

      if (hasOnlyPrimaryModifier && key === "Enter" && canNavigateSessions) {
        event.preventDefault();

        if (!activeSession.isHidden && activeSession.canResume) {
          void handleResume(activeSession);
        }

        return;
      }

      if (
        (hasNoModifiers && key === "Enter" && canNavigateSessions) ||
        (hasNoModifiers && key === " " && !targetIsInteractive)
      ) {
        event.preventDefault();
        void openSessionHistory(activeSession);

        return;
      }

      if (
        !targetIsEditable &&
        !targetIsInteractive &&
        !activeSession.isHidden &&
        hasNoModifiers &&
        key === "Delete"
      ) {
        event.preventDefault();
        setDeleteTarget(activeSession);
        return;
      }

      if (!hasNoModifiers || targetIsEditable || targetIsInteractive) {
        return;
      }

      if (keyLower === "p") {
        event.preventDefault();
        void toggleSessionPinned(activeSession);
        return;
      }

      if (keyLower === "c") {
        event.preventDefault();
        openCollection(activeSession);
        return;
      }

      if (keyLower === "n") {
        event.preventDefault();
        openNote(activeSession);
        return;
      }

      if (keyLower === "t") {
        event.preventDefault();
        openTags(activeSession);
        return;
      }

      if (keyLower === "u" && activeSession.isHidden) {
        event.preventDefault();
        void handleUnhide(activeSession);
      }
    }

    document.addEventListener("keydown", handleKeyDown);

    return () => document.removeEventListener("keydown", handleKeyDown);
  }, [
    branchTarget,
    commandPaletteOpen,
    collectionTarget,
    deleteTarget,
    displayedSessions,
    historyTarget,
    noteTarget,
    renameTarget,
    search,
    selectedSession,
    statsOpen,
    tagsTarget,
    settingsOpen,
  ]);

  async function saveSettings(nextSettings: AppSettings) {
    try {
      await api.saveSettings(nextSettings);
      setSettings(nextSettings);
      setSettingsOpen(false);
      setMessage("Settings saved.");
    } catch (err) {
      setError(err instanceof Error ? err.message : String(err));
    }
  }

  const hasSearchQuery = search.trim().length > 0;
  const hasActivityFilter = activityFilter !== "all";
  const activeActivityLabel =
    activityFilterOptions.find((option) => option.value === activityFilter)
      ?.label ?? "activity";
  let emptyTitle = "No sessions found";
  let emptyDescription =
    "Install Codex CLI or Claude Code, create a session, then refresh.";

  if (hasSearchQuery) {
    emptyTitle = "No matching sessions";
    emptyDescription =
      sessionView === "pinned"
        ? "No pinned sessions match the current search, provider, collection, and activity filters."
        : "No custom session name, note, tag, collection, session id, provider, activity, or chat-history match was found.";
  } else if (activityFilter === "resumed") {
    emptyTitle = "No recently resumed sessions";
    emptyDescription = "Resume a session and it will appear here.";
  } else if (hasActivityFilter) {
    emptyTitle = `No ${activeActivityLabel.toLowerCase()} sessions`;
    emptyDescription =
      "No sessions fall into this activity window under the current provider, collection, and visibility filters.";
  } else if (sessionView === "pinned") {
    emptyTitle = "No pinned sessions";
    emptyDescription = "Pinned sessions stay at the top of All and collect here.";
  } else if (collectionFilter !== allCollectionsFilter) {
    emptyDescription =
      "No sessions are currently assigned to this collection under the active provider and visibility filters.";
  }

  return (
    <main className="min-h-screen bg-slate-50 text-slate-950 dark:bg-slate-950 dark:text-slate-50">
      <div className="mx-auto flex min-h-screen w-full max-w-7xl flex-col px-6 py-6">
        <header className="sticky top-3 z-20 flex items-center justify-between gap-3 rounded-lg border border-slate-200/80 bg-white/90 px-3.5 py-3 shadow-[0_16px_40px_-32px_rgba(15,23,42,0.85)] backdrop-blur-xl dark:border-white/10 dark:bg-slate-950/90 dark:shadow-black/50 sm:px-4">
          <div className="flex min-w-0 items-center gap-3">
            <img
              src={appIcon}
              alt=""
              className="h-10 w-10 shrink-0 rounded-lg border border-slate-200/80 bg-slate-100 shadow-sm shadow-slate-950/5 dark:border-white/10 dark:bg-slate-900 dark:shadow-black/30"
            />
            <div className="min-w-0">
              <h1 className="truncate text-base font-semibold sm:text-lg">
                SessionDex
              </h1>
              <p className="hidden text-xs font-medium text-slate-500 dark:text-slate-400 sm:block">
                Rolodex of AI CLI sessions
              </p>
            </div>
          </div>

          <div className="flex shrink-0 items-center gap-1.5">
            <HeaderActionButton
              title={
                isRefreshingSessions
                  ? "Refreshing sessions"
                  : "Refresh sessions (F5)"
              }
              aria-keyshortcuts="F5 Control+R Meta+R"
              disabled={isRefreshingSessions}
              icon={RotateCw}
              iconClassName={isRefreshingSessions ? "animate-spin" : undefined}
              onClick={() => void loadData()}
            />
            <HeaderActionButton
              title="Command palette (Cmd/Ctrl+Shift+P)"
              aria-keyshortcuts="Control+Shift+P Meta+Shift+P"
              icon={Keyboard}
              onClick={openCommandPalette}
            />
            <div ref={statsPopoverRef} className="relative">
              <HeaderActionButton
                title="Session statistics"
                aria-controls="session-statistics-popover"
                aria-expanded={statsOpen}
                icon={BarChart3}
                onClick={() => setStatsOpen((currentOpen) => !currentOpen)}
                className={
                  statsOpen
                    ? "border-slate-300 bg-white text-slate-950 dark:border-white/20 dark:bg-white/[0.08] dark:text-white"
                    : undefined
                }
              />
              {statsOpen && (
                <SessionStatisticsPopover statistics={sessionStatistics} />
              )}
            </div>
            <HeaderActionButton
              title="Settings (Cmd/Ctrl+,)"
              aria-keyshortcuts="Control+, Meta+,"
              icon={SlidersHorizontal}
              onClick={() => {
                setStatsOpen(false);
                setSettingsOpen(true);
              }}
            />
          </div>
        </header>

        <section className="mt-4 rounded-lg border border-slate-200/80 bg-white/85 p-2 shadow-sm shadow-slate-950/5 backdrop-blur-xl dark:border-white/10 dark:bg-slate-950/80 dark:shadow-black/20">
          <div className="flex flex-col gap-2 lg:flex-row lg:items-center">
            <div className="grid w-full min-w-0 gap-2 sm:grid-cols-[13rem_minmax(0,1fr)] lg:flex-1">
              <div className="relative">
                <label className="sr-only" htmlFor="provider-filter">
                  Provider filter
                </label>
                <select
                  id="provider-filter"
                  value={settings.providerFilter}
                  onChange={(event) =>
                    void handleProviderFilterChange(event.target.value)
                  }
                  className="h-11 w-full appearance-none rounded-md border border-transparent bg-slate-100/80 py-0 pl-3.5 pr-9 text-sm font-semibold text-slate-800 outline-none transition-all hover:bg-slate-100 focus:border-slate-300 focus:bg-white focus:ring-2 focus:ring-slate-200 dark:bg-white/[0.04] dark:text-slate-100 dark:hover:bg-white/[0.07] dark:focus:border-slate-700 dark:focus:bg-slate-950 dark:focus:ring-slate-800"
                >
                  <option value="all">All providers</option>
                  {providers.map((provider) => (
                    <option key={provider.id} value={provider.id}>
                      {provider.displayName}
                    </option>
                  ))}
                </select>
                <PremiumIcon
                  icon={ChevronDown}
                  className="pointer-events-none absolute right-3 top-1/2 h-4 w-4 -translate-y-1/2 text-slate-400 dark:text-slate-500"
                />
              </div>

              <div className="relative min-w-0">
                <PremiumIcon
                  icon={Search}
                  className="pointer-events-none absolute left-3.5 top-1/2 h-4 w-4 -translate-y-1/2 text-slate-400 dark:text-slate-500"
                />
                <Input
                  ref={searchInputRef}
                  value={search}
                  onChange={(event) => setSearch(event.target.value)}
                  aria-keyshortcuts="Control+K Meta+K"
                  title="Search sessions (Cmd/Ctrl+K)"
                  placeholder="Search names, notes, ids, history, or filters like tag:java"
                  className="h-11 rounded-md border-transparent bg-slate-100/80 pl-10 pr-20 font-medium shadow-none transition-all hover:bg-slate-100 focus:border-slate-300 focus:bg-white focus:ring-2 focus:ring-slate-200 dark:bg-white/[0.04] dark:hover:bg-white/[0.07] dark:focus:border-slate-700 dark:focus:bg-slate-950 dark:focus:ring-slate-800"
                />
                {searching && (
                  <PremiumIcon
                    icon={Loader2}
                    className={cn(
                      "pointer-events-none absolute top-1/2 h-4 w-4 -translate-y-1/2 animate-spin text-slate-400 dark:text-slate-500",
                      search ? "right-10" : "right-3.5",
                    )}
                  />
                )}
                {search && (
                  <button
                    type="button"
                    aria-label="Clear search"
                    title="Clear search"
                    onClick={() => {
                      setSearch("");
                      searchInputRef.current?.focus();
                    }}
                    className="absolute right-2 top-1/2 flex h-7 w-7 -translate-y-1/2 items-center justify-center rounded-md text-slate-400 transition-colors hover:bg-white hover:text-slate-900 focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-slate-300 dark:hover:bg-slate-900 dark:hover:text-slate-100 dark:focus-visible:ring-slate-700"
                  >
                    <PremiumIcon icon={X} className="h-4 w-4" />
                  </button>
                )}
              </div>
            </div>

            <div
              className="grid h-11 shrink-0 grid-cols-2 items-center rounded-md border border-slate-200/80 bg-slate-100/80 p-1 dark:border-white/10 dark:bg-white/[0.04] sm:w-[15rem]"
              aria-label="Session view"
            >
              <SessionViewButton
                active={sessionView === "all"}
                count={filteredSessions.length}
                label="All"
                onClick={() => setSessionView("all")}
              />
              <SessionViewButton
                active={sessionView === "pinned"}
                count={pinnedFilteredCount}
                icon={<PremiumIcon icon={Pin} className="h-3.5 w-3.5" />}
                label="Pinned"
                onClick={() => setSessionView("pinned")}
              />
            </div>
          </div>

          <div className="mt-2 flex flex-col gap-2 border-t border-slate-200/70 pt-2 dark:border-white/10 sm:flex-row sm:items-center sm:justify-between">
            <div className="flex shrink-0 items-center gap-1.5 px-1 text-xs font-semibold text-slate-500 dark:text-slate-400">
              <PremiumIcon icon={SlidersHorizontal} className="h-3.5 w-3.5" />
              <span>Filters</span>
            </div>

            <div className="grid w-full min-w-0 gap-2 sm:w-auto sm:grid-cols-[13rem_13rem]">
              <div className="relative min-w-0">
                <label className="sr-only" htmlFor="activity-filter">
                  Activity filter
                </label>
                <PremiumIcon
                  icon={Clock3}
                  className="pointer-events-none absolute left-3 top-1/2 h-3.5 w-3.5 -translate-y-1/2 text-slate-400 dark:text-slate-500"
                />
                <select
                  id="activity-filter"
                  value={activityFilter}
                  onChange={(event) =>
                    setActivityFilter(event.target.value as ActivityFilter)
                  }
                  className="h-9 w-full appearance-none rounded-md border border-transparent bg-slate-100/70 py-0 pl-8 pr-8 text-xs font-semibold text-slate-700 outline-none transition-all hover:bg-slate-100 focus:border-slate-300 focus:bg-white focus:ring-2 focus:ring-slate-200 dark:bg-white/[0.04] dark:text-slate-200 dark:hover:bg-white/[0.07] dark:focus:border-slate-700 dark:focus:bg-slate-950 dark:focus:ring-slate-800"
                >
                  {activityFilterOptions.map((activityOption) => (
                    <option
                      key={activityOption.value}
                      value={activityOption.value}
                    >
                      {`${activityOption.label} (${activityCounts[activityOption.value]})`}
                    </option>
                  ))}
                </select>
                <PremiumIcon
                  icon={ChevronDown}
                  className="pointer-events-none absolute right-2.5 top-1/2 h-3.5 w-3.5 -translate-y-1/2 text-slate-400 dark:text-slate-500"
                />
              </div>

              <div className="relative min-w-0">
                <label className="sr-only" htmlFor="collection-filter">
                  Collection filter
                </label>
                <PremiumIcon
                  icon={FolderOpen}
                  className="pointer-events-none absolute left-3 top-1/2 h-3.5 w-3.5 -translate-y-1/2 text-slate-400 dark:text-slate-500"
                />
                <select
                  id="collection-filter"
                  value={collectionFilter}
                  onChange={(event) => setCollectionFilter(event.target.value)}
                  className="h-9 w-full appearance-none rounded-md border border-transparent bg-slate-100/70 py-0 pl-8 pr-8 text-xs font-semibold text-slate-700 outline-none transition-all hover:bg-slate-100 focus:border-slate-300 focus:bg-white focus:ring-2 focus:ring-slate-200 dark:bg-white/[0.04] dark:text-slate-200 dark:hover:bg-white/[0.07] dark:focus:border-slate-700 dark:focus:bg-slate-950 dark:focus:ring-slate-800"
                >
                  <option value={allCollectionsFilter}>All collections</option>
                  {unassignedCollectionCount > 0 && (
                    <option value={unassignedCollectionFilter}>
                      Unassigned ({unassignedCollectionCount})
                    </option>
                  )}
                  {collectionOptions.map((collection) => (
                    <option
                      key={collection.name}
                      value={collectionFilterValue(collection.name)}
                    >
                      {collection.name} ({collection.count})
                    </option>
                  ))}
                </select>
                <PremiumIcon
                  icon={ChevronDown}
                  className="pointer-events-none absolute right-2.5 top-1/2 h-3.5 w-3.5 -translate-y-1/2 text-slate-400 dark:text-slate-500"
                />
              </div>
            </div>
          </div>
        </section>

        {(message || error) && (
          <div
            className={
              error
                ? "mt-4 whitespace-pre-wrap break-words rounded-lg border border-red-200 bg-red-50 px-4 py-3 text-sm text-red-700 dark:border-red-900 dark:bg-red-950 dark:text-red-200"
                : "mt-4 whitespace-pre-wrap break-words rounded-lg border border-emerald-200 bg-emerald-50 px-4 py-3 text-sm text-emerald-700 dark:border-emerald-900 dark:bg-emerald-950 dark:text-emerald-200"
            }
          >
            {error ?? message}
          </div>
        )}

        <section
          ref={sessionsGridRef}
          className="mt-5 grid flex-1 auto-rows-max content-start items-start gap-4 md:grid-cols-2 xl:grid-cols-3"
        >
          {isInitialSessionLoad ? (
            <Card className="col-span-full p-8 text-center text-sm text-slate-500 dark:text-slate-400">
              Loading sessions...
            </Card>
          ) : searching && hasSearchQuery && displayedSessions.length === 0 ? (
            <Card className="col-span-full flex items-center justify-center gap-2 p-8 text-sm text-slate-500 dark:text-slate-400">
              <PremiumIcon icon={Loader2} className="h-4 w-4 animate-spin" />
              Searching chat history...
            </Card>
          ) : displayedSessions.length === 0 ? (
            <Card className="col-span-full p-8 text-center">
              <h2 className="font-medium">{emptyTitle}</h2>
              <p className="mt-2 text-sm text-slate-500 dark:text-slate-400">
                {emptyDescription}
              </p>
            </Card>
          ) : (
            displayedSessions.map((session) => {
              const currentSessionKey = sessionKey(session);
              const isSelected = currentSessionKey === selectedSessionKey;
              const searchMatch = searchMatchesBySession.get(
                currentSessionKey,
              );
              const collectionStyle = collectionColorStyle(
                session.collectionColor,
              );

              return (
                <div
                  key={currentSessionKey}
                  ref={(node) => {
                    if (node) {
                      sessionCardRefs.current.set(currentSessionKey, node);
                    } else {
                      sessionCardRefs.current.delete(currentSessionKey);
                    }
                  }}
                  role="group"
                  tabIndex={isSelected ? 0 : -1}
                  aria-current={isSelected ? "true" : undefined}
                  aria-label={`${session.displayName}, ${session.providerDisplayName}`}
                  onClick={() => setSelectedSessionKey(currentSessionKey)}
                  onFocus={() => setSelectedSessionKey(currentSessionKey)}
                  className="session-card-shell min-w-0 rounded-xl outline-none focus-visible:ring-2 focus-visible:ring-violet-300 focus-visible:ring-offset-2 focus-visible:ring-offset-slate-50 dark:focus-visible:ring-violet-500 dark:focus-visible:ring-offset-slate-950"
                >
                  <Card
                    className={cn(
                      "group relative flex flex-col overflow-hidden p-0 transition-all hover:shadow-md",
                      hasSearchQuery ? "h-[27rem]" : "h-[32rem]",
                      session.isPinned &&
                        "border-amber-200 ring-1 ring-amber-200/70 dark:border-amber-900/80 dark:ring-amber-900/50",
                      isSelected &&
                        "border-violet-300 ring-2 ring-violet-300/80 dark:border-violet-500 dark:ring-violet-500/70",
                      session.isHidden &&
                        "border-dashed bg-slate-100/80 opacity-70 grayscale dark:bg-slate-900/60",
                    )}
                  >
                    <div
                      className={cn(
                        "h-1 shrink-0",
                        isSelected
                          ? "bg-violet-500 dark:bg-violet-400"
                          : session.isPinned
                            ? "bg-amber-400 dark:bg-amber-500"
                            : (collectionStyle?.topBarClassName ??
                              "bg-transparent"),
                      )}
                    />

                    <div className="px-4 pt-4">
                      <div className="flex items-start justify-between gap-3">
                        <div className="flex min-w-0 flex-wrap gap-2">
                          <Badge>{session.providerDisplayName}</Badge>
                          {session.note && (
                            <Badge className="border-emerald-200 bg-emerald-50 text-emerald-700 dark:border-emerald-950 dark:bg-emerald-950/35 dark:text-emerald-300">
                              <PremiumIcon
                                icon={StickyNote}
                                className="mr-1 h-3 w-3"
                              />
                              Note
                            </Badge>
                          )}
                          {session.isPinned && (
                            <Badge className="border-amber-200 bg-amber-50 text-amber-700 dark:border-amber-900 dark:bg-amber-950/35 dark:text-amber-300">
                              <PremiumIcon icon={Pin} className="mr-1 h-3 w-3" />
                              Pinned
                            </Badge>
                          )}
                          {session.isHidden && <Badge>Hidden</Badge>}
                        </div>
                        {!session.canResume && (
                          <Badge className="shrink-0">CLI not detected</Badge>
                        )}
                      </div>

                      <div className="mt-3 border-b border-slate-200 pb-4 dark:border-slate-800">
                        <div className="flex items-start gap-2">
                          <h2
                            title={session.displayName}
                            className="line-clamp-2 min-w-0 flex-1 break-words text-lg font-semibold leading-6 text-slate-950 dark:text-slate-50"
                          >
                            {session.displayName}
                          </h2>
                          {!session.isHidden && (
                            <IconActionButton
                              title="Rename session (F2)"
                              aria-keyshortcuts="F2"
                              onClick={() => openRename(session)}
                              icon={SquarePen}
                              className="mt-0.5 h-8 w-8 shrink-0 rounded-lg"
                            />
                          )}
                        </div>
                      </div>
                    </div>

                    <SessionMetadataPanel
                      session={session}
                      collectionStyle={collectionStyle}
                      onCopySessionId={() => void copySessionId(session)}
                      onCopyResumeCommand={() => void copyResumeCommand(session)}
                      onOpenBranchCorrection={() => openBranchCorrection(session)}
                      onToggleFavoriteProject={() =>
                        void toggleProjectFavorite(session)
                      }
                      onOpenCollection={() => openCollection(session)}
                    />

                    {hasSearchQuery ? (
                      searchMatch && (
                        <div className="mx-4 mt-3">
                          <SearchMatchPreview snippet={searchMatch.snippet} />
                        </div>
                      )
                    ) : (
                      <div className="mx-4 mt-3">
                        <SessionPreview
                          firstUserInput={session.firstUserInput}
                          lastUserInput={session.lastUserInput}
                          lastMessagePreview={session.lastMessagePreview}
                          onOpenHistory={() => void openSessionHistory(session)}
                        />
                      </div>
                    )}

                    <div className="mt-auto border-t border-slate-200 bg-slate-50/80 p-3 dark:border-slate-800 dark:bg-white/[0.02]">
                      <div className="flex items-center justify-between gap-2">
                        <div className="flex items-center gap-1.5 rounded-lg border border-slate-200 bg-white p-1 shadow-sm shadow-slate-950/5 dark:border-slate-800 dark:bg-slate-950 dark:shadow-black/20">
                          <IconActionButton
                            title={
                              session.note
                                ? "Edit note (N)"
                                : "Add note (N)"
                            }
                            aria-keyshortcuts="N"
                            onClick={() => openNote(session)}
                            icon={session.note ? StickyNote : StickyNotePlus}
                            tone={session.note ? "note" : "neutral"}
                            active={Boolean(session.note)}
                          />
                          <IconActionButton
                            title={
                              session.tags.length > 0
                                ? "Edit tags (T)"
                                : "Add tags (T)"
                            }
                            aria-keyshortcuts="T"
                            onClick={() => openTags(session)}
                            icon={session.tags.length > 0 ? Tags : TagPlus}
                            tone={session.tags.length > 0 ? "tag" : "neutral"}
                            active={session.tags.length > 0}
                          />
                          <IconActionButton
                            title={
                              session.collection
                                ? "Change collection (C)"
                                : "Add to collection (C)"
                            }
                            aria-keyshortcuts="C"
                            onClick={() => openCollection(session)}
                            icon={session.collection ? FolderOpen : FolderPlus}
                            tone={session.collection ? "collection" : "neutral"}
                            active={Boolean(session.collection)}
                          />
                          <IconActionButton
                            title={
                              session.isPinned
                                ? "Unpin session (P)"
                                : "Pin session (P)"
                            }
                            aria-keyshortcuts="P"
                            onClick={() => void toggleSessionPinned(session)}
                            icon={Pin}
                            tone={session.isPinned ? "accent" : "neutral"}
                            active={session.isPinned}
                          />
                        </div>

                        <div className="flex items-center gap-1.5">
                          {session.isHidden ? (
                            <Button
                              variant="secondary"
                              title="Unhide session (U)"
                              aria-keyshortcuts="U"
                              onClick={() => void handleUnhide(session)}
                              className="h-9 gap-2 rounded-lg px-3 shadow-sm"
                            >
                              <PremiumIcon icon={Eye} className="h-4 w-4" />
                              Unhide
                            </Button>
                          ) : (
                            <>
                              <Button
                                disabled={!session.canResume}
                                title={
                                  session.canResume
                                    ? "Resume this session (Cmd/Ctrl+Enter)"
                                    : `${session.providerDisplayName} is not installed or not available on PATH`
                                }
                                aria-keyshortcuts="Control+Enter Meta+Enter"
                                onClick={() => void handleResume(session)}
                                className="h-9 gap-2 rounded-lg px-3 shadow-sm"
                              >
                                <PremiumIcon icon={SquareTerminal} className="h-4 w-4" />
                                Resume
                              </Button>
                              <IconActionButton
                                title={
                                  session.canDelete
                                    ? "Delete session (Del)"
                                    : "Hide session (Del)"
                                }
                                aria-keyshortcuts="Delete"
                                onClick={() => setDeleteTarget(session)}
                                icon={session.canDelete ? Trash : EyeOff}
                                tone={session.canDelete ? "danger" : "neutral"}
                                className="h-9 w-9 rounded-lg"
                              />
                            </>
                          )}
                        </div>
                      </div>
                    </div>
                  </Card>
                </div>
              );
            })
          )}
        </section>
      </div>

      {commandPaletteOpen && (
        <CommandPalette
          commands={commandPaletteCommands}
          query={commandPaletteQuery}
          onClose={closeCommandPalette}
          onQueryChange={setCommandPaletteQuery}
          onRun={runCommandPaletteCommand}
        />
      )}

      {renameTarget && (
        <Modal title="Rename session" onClose={() => setRenameTarget(null)}>
          <form
            className="space-y-4"
            onSubmit={(event) => {
              event.preventDefault();
              void saveRename();
            }}
          >
            <div>
              <label className="text-sm font-medium">Custom session name</label>
              <Input
                className={cn(
                  "mt-2",
                  renameNameError &&
                    "border-red-300 focus:border-red-400 focus:ring-2 focus:ring-red-100 dark:border-red-900 dark:focus:border-red-700 dark:focus:ring-red-950/50",
                )}
                value={renameValue}
                onChange={(event) => setRenameValue(event.target.value)}
                maxLength={customSessionNameMaxLength}
                aria-invalid={Boolean(renameNameError)}
                placeholder={renameTarget.displayName}
                title={renameTarget.displayName}
                autoFocus
              />
              {!renameTarget.friendlyName && (
                <div className="mt-2 flex min-w-0 items-center gap-2 rounded-lg border border-slate-200 bg-slate-50/80 px-3 py-2 text-xs dark:border-slate-800 dark:bg-white/[0.03]">
                  <span className="shrink-0 font-medium text-slate-500 dark:text-slate-400">
                    Provider title
                  </span>
                  <span
                    title={renameTarget.displayName}
                    className="min-w-0 flex-1 truncate font-semibold text-slate-700 dark:text-slate-200"
                  >
                    {renameTarget.displayName}
                  </span>
                </div>
              )}
              <FieldLimitMeter
                count={renameNameLength}
                max={customSessionNameMaxLength}
                error={renameNameError}
                helper={
                  renameTarget.friendlyName
                    ? "Leave blank to remove the custom SessionDex name."
                    : "Leave blank to keep using the provider title."
                }
              />
            </div>
            <div className="flex justify-end gap-2">
              <Button variant="ghost" onClick={() => setRenameTarget(null)}>
                Cancel
              </Button>
              <Button type="submit" disabled={Boolean(renameNameError)}>
                Save
              </Button>
            </div>
          </form>
        </Modal>
      )}

      {collectionTarget && (
        <Modal
          title={
            collectionTarget.collection ? "Change collection" : "Add collection"
          }
          onClose={() => setCollectionTarget(null)}
        >
          <form
            className="space-y-4"
            onSubmit={(event) => {
              event.preventDefault();
              void saveCollection();
            }}
          >
            <div>
              <label className="text-sm font-medium">Collection</label>
              <Input
                className={cn(
                  "mt-2",
                  collectionNameError &&
                    "border-red-300 focus:border-red-400 focus:ring-2 focus:ring-red-100 dark:border-red-900 dark:focus:border-red-700 dark:focus:ring-red-950/50",
                )}
                value={collectionValue}
                onChange={(event) => setCollectionValue(event.target.value)}
                list="session-collection-options"
                placeholder="Production Issues"
                maxLength={collectionNameMaxLength}
                aria-invalid={Boolean(collectionNameError)}
                autoFocus
              />
              {collectionNames.length > 0 && (
                <datalist id="session-collection-options">
                  {collectionNames.map((collectionName) => (
                    <option key={collectionName} value={collectionName} />
                  ))}
                </datalist>
              )}
              <fieldset className="mt-4">
                <legend className="text-sm font-medium">Color</legend>
                <div className="mt-2 flex flex-wrap gap-2">
                  {collectionColorChoices.map((color) => {
                    const isActive = collectionColorValue === color.value;
                    const colorStyle =
                      color.value === "none"
                        ? null
                        : collectionColorStyles[color.value];

                    return (
                      <button
                        key={color.value}
                        type="button"
                        title={color.label}
                        aria-label={color.label}
                        aria-pressed={isActive}
                        onClick={() => {
                          setCollectionColorTouched(true);
                          setCollectionColorValue(color.value);
                        }}
                        className={cn(
                          "flex h-8 w-8 items-center justify-center rounded-md border bg-white transition-all hover:-translate-y-px hover:shadow-sm focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-slate-300 dark:bg-slate-950 dark:focus-visible:ring-slate-700",
                          isActive
                            ? "border-slate-400 ring-2 ring-slate-300 dark:border-slate-500 dark:ring-slate-700"
                            : "border-slate-200 hover:border-slate-300 dark:border-slate-800 dark:hover:border-slate-700",
                        )}
                      >
                        <span
                          aria-hidden="true"
                          className={cn(
                            "h-3.5 w-3.5 rounded-full",
                            colorStyle
                              ? colorStyle.swatchClassName
                              : "border border-slate-300 bg-transparent dark:border-slate-600",
                          )}
                        />
                      </button>
                    );
                  })}
                </div>
              </fieldset>
              <FieldLimitMeter
                count={collectionNameLength}
                max={collectionNameMaxLength}
                error={collectionNameError}
                helper="Leave blank to remove this session from its collection."
              />
            </div>
            <div className="flex justify-end gap-2">
              <Button
                variant="ghost"
                onClick={() => setCollectionTarget(null)}
              >
                Cancel
              </Button>
              <Button type="submit" disabled={Boolean(collectionNameError)}>
                Save
              </Button>
            </div>
          </form>
        </Modal>
      )}

      {branchTarget && (
        <Modal title="Session Branch" onClose={closeBranchCorrection}>
          <form
            className="space-y-4"
            onSubmit={(event) => {
              event.preventDefault();
              void saveBranchCorrection();
            }}
          >
            <div>
              <div className="flex items-center justify-between gap-3">
                <label className="text-sm font-medium">Branch</label>
                <span className="rounded-full border border-slate-200 bg-slate-50 px-2 py-0.5 text-[11px] font-semibold text-slate-500 dark:border-slate-800 dark:bg-slate-900/60 dark:text-slate-400">
                  Metadata
                </span>
              </div>
              <div className="relative mt-2">
                <PremiumIcon
                  icon={GitBranch}
                  className="pointer-events-none absolute left-3 top-1/2 h-4 w-4 -translate-y-1/2 text-slate-400 dark:text-slate-500"
                />
                <Input
                  value={branchValue}
                  onChange={(event) => {
                    setBranchValue(event.target.value);
                    setBranchError(null);
                  }}
                  role="combobox"
                  aria-autocomplete="list"
                  aria-controls="session-branch-options"
                  aria-expanded={branchOptions.length > 0}
                  placeholder="Search or type a branch"
                  autoFocus
                  className="pl-10 pr-9 font-mono"
                />
                <PremiumIcon
                  icon={ChevronDown}
                  className="pointer-events-none absolute right-3 top-1/2 h-4 w-4 -translate-y-1/2 text-slate-400 dark:text-slate-500"
                />
              </div>
              {filteredBranchOptions.length > 0 && (
                <div
                  id="session-branch-options"
                  role="listbox"
                  aria-label="Branch suggestions"
                  className="sessiondex-scrollbar mt-2 max-h-60 overflow-y-scroll rounded-lg border border-slate-200 bg-white p-1 shadow-sm shadow-slate-950/5 dark:border-slate-800 dark:bg-slate-950 dark:shadow-black/20"
                >
                  <div className="flex items-center justify-between gap-3 px-2 py-1 text-[10px] font-semibold uppercase tracking-wide text-slate-400 dark:text-slate-500">
                    <span>Branch suggestions</span>
                    <span className="font-mono normal-case tracking-normal">
                      {filteredBranchOptions.length}/{branchOptions.length}
                    </span>
                  </div>
                  {filteredBranchOptions.map((branchName) => (
                    <button
                      key={branchName}
                      type="button"
                      role="option"
                      aria-selected={branchName === sessionBranchValue}
                      title={branchName}
                      onClick={() => {
                        setBranchValue(branchName);
                        setBranchError(null);
                      }}
                      className={cn(
                        "flex h-9 w-full min-w-0 items-center gap-2 rounded-md px-2 text-left text-sm font-medium text-slate-700 transition-colors hover:bg-slate-100 hover:text-slate-950 focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-slate-300 dark:text-slate-200 dark:hover:bg-slate-900 dark:hover:text-white dark:focus-visible:ring-slate-700",
                        branchName === sessionBranchValue &&
                          "bg-sky-50 text-sky-800 dark:bg-sky-950/35 dark:text-sky-200",
                      )}
                    >
                      <PremiumIcon
                        icon={GitBranch}
                        className="h-3.5 w-3.5 text-slate-400 dark:text-slate-500"
                      />
                      <span className="min-w-0 truncate">{branchName}</span>
                      {branchName === sessionBranchValue && (
                        <span className="ml-auto shrink-0 rounded-full bg-sky-100 px-1.5 py-0.5 text-[10px] font-semibold text-sky-700 dark:bg-sky-950 dark:text-sky-200">
                          Selected
                        </span>
                      )}
                    </button>
                  ))}
                </div>
              )}
              {sessionBranchIsTyped && (
                <div className="mt-2 flex min-w-0 items-center gap-2 rounded-lg border border-slate-200 bg-slate-50/80 px-3 py-2 text-xs dark:border-slate-800 dark:bg-white/[0.03]">
                  <PremiumIcon
                    icon={SquarePen}
                    className="h-3.5 w-3.5 text-slate-400 dark:text-slate-500"
                  />
                  <span className="shrink-0 font-medium text-slate-500 dark:text-slate-400">
                    Typed branch
                  </span>
                  <span
                    title={sessionBranchValue}
                    className="min-w-0 flex-1 truncate font-mono font-semibold text-slate-700 dark:text-slate-200"
                  >
                    {sessionBranchValue}
                  </span>
                </div>
              )}
              {!branchOptionsLoading &&
                branchOptions.length > 0 &&
                filteredBranchOptions.length === 0 && (
                  <p className="mt-2 text-xs text-slate-500 dark:text-slate-400">
                    No matching branch suggestions. This branch can still be saved.
                  </p>
                )}
              {branchOptionsLoading && (
                <p className="mt-2 text-xs text-slate-500 dark:text-slate-400">
                  Loading branch suggestions...
                </p>
              )}
              <p className="mt-2 text-xs text-slate-500 dark:text-slate-400">
                {sessionBranchChanged
                  ? "Saving confirms this branch for the session. Your repository checkout is not changed."
                  : "Discovered by SessionDex unless you edit and save it. Your repository checkout is not changed."}
              </p>
              {branchError && (
                <p className="mt-2 text-xs text-red-600 dark:text-red-300">
                  {branchError}
                </p>
              )}
            </div>
            <div className="flex justify-end gap-2">
              <Button variant="ghost" onClick={closeBranchCorrection}>
                Cancel
              </Button>
              <Button type="submit" disabled={!branchValue.trim()}>
                Save branch
              </Button>
            </div>
          </form>
        </Modal>
      )}

      {noteTarget && (
        <Modal
          title={noteTarget.note ? "Edit note" : "Add note"}
          onClose={() => setNoteTarget(null)}
        >
          <form
            className="space-y-4"
            onSubmit={(event) => {
              event.preventDefault();
              void saveNote();
            }}
          >
            <div>
              <label className="text-sm font-medium">Note</label>
              <textarea
                spellCheck={false}
                autoCorrect="off"
                autoCapitalize="off"
                className="mt-2 min-h-36 w-full resize-y rounded-md border border-slate-200 bg-white px-3 py-2 text-sm leading-6 outline-none transition-colors placeholder:text-slate-400 focus:border-slate-400 dark:border-slate-800 dark:bg-slate-950 dark:text-slate-100 dark:placeholder:text-slate-500 dark:focus:border-slate-600"
                value={noteValue}
                onChange={(event) => setNoteValue(event.target.value)}
                placeholder={
                  "Production fix\nCustomer waiting for verification\nPR #284"
                }
                autoFocus
              />
              <p className="mt-2 text-xs text-slate-500 dark:text-slate-400">
                Leave blank to remove this session note.
              </p>
            </div>
            <div className="flex justify-end gap-2">
              <Button variant="ghost" onClick={() => setNoteTarget(null)}>
                Cancel
              </Button>
              <Button type="submit">Save</Button>
            </div>
          </form>
        </Modal>
      )}

      {tagsTarget && (
        <Modal
          title={tagsTarget.tags.length > 0 ? "Edit tags" : "Add tags"}
          onClose={closeTags}
        >
          <form
            className="space-y-4"
            onSubmit={(event) => {
              event.preventDefault();
              void saveTags();
            }}
          >
            <div>
              <label className="text-sm font-medium">Tags</label>
              <div className="mt-2 flex gap-2">
                <Input
                  value={tagValue}
                  onChange={(event) => setTagValue(event.target.value)}
                  onKeyDown={(event) => {
                    if (event.key === "Enter" || event.key === ",") {
                      event.preventDefault();
                      addTagsFromValue(tagValue);
                    }
                  }}
                  list="session-tag-options"
                  placeholder="java, postgres, deployment"
                  aria-invalid={Boolean(tagInputError)}
                  className={cn(
                    tagInputError &&
                      "border-red-300 focus:border-red-400 focus:ring-2 focus:ring-red-100 dark:border-red-900 dark:focus:border-red-700 dark:focus:ring-red-950/50",
                  )}
                  autoFocus
                />
                <Button
                  type="button"
                  variant="secondary"
                  onClick={() => addTagsFromValue(tagValue)}
                  disabled={!canAddTagInput}
                  className="shrink-0 gap-2"
                >
                  <PremiumIcon icon={TagPlus} className="h-4 w-4" />
                  Add
                </Button>
              </div>
              <FieldLimitMeter
                count={tagInputLongestLength}
                max={tagNameMaxLength}
                error={tagInputError}
                helper="Separate tags with commas or spaces."
                suffix=" per tag"
              />
              {allTagNames.length > 0 && (
                <datalist id="session-tag-options">
                  {allTagNames.map((tagName) => (
                    <option key={tagName} value={tagName} />
                  ))}
                </datalist>
              )}

              <div className="mt-3 min-h-10 rounded-lg border border-slate-200 bg-slate-50/80 p-2 dark:border-slate-800 dark:bg-white/[0.03]">
                {tagDraft.length > 0 ? (
                  <div className="flex flex-wrap gap-2">
                    {tagDraft.map((tag) => (
                      <button
                        key={tag}
                        type="button"
                        title={`Remove ${tag}`}
                        onClick={() => removeTag(tag)}
                        className="inline-flex h-7 min-w-0 items-center gap-1.5 rounded-md border border-slate-200 bg-white px-2 text-xs font-semibold text-slate-600 transition-colors hover:border-slate-300 hover:bg-slate-100 hover:text-slate-950 focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-slate-300 dark:border-slate-800 dark:bg-slate-950 dark:text-slate-300 dark:hover:border-slate-700 dark:hover:bg-slate-900 dark:hover:text-white dark:focus-visible:ring-slate-700"
                      >
                        <span className="max-w-36 truncate">{tag}</span>
                        <PremiumIcon icon={X} className="h-3 w-3" />
                      </button>
                    ))}
                  </div>
                ) : (
                  <p className="px-1 py-1 text-sm text-slate-500 dark:text-slate-400">
                    No tags
                  </p>
                )}
              </div>
            </div>

            <div className="flex justify-end gap-2">
              <Button variant="ghost" onClick={closeTags}>
                Cancel
              </Button>
              <Button type="submit" disabled={Boolean(tagInputError)}>
                Save
              </Button>
            </div>
          </form>
        </Modal>
      )}

      {deleteTarget && (
        <Modal
          title={deleteTarget.canDelete ? "Delete session" : "Hide session"}
          onClose={() => setDeleteTarget(null)}
        >
          <div className="space-y-4">
            <p className="text-sm text-slate-600 dark:text-slate-300">
              {deleteTarget.canDelete
                ? "This provider supports deletion. SessionDex will ask the provider to delete this session."
                : "This provider does not expose safe deletion. SessionDex will hide this session from its own dashboard only."}
            </p>
            <div className="rounded-lg bg-slate-100 p-3 font-mono text-xs text-slate-600 dark:bg-slate-900 dark:text-slate-300">
              {deleteTarget.sessionId}
            </div>
            <div className="flex justify-end gap-2">
              <Button variant="ghost" onClick={() => setDeleteTarget(null)}>
                Cancel
              </Button>
              <Button
                variant={deleteTarget.canDelete ? "danger" : "primary"}
                onClick={() => void confirmDeleteOrHide()}
              >
                {deleteTarget.canDelete ? "Delete" : "Hide"}
              </Button>
            </div>
          </div>
        </Modal>
      )}

      {settingsOpen && (
        <SettingsModal
          settings={settings}
          providers={providers}
          onClose={() => setSettingsOpen(false)}
          onSave={(nextSettings) => void saveSettings(nextSettings)}
        />
      )}

      {historyTarget && (
        <SessionHistoryModal
          session={historyTarget}
          history={sessionHistory}
          loading={historyLoading}
          error={historyError}
          onClose={closeSessionHistory}
        />
      )}

      {toast && (
        <Toast
          toast={toast}
          onClose={() => setToast(null)}
        />
      )}
    </main>
  );
}

function Toast({
  toast,
  onClose,
}: {
  toast: ToastState;
  onClose: () => void;
}) {
  return (
    <div className="fixed bottom-5 right-5 z-50 w-[min(420px,calc(100vw-2.5rem))]">
      <div
        className={cn(
          "rounded-xl border bg-white p-4 shadow-lg dark:bg-slate-950",
          toast.tone === "success" &&
            "border-emerald-200 dark:border-emerald-900",
          toast.tone === "error" && "border-red-200 dark:border-red-900",
        )}
      >
        <div className="flex items-start justify-between gap-3">
          <div className="min-w-0">
            <p className="text-sm font-medium">{toast.title}</p>
            <pre className="mt-2 max-h-32 overflow-auto whitespace-pre-wrap break-words rounded-lg bg-slate-100 p-2 font-mono text-[11px] leading-5 text-slate-700 dark:bg-slate-900 dark:text-slate-200">
              {toast.description}
            </pre>
          </div>
          <button
            type="button"
            onClick={onClose}
            className="rounded-md px-2 py-1 text-xs text-slate-500 transition-colors hover:bg-slate-100 hover:text-slate-900 dark:text-slate-400 dark:hover:bg-slate-900 dark:hover:text-slate-100"
          >
            Close
          </button>
        </div>
      </div>
    </div>
  );
}

function CommandPalette({
  commands,
  query,
  onClose,
  onQueryChange,
  onRun,
}: {
  commands: CommandPaletteCommand[];
  query: string;
  onClose: () => void;
  onQueryChange: (query: string) => void;
  onRun: (command: CommandPaletteCommand) => void;
}) {
  const [activeIndex, setActiveIndex] = useState(0);
  const inputRef = useRef<HTMLInputElement>(null);
  const activeCommandRef = useRef<HTMLButtonElement | null>(null);
  const visibleCommands = useMemo(() => {
    return commands
      .filter((command) => commandMatchesQuery(command, query))
      .slice(0, 80);
  }, [commands, query]);

  useBodyScrollLock();

  useEffect(() => {
    inputRef.current?.focus();
  }, []);

  useEffect(() => {
    const firstEnabledIndex = visibleCommands.findIndex(
      (command) => !command.disabled,
    );

    setActiveIndex(firstEnabledIndex >= 0 ? firstEnabledIndex : 0);
  }, [query, visibleCommands.length]);

  useEffect(() => {
    activeCommandRef.current?.scrollIntoView({
      block: "nearest",
      inline: "nearest",
    });
  }, [activeIndex]);

  function moveActiveIndex(offset: number) {
    if (visibleCommands.length === 0) {
      return;
    }

    setActiveIndex((currentIndex) => {
      for (let step = 1; step <= visibleCommands.length; step += 1) {
        const nextIndex =
          (currentIndex + offset * step + visibleCommands.length) %
          visibleCommands.length;

        if (!visibleCommands[nextIndex]?.disabled) {
          return nextIndex;
        }
      }

      return currentIndex;
    });
  }

  function runActiveCommand() {
    const activeCommand = visibleCommands[activeIndex];

    if (!activeCommand || activeCommand.disabled) {
      return;
    }

    onRun(activeCommand);
  }

  function handleKeyDown(event: React.KeyboardEvent<HTMLDivElement>) {
    event.stopPropagation();

    if (
      (event.metaKey || event.ctrlKey) &&
      event.shiftKey &&
      !event.altKey &&
      event.key.toLowerCase() === "p"
    ) {
      event.preventDefault();
      onClose();
      return;
    }

    if (event.key === "Escape") {
      event.preventDefault();
      onClose();
      return;
    }

    if (event.key === "ArrowDown") {
      event.preventDefault();
      moveActiveIndex(1);
      return;
    }

    if (event.key === "ArrowUp") {
      event.preventDefault();
      moveActiveIndex(-1);
      return;
    }

    if (event.key === "Enter") {
      event.preventDefault();
      runActiveCommand();
    }
  }

  return (
    <div
      role="dialog"
      aria-modal="true"
      aria-label="Command palette"
      className="fixed inset-0 z-[60] bg-slate-950/45 px-4 py-16 backdrop-blur-sm"
      onKeyDown={handleKeyDown}
      onMouseDown={(event) => {
        if (event.target === event.currentTarget) {
          onClose();
        }
      }}
    >
      <div className="mx-auto flex max-h-[min(42rem,calc(100vh-8rem))] w-full max-w-2xl flex-col overflow-hidden rounded-xl border border-slate-200 bg-white shadow-2xl shadow-slate-950/30 dark:border-slate-800 dark:bg-slate-950 dark:shadow-black/60">
        <div className="flex shrink-0 items-center gap-3 border-b border-slate-200 px-4 py-3 dark:border-slate-800">
          <PremiumIcon
            icon={Search}
            className="h-4 w-4 text-slate-400 dark:text-slate-500"
          />
          <input
            ref={inputRef}
            spellCheck={false}
            autoCorrect="off"
            autoCapitalize="off"
            value={query}
            onChange={(event) => onQueryChange(event.target.value)}
            placeholder="Search commands or sessions"
            className="h-10 min-w-0 flex-1 bg-transparent text-sm font-medium text-slate-950 outline-none placeholder:text-slate-400 dark:text-slate-50 dark:placeholder:text-slate-500"
          />
          <button
            type="button"
            onClick={onClose}
            className="flex h-8 w-8 shrink-0 items-center justify-center rounded-md text-slate-400 transition-colors hover:bg-slate-100 hover:text-slate-950 focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-slate-300 dark:hover:bg-slate-900 dark:hover:text-slate-50 dark:focus-visible:ring-slate-700"
          >
            <PremiumIcon icon={X} className="h-4 w-4" />
          </button>
        </div>

        <div className="min-h-0 flex-1 overflow-y-auto p-2">
          {visibleCommands.length === 0 ? (
            <div className="px-4 py-8 text-center text-sm text-slate-500 dark:text-slate-400">
              No commands found
            </div>
          ) : (
            <div className="space-y-1">
              {visibleCommands.map((command, index) => {
                const isActive = index === activeIndex;

                return (
                  <button
                    key={command.id}
                    ref={isActive ? activeCommandRef : undefined}
                    type="button"
                    disabled={command.disabled}
                    onMouseEnter={() => setActiveIndex(index)}
                    onClick={() => onRun(command)}
                    className={cn(
                      "flex w-full min-w-0 items-center gap-3 rounded-lg px-3 py-2.5 text-left transition-colors focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-slate-300 disabled:cursor-not-allowed disabled:opacity-45 dark:focus-visible:ring-slate-700",
                      isActive
                        ? "bg-slate-100 text-slate-950 dark:bg-slate-900 dark:text-slate-50"
                        : "text-slate-700 hover:bg-slate-50 dark:text-slate-200 dark:hover:bg-slate-900/70",
                    )}
                  >
                    <span className="flex h-8 w-8 shrink-0 items-center justify-center rounded-md border border-slate-200 bg-white text-slate-500 dark:border-slate-800 dark:bg-slate-950 dark:text-slate-400">
                      <PremiumIcon icon={command.icon} className="h-4 w-4" />
                    </span>
                    <span className="min-w-0 flex-1">
                      <span className="block truncate text-sm font-semibold">
                        {command.label}
                      </span>
                      {command.detail && (
                        <span className="mt-0.5 block truncate text-xs text-slate-500 dark:text-slate-400">
                          {command.detail}
                        </span>
                      )}
                    </span>
                  </button>
                );
              })}
            </div>
          )}
        </div>
      </div>
    </div>
  );
}

function SessionViewButton({
  active,
  count,
  icon,
  label,
  onClick,
}: {
  active: boolean;
  count: number;
  icon?: React.ReactNode;
  label: string;
  onClick: () => void;
}) {
  return (
    <button
      type="button"
      aria-pressed={active}
      onClick={onClick}
      className={cn(
        "inline-flex h-9 min-w-0 items-center justify-center gap-1.5 rounded px-2.5 text-sm font-semibold transition-all focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-slate-300 dark:focus-visible:ring-slate-700",
        active
          ? "bg-white text-slate-950 shadow-sm shadow-slate-950/10 dark:bg-slate-100 dark:text-slate-950 dark:shadow-black/30"
          : "text-slate-500 hover:bg-white/70 hover:text-slate-950 dark:text-slate-400 dark:hover:bg-white/[0.07] dark:hover:text-slate-100",
      )}
    >
      {icon}
      <span>{label}</span>
      <span
        className={cn(
          "ml-0.5 inline-flex min-w-5 items-center justify-center rounded px-1 text-xs transition-colors",
          active
            ? "bg-slate-950/10 text-current"
            : "bg-white/70 text-slate-500 dark:bg-white/[0.05] dark:text-slate-400",
        )}
      >
        {count}
      </span>
    </button>
  );
}

function SessionStatisticsPopover({
  statistics,
}: {
  statistics: SessionStatistics;
}) {
  return (
    <div
      id="session-statistics-popover"
      role="region"
      aria-label="Session statistics"
      className="absolute right-0 top-11 z-30 w-[20rem] max-w-[calc(100vw-2rem)] overflow-hidden rounded-lg border border-slate-200 bg-white shadow-2xl shadow-slate-950/15 dark:border-slate-800 dark:bg-slate-950 dark:shadow-black/40"
    >
      <div className="flex items-center justify-between gap-3 border-b border-slate-200 px-3.5 py-3 dark:border-slate-800">
        <div className="flex min-w-0 items-center gap-2">
          <span className="flex h-7 w-7 shrink-0 items-center justify-center rounded-md bg-slate-100 text-slate-600 dark:bg-slate-900 dark:text-slate-300">
            <PremiumIcon icon={BarChart3} className="h-4 w-4" />
          </span>
          <h2 className="truncate text-sm font-semibold">
            Session Statistics
          </h2>
        </div>
      </div>

      <div className="space-y-1 p-2">
        <StatisticRow
          label="Total Sessions"
          value={statistics.totalSessions}
          strong
        />

        <div className="my-1 border-t border-slate-200 dark:border-slate-800" />

        {statistics.providerCounts.map((provider) => (
          <StatisticRow
            key={provider.id}
            label={`${provider.label} Sessions`}
            value={provider.count}
          />
        ))}

        <div className="my-1 border-t border-slate-200 dark:border-slate-800" />

        <StatisticRow label="Pinned" value={statistics.pinnedSessions} />
        <StatisticRow label="Hidden" value={statistics.hiddenSessions} />
        <StatisticRow label="Collections" value={statistics.collectionCount} />
        <StatisticRow label="Tags" value={statistics.tagCount} />
      </div>
    </div>
  );
}

function StatisticRow({
  label,
  value,
  strong = false,
}: {
  label: string;
  value: number;
  strong?: boolean;
}) {
  return (
    <div
      className={cn(
        "flex h-8 items-center justify-between gap-4 rounded-md px-2.5 text-sm",
        strong
          ? "bg-slate-100 text-slate-950 dark:bg-slate-900 dark:text-slate-50"
          : "text-slate-600 dark:text-slate-300",
      )}
    >
      <span className="min-w-0 truncate font-medium">{label}</span>
      <span
        className={cn(
          "shrink-0 font-mono text-xs tabular-nums",
          strong
            ? "font-semibold text-slate-950 dark:text-slate-50"
            : "text-slate-500 dark:text-slate-400",
        )}
      >
        {value.toLocaleString()}
      </span>
    </div>
  );
}

function SessionMetadataPanel({
  session,
  collectionStyle,
  onCopySessionId,
  onCopyResumeCommand,
  onOpenBranchCorrection,
  onToggleFavoriteProject,
  onOpenCollection,
}: {
  session: SessionRecord;
  collectionStyle: CollectionColorStyle | null;
  onCopySessionId: () => void;
  onCopyResumeCommand: () => void;
  onOpenBranchCorrection: () => void;
  onToggleFavoriteProject: () => void;
  onOpenCollection: () => void;
}) {
  const folderName = folderNameFromPath(session.workingDirectory);
  const projectPath = projectKeyFromPath(session.workingDirectory);
  const discoveredBranch = session.discoveredBranch?.trim();
  const discoveredTitle =
    discoveredBranch && session.discoveredAt
      ? `Session branch ${discoveredBranch} · ${formatModifiedTime(session.discoveredAt)}`
      : discoveredBranch
        ? `Session branch ${discoveredBranch}`
        : undefined;
  const updatedTime = formatModifiedTime(session.lastModified);

  return (
    <div className="mx-4 mt-3 rounded-lg border border-slate-200 bg-slate-50/80 px-3 py-1.5 text-xs dark:border-slate-800 dark:bg-white/[0.03]">
      <div className="space-y-1">
        <CompactMetadataRow
          icon={FolderOpen}
          title={projectPath ?? undefined}
        >
          <span className="min-w-0 flex-1 truncate font-medium text-slate-700 dark:text-slate-200">
            {folderName ?? "No project folder"}
          </span>
          {discoveredBranch && (
            <button
              type="button"
              title={discoveredTitle}
              aria-label="Edit session branch"
              onClick={onOpenBranchCorrection}
              className="flex min-w-[4.5rem] max-w-[clamp(5rem,42%,10rem)] shrink items-center gap-1 rounded border border-slate-200 bg-slate-50 px-1.5 py-0.5 font-mono text-[0.68rem] leading-none text-slate-500 transition-colors hover:border-slate-300 hover:bg-slate-100 hover:text-slate-800 focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-slate-300 dark:border-slate-800 dark:bg-slate-900/80 dark:text-slate-400 dark:hover:border-slate-700 dark:hover:bg-slate-900 dark:hover:text-slate-200 dark:focus-visible:ring-slate-700"
            >
              <PremiumIcon icon={GitBranch} className="h-3 w-3" />
              <span className="min-w-0 truncate">{discoveredBranch}</span>
              <PremiumIcon icon={ChevronDown} className="h-3 w-3" />
            </button>
          )}
          {projectPath && (
            <button
              type="button"
              title={
                session.isFavoriteProject
                  ? "Remove project favorite"
                  : "Favorite project"
              }
              aria-label={
                session.isFavoriteProject
                  ? "Remove project favorite"
                  : "Favorite project"
              }
              aria-pressed={session.isFavoriteProject}
              onClick={onToggleFavoriteProject}
              className={cn(
                "flex h-5 w-5 shrink-0 items-center justify-center rounded-md text-slate-400 transition-colors hover:bg-slate-100 hover:text-amber-500 focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-slate-300 dark:text-slate-500 dark:hover:bg-slate-900 dark:hover:text-amber-300 dark:focus-visible:ring-slate-700",
                session.isFavoriteProject &&
                  "text-amber-500 dark:text-amber-300",
              )}
            >
              <PremiumIcon
                icon={Star}
                className={cn(
                  "h-3.5 w-3.5",
                  session.isFavoriteProject && "fill-current",
                )}
              />
            </button>
          )}
        </CompactMetadataRow>

        <CompactMetadataRow icon={Clock3} title={updatedTime}>
          <span className="truncate font-medium text-slate-600 dark:text-slate-300">
            {updatedTime}
          </span>
        </CompactMetadataRow>

        <CompactMetadataRow
          title={
            session.collection
              ? `Change collection: ${session.collection}`
              : "Add collection"
          }
        >
          {collectionStyle ? (
            <span
              aria-hidden="true"
              className={cn(
                "h-2.5 w-2.5 shrink-0 rounded-full",
                collectionStyle.dotClassName,
              )}
            />
          ) : (
            <PremiumIcon
              icon={session.collection ? FolderOpen : FolderPlus}
              className={cn(
                "h-3.5 w-3.5",
                session.collection
                  ? "text-slate-400 dark:text-slate-500"
                  : "text-sky-500 dark:text-sky-300",
              )}
            />
          )}
          <button
            type="button"
            title={session.collection ? "Edit collection" : "Add collection"}
            aria-label={
              session.collection
                ? `Edit collection: ${session.collection}`
                : "Add collection"
            }
            onClick={onOpenCollection}
            className={cn(
              "group/collection flex h-5 min-w-0 flex-1 items-center gap-1.5 rounded px-1 text-left font-medium transition-colors focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-slate-300 dark:focus-visible:ring-slate-700",
              session.collection
                ? "text-slate-600 hover:bg-slate-100 hover:text-slate-950 dark:text-slate-300 dark:hover:bg-slate-900 dark:hover:text-white"
                : "text-sky-700 hover:bg-sky-50 hover:text-sky-900 dark:text-sky-300 dark:hover:bg-sky-950/35 dark:hover:text-sky-100",
            )}
          >
            <span className="min-w-0 flex-1 truncate">
              {session.collection ?? "Add collection"}
            </span>
            <PremiumIcon
              icon={session.collection ? SquarePen : FolderPlus}
              className={cn(
                "h-3 w-3 shrink-0 opacity-70 transition-opacity group-hover/collection:opacity-100",
                session.collection
                  ? "text-slate-400 dark:text-slate-500"
                  : "text-sky-500 dark:text-sky-300",
              )}
            />
          </button>
        </CompactMetadataRow>

        <CompactMetadataRow icon={Hash} title={session.sessionId}>
          <button
            type="button"
            title="Copy session ID"
            onClick={onCopySessionId}
            className="block min-w-0 flex-1 truncate text-left font-mono text-xs text-slate-600 transition-colors hover:text-slate-950 focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-slate-300 dark:text-slate-300 dark:hover:text-white dark:focus-visible:ring-slate-700"
          >
            {session.sessionId}
          </button>
          <button
            type="button"
            title="Copy resume command"
            aria-label="Copy resume command"
            onClick={onCopyResumeCommand}
            className="flex h-6 w-6 shrink-0 items-center justify-center rounded-md border border-slate-200 bg-white text-slate-500 shadow-sm shadow-slate-950/5 transition-colors hover:border-slate-300 hover:bg-slate-50 hover:text-slate-950 focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-slate-300 dark:border-slate-800 dark:bg-slate-950 dark:text-slate-400 dark:shadow-black/20 dark:hover:border-slate-700 dark:hover:bg-slate-900 dark:hover:text-slate-50 dark:focus-visible:ring-slate-700"
          >
            <PremiumIcon icon={SquareTerminal} className="h-3.5 w-3.5" />
          </button>
        </CompactMetadataRow>
      </div>
    </div>
  );
}

function CompactMetadataRow({
  icon,
  title,
  children,
}: {
  icon?: LucideIcon;
  title?: string;
  children: React.ReactNode;
}) {
  return (
    <div
      title={title}
      className="flex h-6 min-w-0 items-center gap-2 rounded-md bg-white px-2.5 dark:bg-slate-950/70"
    >
      {icon && (
        <PremiumIcon
          icon={icon}
          className="h-3.5 w-3.5 text-slate-400 dark:text-slate-500"
        />
      )}
      {children}
    </div>
  );
}

function PremiumIcon({
  icon: Icon,
  className,
  strokeWidth = 1.75,
}: {
  icon: LucideIcon;
  className?: string;
  strokeWidth?: number;
}) {
  return (
    <Icon
      aria-hidden="true"
      className={cn("shrink-0", className)}
      strokeWidth={strokeWidth}
    />
  );
}

function HeaderActionButton({
  title,
  icon,
  iconClassName,
  className,
  ...props
}: Omit<React.ComponentProps<typeof Button>, "children" | "variant"> & {
  title: string;
  icon: LucideIcon;
  iconClassName?: string;
}) {
  return (
    <Button
      variant="ghost"
      title={title}
      aria-label={title}
      className={cn(
        "group h-9 w-9 rounded-lg border border-slate-200/80 bg-white/70 p-0 text-slate-600 shadow-sm shadow-slate-950/5 transition-all hover:-translate-y-px hover:border-slate-300 hover:bg-white hover:text-slate-950 hover:shadow-md focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-slate-300 disabled:translate-y-0 disabled:shadow-sm dark:border-white/10 dark:bg-white/[0.03] dark:text-slate-300 dark:shadow-black/20 dark:hover:border-white/15 dark:hover:bg-white/[0.07] dark:hover:text-white dark:focus-visible:ring-slate-700",
        className,
      )}
      {...props}
    >
      <PremiumIcon
        icon={icon}
        className={cn(
          "h-4 w-4 transition-transform group-hover:scale-105",
          iconClassName,
        )}
      />
    </Button>
  );
}

type IconTone = "neutral" | "accent" | "collection" | "note" | "tag" | "danger";

const iconToneClasses: Record<IconTone, string> = {
  neutral:
    "border-slate-200 bg-white text-slate-600 shadow-slate-950/5 hover:border-slate-300 hover:bg-slate-50 hover:text-slate-950 dark:border-slate-800 dark:bg-slate-950 dark:text-slate-300 dark:shadow-black/20 dark:hover:border-slate-700 dark:hover:bg-slate-900 dark:hover:text-white",
  accent:
    "border-amber-200 bg-amber-50 text-amber-700 shadow-amber-950/5 hover:border-amber-300 hover:bg-amber-100/80 dark:border-amber-900 dark:bg-amber-950/35 dark:text-amber-300 dark:shadow-black/20 dark:hover:border-amber-800 dark:hover:bg-amber-950/55",
  collection:
    "border-sky-200 bg-sky-50 text-sky-700 shadow-sky-950/5 hover:border-sky-300 hover:bg-sky-100/80 dark:border-sky-950 dark:bg-sky-950/35 dark:text-sky-300 dark:shadow-black/20 dark:hover:border-sky-900 dark:hover:bg-sky-950/55",
  note:
    "border-emerald-200 bg-emerald-50 text-emerald-700 shadow-emerald-950/5 hover:border-emerald-300 hover:bg-emerald-100/80 dark:border-emerald-950 dark:bg-emerald-950/35 dark:text-emerald-300 dark:shadow-black/20 dark:hover:border-emerald-900 dark:hover:bg-emerald-950/55",
  tag: "border-slate-300 bg-slate-100 text-slate-700 shadow-slate-950/5 hover:border-slate-400 hover:bg-slate-100 dark:border-slate-700 dark:bg-slate-900 dark:text-slate-100 dark:shadow-black/20 dark:hover:border-slate-600 dark:hover:bg-slate-800",
  danger:
    "border-red-200 bg-white text-red-600 shadow-red-950/5 hover:border-red-300 hover:bg-red-50 dark:border-red-950 dark:bg-slate-950 dark:text-red-300 dark:shadow-black/20 dark:hover:border-red-900 dark:hover:bg-red-950/35",
};

function IconActionButton({
  title,
  variant = "secondary",
  icon,
  tone,
  active = false,
  className,
  ...props
}: Omit<React.ComponentProps<typeof Button>, "children"> & {
  title: string;
  icon: LucideIcon;
  tone?: IconTone;
  active?: boolean;
}) {
  const resolvedTone = tone ?? (variant === "danger" ? "danger" : "neutral");

  return (
    <Button
      variant={variant === "danger" ? "secondary" : variant}
      title={title}
      aria-label={title}
      className={cn(
        "group h-8 w-8 border p-0 shadow-sm transition-all hover:-translate-y-px hover:shadow-md focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-slate-300 dark:focus-visible:ring-slate-700",
        iconToneClasses[resolvedTone],
        className,
      )}
      {...props}
    >
      <PremiumIcon
        icon={icon}
        className="h-4 w-4 transition-transform group-hover:scale-105"
        strokeWidth={active ? 2 : 1.75}
      />
    </Button>
  );
}

type IconSurfaceTone = "slate" | "sky";

const iconSurfaceToneClasses: Record<IconSurfaceTone, string> = {
  slate:
    "border-slate-200 bg-slate-100 text-slate-600 shadow-slate-950/5 dark:border-slate-800 dark:bg-slate-900 dark:text-slate-300 dark:shadow-black/20",
  sky: "border-sky-200 bg-sky-50 text-sky-700 shadow-sky-950/5 dark:border-sky-950 dark:bg-sky-950/35 dark:text-sky-300 dark:shadow-black/20",
};

function IconSurface({
  icon,
  tone = "slate",
  className,
  iconClassName,
}: {
  icon: LucideIcon;
  tone?: IconSurfaceTone;
  className?: string;
  iconClassName?: string;
}) {
  return (
    <div
      className={cn(
        "flex h-10 w-10 shrink-0 items-center justify-center rounded-lg border shadow-sm",
        iconSurfaceToneClasses[tone],
        className,
      )}
    >
      <PremiumIcon icon={icon} className={cn("h-5 w-5", iconClassName)} />
    </div>
  );
}

function FieldLimitMeter({
  count,
  max,
  error,
  helper,
  suffix = "",
}: {
  count: number;
  max: number;
  error: string | null;
  helper: string;
  suffix?: string;
}) {
  const percentage = Math.min(100, Math.round((count / max) * 100));
  const isNearLimit = count >= Math.floor(max * 0.8);
  const isOverLimit = count > max;

  return (
    <div className="mt-2 space-y-1.5">
      <div className="flex items-start justify-between gap-3 text-xs">
        <p
          className={cn(
            "min-w-0 flex-1 text-slate-500 dark:text-slate-400",
            error && "font-medium text-red-600 dark:text-red-300",
          )}
        >
          {error ?? helper}
        </p>
        <span
          className={cn(
            "shrink-0 rounded-full border px-2 py-0.5 font-mono text-[11px] leading-none tabular-nums",
            isOverLimit
              ? "border-red-200 bg-red-50 text-red-700 dark:border-red-900 dark:bg-red-950/40 dark:text-red-200"
              : isNearLimit
                ? "border-amber-200 bg-amber-50 text-amber-700 dark:border-amber-900 dark:bg-amber-950/35 dark:text-amber-200"
                : "border-slate-200 bg-slate-50 text-slate-500 dark:border-slate-800 dark:bg-slate-900/60 dark:text-slate-400",
          )}
        >
          {count}/{max}
          {suffix}
        </span>
      </div>
      <div className="h-1 overflow-hidden rounded-full bg-slate-100 dark:bg-slate-900">
        <div
          className={cn(
            "h-full rounded-full transition-all",
            isOverLimit
              ? "bg-red-500 dark:bg-red-400"
              : isNearLimit
                ? "bg-amber-400 dark:bg-amber-300"
                : "bg-sky-500 dark:bg-sky-400",
          )}
          style={{ width: `${percentage}%` }}
        />
      </div>
    </div>
  );
}

function Modal({
  title,
  children,
  onClose,
}: {
  title: string;
  children: React.ReactNode;
  onClose: () => void;
}) {
  useBodyScrollLock();

  return (
    <div className="fixed inset-0 z-50 flex items-center justify-center bg-slate-950/50 px-4">
      <Card className="w-full max-w-md p-5">
        <div className="mb-4 flex items-center justify-between gap-3">
          <h2 title={title} className="min-w-0 truncate text-lg font-semibold">
            {title}
          </h2>
          <Button variant="ghost" onClick={onClose}>
            Close
          </Button>
        </div>
        {children}
      </Card>
    </div>
  );
}

function SettingsModal({
  settings,
  providers,
  onClose,
  onSave,
}: {
  settings: AppSettings;
  providers: ProviderStatus[];
  onClose: () => void;
  onSave: (settings: AppSettings) => void;
}) {
  const [draft, setDraft] = useState<AppSettings>(settings);
  const [advancedOpen, setAdvancedOpen] = useState(false);
  const [shortcutsOpen, setShortcutsOpen] = useState(false);
  const settingsPanelRef = useRef<HTMLElement | null>(null);

  useBodyScrollLock();

  useEffect(() => {
    function focusableSettingsControls() {
      const panel = settingsPanelRef.current;

      if (!panel) {
        return [];
      }

      return Array.from(
        panel.querySelectorAll<HTMLElement>(
          "button:not([disabled]), input:not([disabled]), select:not([disabled]), textarea:not([disabled]), a[href], [tabindex]:not([tabindex='-1'])",
        ),
      ).filter((element) => element.getClientRects().length > 0);
    }

    function handleSettingsKeyDown(event: KeyboardEvent) {
      if (
        event.defaultPrevented ||
        event.metaKey ||
        event.ctrlKey ||
        event.altKey ||
        event.shiftKey ||
        (event.key !== "ArrowDown" && event.key !== "ArrowUp")
      ) {
        return;
      }

      const controls = focusableSettingsControls();

      if (controls.length === 0) {
        return;
      }

      const direction = event.key === "ArrowDown" ? 1 : -1;
      const activeElement = document.activeElement;
      const activeIndex =
        activeElement instanceof HTMLElement
          ? controls.indexOf(activeElement)
          : -1;
      const nextIndex =
        activeIndex >= 0
          ? Math.max(0, Math.min(activeIndex + direction, controls.length - 1))
          : direction > 0
            ? 0
            : controls.length - 1;

      event.preventDefault();
      controls[nextIndex]?.focus();
      controls[nextIndex]?.scrollIntoView({
        block: "nearest",
        inline: "nearest",
      });
    }

    document.addEventListener("keydown", handleSettingsKeyDown);

    return () => document.removeEventListener("keydown", handleSettingsKeyDown);
  }, [advancedOpen, shortcutsOpen]);

  return (
    <div className="fixed inset-0 z-50 bg-slate-950/35 dark:bg-black/50">
      <aside
        ref={settingsPanelRef}
        className="ml-auto flex h-full w-full max-w-[28rem] flex-col border-l border-slate-200 bg-white shadow-2xl shadow-slate-950/20 dark:border-slate-800 dark:bg-slate-950 dark:shadow-black/50"
      >
        <div className="flex shrink-0 items-center justify-between gap-3 border-b border-slate-200 px-5 py-4 dark:border-slate-800">
          <div className="min-w-0">
            <h2 className="text-base font-semibold">Settings</h2>
          </div>
          <Button
            variant="ghost"
            title="Close settings"
            aria-label="Close settings"
            onClick={onClose}
            className="h-9 w-9 shrink-0 p-0"
          >
            <PremiumIcon icon={X} className="h-4 w-4" />
          </Button>
        </div>

        <div className="min-h-0 flex-1 space-y-5 overflow-y-auto bg-slate-100/70 px-5 py-5 dark:bg-slate-900/35">
          <SettingsSection
            icon={draft.theme === "dark" ? Moon : Sun}
            title="Theme"
          >
            <div className="grid grid-cols-2 gap-2">
              <Button
                variant={draft.theme === "dark" ? "primary" : "secondary"}
                onClick={() => setDraft({ ...draft, theme: "dark" })}
                className="h-10 gap-2 rounded-lg"
              >
                <PremiumIcon icon={Moon} className="h-4 w-4" />
                Dark
              </Button>
              <Button
                variant={draft.theme === "light" ? "primary" : "secondary"}
                onClick={() => setDraft({ ...draft, theme: "light" })}
                className="h-10 gap-2 rounded-lg"
              >
                <PremiumIcon icon={Sun} className="h-4 w-4" />
                Light
              </Button>
            </div>
          </SettingsSection>

          <SettingsSection icon={SquareTerminal} title="Detected AI CLIs">
            <div className="space-y-2">
              {providers.map((provider) => (
                <div
                  key={provider.id}
                  className="rounded-lg border border-slate-200 bg-slate-50/80 px-3 py-2.5 dark:border-slate-800 dark:bg-white/[0.03]"
                >
                  <div className="flex items-center justify-between gap-3">
                    <div className="flex min-w-0 items-center gap-2">
                      <PremiumIcon
                        icon={provider.available ? BadgeCheck : BadgeAlert}
                        className={cn(
                          "h-4 w-4",
                          provider.available
                            ? "text-emerald-600 dark:text-emerald-400"
                            : "text-amber-600 dark:text-amber-400",
                        )}
                      />
                      <div className="min-w-0">
                        <p className="truncate text-sm font-medium">
                          {provider.displayName}
                        </p>
                        <p className="mt-0.5 truncate font-mono text-xs text-slate-500 dark:text-slate-400">
                          {provider.executable}
                        </p>
                      </div>
                    </div>
                    <Badge className="shrink-0">
                      {provider.available ? "Detected" : "Missing"}
                    </Badge>
                  </div>
                </div>
              ))}
            </div>
          </SettingsSection>

          <CollapsibleSettingsSection
            icon={SlidersHorizontal}
            open={advancedOpen}
            title="Advanced"
            onToggle={() => setAdvancedOpen((current) => !current)}
          >
            <div className="space-y-3">
              <div>
                <label className="text-sm font-medium">
                  Terminal executable
                </label>
                <Input
                  className="mt-2"
                  value={draft.terminalExecutable ?? ""}
                  onChange={(event) =>
                    setDraft({
                      ...draft,
                      terminalExecutable: event.target.value || null,
                    })
                  }
                  placeholder="Leave blank for OS default"
                />
                <p className="mt-2 text-xs text-slate-500 dark:text-slate-400">
                  macOS uses Terminal. Linux and Windows auto-detect common
                  terminals.
                </p>
              </div>
            </div>
          </CollapsibleSettingsSection>

          <SettingsSection icon={EyeOff} title="Hidden Sessions">
            <label className="flex items-center justify-between gap-3 rounded-lg border border-slate-200 bg-slate-50/80 px-3 py-2.5 dark:border-slate-800 dark:bg-white/[0.03]">
              <span className="min-w-0">
                <span className="block text-sm font-medium">
                  Show hidden sessions
                </span>
                <span className="mt-0.5 block text-xs text-slate-500 dark:text-slate-400">
                  Include hidden cards in the dashboard.
                </span>
              </span>
              <input
                type="checkbox"
                checked={draft.showHiddenSessions}
                onChange={(event) =>
                  setDraft({
                    ...draft,
                    showHiddenSessions: event.target.checked,
                  })
                }
                className="h-4 w-4 shrink-0"
              />
            </label>
          </SettingsSection>

          <CollapsibleSettingsSection
            icon={Keyboard}
            open={shortcutsOpen}
            title="Keyboard Shortcuts"
            onToggle={() => setShortcutsOpen((current) => !current)}
          >
            <div className="overflow-hidden rounded-lg border border-slate-200 bg-white dark:border-slate-800 dark:bg-slate-950/60">
              <ShortcutRow
                keys={[["Cmd/Ctrl", "Shift", "P"]]}
                label="Open command palette"
              />
              <ShortcutRow keys={[["Cmd/Ctrl", "K"]]} label="Focus search" />
              <ShortcutRow
                keys={["Left/Right", "Up/Down"]}
                label="Select session card"
              />
              <ShortcutRow
                keys={["Up/Down"]}
                label="Select history message"
              />
              <ShortcutRow
                keys={["Home/End"]}
                label="Select first or last session"
              />
              <ShortcutRow
                keys={["Enter"]}
                label="Open selected chat history"
              />
              <ShortcutRow
                keys={[["Cmd/Ctrl", "Enter"]]}
                label="Resume selected session"
              />
              <ShortcutRow keys={["F2"]} label="Rename selected session" />
              <ShortcutRow
                keys={["P"]}
                label="Pin or unpin selected session"
              />
              <ShortcutRow
                keys={["C"]}
                label="Add or change selected session collection"
              />
              <ShortcutRow
                keys={["N"]}
                label="Add or edit selected session note"
              />
              <ShortcutRow
                keys={["T"]}
                label="Add or edit selected session tags"
              />
              <ShortcutRow
                keys={["Delete"]}
                label="Delete or hide selected session"
              />
              <ShortcutRow
                keys={["U"]}
                label="Unhide selected hidden session"
              />
              <ShortcutRow
                keys={["F5", ["Cmd/Ctrl", "R"]]}
                label="Refresh sessions"
              />
              <ShortcutRow
                keys={[
                  ["Cmd/Ctrl", "1"],
                  ["Cmd/Ctrl", "2"],
                ]}
                label="Switch All or Pinned view"
              />
              <ShortcutRow keys={[["Cmd/Ctrl", ","]]} label="Open settings" />
              <ShortcutRow
                keys={["Esc"]}
                label="Close dialog or clear search"
              />
            </div>
          </CollapsibleSettingsSection>

        </div>

        <div className="flex shrink-0 justify-end gap-2 border-t border-slate-200 bg-slate-50/90 px-5 py-4 dark:border-slate-800 dark:bg-slate-950/95">
          <Button variant="ghost" onClick={onClose}>
            Cancel
          </Button>
          <Button onClick={() => onSave(draft)}>Save</Button>
        </div>
      </aside>
    </div>
  );
}

function SettingsSection({
  icon,
  title,
  tone = "default",
  children,
}: {
  icon: LucideIcon;
  title: string;
  tone?: "default" | "danger";
  children: React.ReactNode;
}) {
  return (
    <section
      className={cn(
        "overflow-hidden rounded-xl border bg-white shadow-sm shadow-slate-950/5 dark:bg-slate-950 dark:shadow-black/20",
        tone === "danger"
          ? "border-red-200 bg-red-50 dark:border-red-900/80 dark:bg-red-950/25"
          : "border-slate-300 dark:border-slate-700/80",
      )}
    >
      <div
        className={cn(
          "flex items-center gap-2 border-b px-3 py-3",
          tone === "danger"
            ? "border-red-200 bg-red-50/90 dark:border-red-900/80 dark:bg-red-950/35"
            : "border-slate-200 bg-slate-50/95 dark:border-slate-800 dark:bg-slate-900/70",
        )}
      >
        <div
          className={cn(
            "flex h-8 w-8 shrink-0 items-center justify-center rounded-lg border",
            tone === "danger"
              ? "border-red-200 bg-white text-red-600 dark:border-red-900 dark:bg-red-950 dark:text-red-300"
              : "border-slate-200 bg-slate-50 text-slate-500 dark:border-slate-800 dark:bg-slate-900 dark:text-slate-300",
          )}
        >
          <PremiumIcon icon={icon} className="h-4 w-4" />
        </div>
        <h3
          className={cn(
            "text-sm font-semibold",
            tone === "danger" && "text-red-800 dark:text-red-200",
          )}
        >
          {title}
        </h3>
      </div>
      <div className="p-3">{children}</div>
    </section>
  );
}

function CollapsibleSettingsSection({
  icon,
  open,
  title,
  onToggle,
  children,
}: {
  icon: LucideIcon;
  open: boolean;
  title: string;
  onToggle: () => void;
  children: React.ReactNode;
}) {
  return (
    <section className="overflow-hidden rounded-xl border border-slate-300 bg-white shadow-sm shadow-slate-950/5 dark:border-slate-700/80 dark:bg-slate-950 dark:shadow-black/20">
      <button
        type="button"
        aria-expanded={open}
        onClick={onToggle}
        className={cn(
          "flex w-full items-center justify-between gap-3 bg-slate-50/95 px-3 py-3 text-left transition-colors hover:bg-slate-100 focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-slate-300 dark:bg-slate-900/70 dark:hover:bg-slate-900 dark:focus-visible:ring-slate-700",
          open && "border-b border-slate-200 dark:border-slate-800",
        )}
      >
        <span className="flex min-w-0 items-center gap-2">
          <span className="flex h-8 w-8 shrink-0 items-center justify-center rounded-lg border border-slate-200 bg-slate-50 text-slate-500 dark:border-slate-800 dark:bg-slate-900 dark:text-slate-300">
            <PremiumIcon icon={icon} className="h-4 w-4" />
          </span>
          <span className="text-sm font-semibold">{title}</span>
        </span>
        <PremiumIcon
          icon={ChevronDown}
          className={cn(
            "h-4 w-4 text-slate-400 transition-transform dark:text-slate-500",
            open && "rotate-180",
          )}
        />
      </button>
      {open && (
        <div className="px-3 py-3">
          {children}
        </div>
      )}
    </section>
  );
}

function ShortcutRow({
  keys,
  label,
}: {
  keys: Array<string | string[]>;
  label: string;
}) {
  return (
    <div className="flex items-start justify-between gap-3 border-b border-slate-200 px-3 py-2.5 last:border-b-0 dark:border-slate-800">
      <span className="min-w-0 text-sm text-slate-700 dark:text-slate-300">
        {label}
      </span>
      <span className="flex shrink-0 flex-wrap items-center justify-end gap-1.5">
        {keys.map((shortcut, shortcutIndex) => {
          const chord = Array.isArray(shortcut) ? shortcut : [shortcut];
          const key = chord.join("+");

          return (
            <span key={key} className="flex items-center gap-1.5">
              {shortcutIndex > 0 && (
                <span className="text-[0.6875rem] font-medium text-slate-400 dark:text-slate-500">
                  or
                </span>
              )}
              {chord.map((keyPart, keyPartIndex) => (
                <span
                  key={`${key}-${keyPartIndex}`}
                  className="flex items-center gap-1.5"
                >
                  {keyPartIndex > 0 && (
                    <span className="font-mono text-xs font-semibold text-slate-400 dark:text-slate-500">
                      +
                    </span>
                  )}
                  <kbd className="min-w-7 whitespace-nowrap rounded border border-slate-200 bg-slate-50 px-1.5 py-0.5 text-center font-mono text-xs font-semibold text-slate-600 dark:border-slate-800 dark:bg-slate-900 dark:text-slate-300">
                    {keyPart}
                  </kbd>
                </span>
              ))}
            </span>
          );
        })}
      </span>
    </div>
  );
}

function SessionHistoryModal({
  session,
  history,
  loading,
  error,
  onClose,
}: {
  session: SessionRecord;
  history: SessionHistory | null;
  loading: boolean;
  error: string | null;
  onClose: () => void;
}) {
  const messageCount = history?.messages.length ?? 0;
  const messageRefs = useRef(new Map<number, HTMLDivElement>());
  const [selectedMessageIndex, setSelectedMessageIndex] = useState(0);

  useBodyScrollLock();

  useEffect(() => {
    messageRefs.current.clear();
    setSelectedMessageIndex(0);
  }, [history?.sessionId, messageCount]);

  function focusHistoryMessage(index: number) {
    window.requestAnimationFrame(() => {
      const element = messageRefs.current.get(index);

      element?.scrollIntoView({ block: "nearest", inline: "nearest" });
      element?.focus({ preventScroll: true });
    });
  }

  function selectHistoryMessage(index: number) {
    if (messageCount === 0) {
      return;
    }

    const nextIndex = Math.max(0, Math.min(index, messageCount - 1));

    setSelectedMessageIndex(nextIndex);
    focusHistoryMessage(nextIndex);
  }

  useEffect(() => {
    function handleKeyDown(event: KeyboardEvent) {
      if (
        messageCount === 0 ||
        event.metaKey ||
        event.ctrlKey ||
        event.altKey ||
        event.shiftKey ||
        isEditableKeyboardTarget(event.target)
      ) {
        return;
      }

      if (event.key === "ArrowDown") {
        event.preventDefault();
        selectHistoryMessage(selectedMessageIndex + 1);
        return;
      }

      if (event.key === "ArrowUp") {
        event.preventDefault();
        selectHistoryMessage(selectedMessageIndex - 1);
      }
    }

    document.addEventListener("keydown", handleKeyDown);

    return () => document.removeEventListener("keydown", handleKeyDown);
  }, [messageCount, selectedMessageIndex]);

  return (
    <div className="fixed inset-0 z-50 flex items-center justify-center bg-slate-950/60 px-4 py-6">
      <Card className="flex max-h-[calc(100vh-3rem)] w-full max-w-5xl flex-col overflow-hidden p-0">
        <div className="flex items-start justify-between gap-4 border-b border-slate-200 px-5 py-4 dark:border-slate-800">
          <div className="flex min-w-0 items-start gap-3">
            <IconSurface icon={MessagesSquare} tone="sky" />
            <div className="min-w-0">
              <div className="flex flex-wrap items-center gap-2">
                <h2
                  title={session.displayName}
                  className="min-w-0 truncate text-lg font-semibold tracking-tight"
                >
                  {session.displayName}
                </h2>
                <Badge>{session.providerDisplayName}</Badge>
                {messageCount > 0 && (
                  <Badge>
                    {messageCount} message{messageCount === 1 ? "" : "s"}
                  </Badge>
                )}
              </div>
              <p className="mt-1 break-all font-mono text-xs text-slate-500 dark:text-slate-400">
                {session.sessionId}
              </p>
            </div>
          </div>

          <Button
            variant="ghost"
            title="Close"
            aria-label="Close chat history"
            onClick={onClose}
            className="h-9 w-9 shrink-0 p-0"
          >
            <PremiumIcon icon={X} className="h-4 w-4" />
          </Button>
        </div>

        <div className="min-h-0 flex-1 overflow-y-auto overscroll-contain bg-slate-50/80 p-4 dark:bg-slate-950/40 sm:p-6">
          {loading ? (
            <div className="flex min-h-72 items-center justify-center gap-2 text-sm text-slate-500 dark:text-slate-400">
              <PremiumIcon icon={Loader2} className="h-4 w-4 animate-spin" />
              Loading chat history...
            </div>
          ) : error ? (
            <HistoryError error={error} />
          ) : (
            <SessionHistoryContent
              history={history}
              selectedMessageIndex={selectedMessageIndex}
              onMessageFocus={setSelectedMessageIndex}
              onMessageRef={(index, node) => {
                if (node) {
                  messageRefs.current.set(index, node);
                } else {
                  messageRefs.current.delete(index);
                }
              }}
            />
          )}
        </div>
      </Card>
    </div>
  );
}

function HistoryError({ error }: { error: string }) {
  return (
    <div className="rounded-xl border border-red-200 bg-red-50 p-4 text-sm text-red-700 dark:border-red-900 dark:bg-red-950/40 dark:text-red-200">
      <div className="flex items-start gap-3">
        <PremiumIcon
          icon={AlertCircle}
          className="mt-0.5 h-4 w-4 text-red-600 dark:text-red-300"
        />
        <div className="min-w-0">
          <h3 className="font-medium">Could not load chat history</h3>
          <p className="mt-1 whitespace-pre-wrap break-words">{error}</p>
        </div>
      </div>
    </div>
  );
}

function SessionHistoryContent({
  history,
  selectedMessageIndex,
  onMessageFocus,
  onMessageRef,
}: {
  history: SessionHistory | null;
  selectedMessageIndex: number;
  onMessageFocus: (index: number) => void;
  onMessageRef: (index: number, node: HTMLDivElement | null) => void;
}) {
  if (!history || history.messages.length === 0) {
    return (
      <div className="flex min-h-72 items-center justify-center rounded-xl border border-dashed border-slate-300 bg-white p-6 text-center dark:border-slate-800 dark:bg-slate-950">
        <div className="max-w-sm">
          <IconSurface
            icon={BookOpenText}
            className="mx-auto h-12 w-12"
            iconClassName="h-6 w-6"
          />
          <h3 className="mt-3 text-sm font-medium">
            No readable chat text found
          </h3>
          <p className="mt-2 text-sm text-slate-500 dark:text-slate-400">
            SessionDex could not find user or assistant text in this session.
            {history?.unreadableLines
              ? ` ${history.unreadableLines} line${
                  history.unreadableLines === 1 ? "" : "s"
                } could not be parsed.`
              : ""}
          </p>
        </div>
      </div>
    );
  }

  return (
    <div className="space-y-4">
      {history.unreadableLines > 0 && (
        <div className="rounded-lg border border-amber-200 bg-amber-50 px-3 py-2 text-xs text-amber-800 dark:border-amber-950 dark:bg-amber-950/35 dark:text-amber-200">
          {history.unreadableLines} line
          {history.unreadableLines === 1 ? "" : "s"} could not be parsed and
          were skipped.
        </div>
      )}

      <div role="list" className="space-y-4">
        {history.messages.map((message, index) => (
          <ChatMessage
            key={`${message.role}-${index}`}
            message={message}
            selected={index === selectedMessageIndex}
            onFocus={() => onMessageFocus(index)}
            messageRef={(node) => onMessageRef(index, node)}
          />
        ))}
      </div>
    </div>
  );
}

function ChatMessage({
  message,
  selected,
  onFocus,
  messageRef,
}: {
  message: SessionMessage;
  selected: boolean;
  onFocus: () => void;
  messageRef: (node: HTMLDivElement | null) => void;
}) {
  const role = message.role.toLowerCase();
  const isUser = role === "user";
  const isAssistant = role === "assistant";

  return (
    <div
      ref={messageRef}
      role="listitem"
      tabIndex={selected ? 0 : -1}
      aria-current={selected ? "true" : undefined}
      onClick={onFocus}
      onFocus={onFocus}
      className={cn(
        "flex rounded-xl outline-none focus-visible:ring-2 focus-visible:ring-violet-300 focus-visible:ring-offset-2 focus-visible:ring-offset-slate-50 dark:focus-visible:ring-violet-500 dark:focus-visible:ring-offset-slate-950",
        isUser ? "justify-end" : "justify-start",
      )}
    >
      <article
        className={cn(
          "w-full max-w-[92%] rounded-xl border p-4 shadow-sm transition-all sm:max-w-[84%]",
          isUser &&
            "border-sky-200 bg-sky-50 dark:border-sky-950 dark:bg-sky-950/35",
          isAssistant &&
            "border-emerald-200 bg-white dark:border-emerald-950 dark:bg-slate-950",
          !isUser &&
            !isAssistant &&
            "border-slate-200 bg-white dark:border-slate-800 dark:bg-slate-950",
          selected &&
            "ring-2 ring-violet-300/80 dark:ring-violet-500/70",
        )}
      >
        <div
          className={cn(
            "mb-3 text-[10px] font-semibold uppercase tracking-wide",
            isUser && "text-sky-700 dark:text-sky-300",
            isAssistant && "text-emerald-700 dark:text-emerald-300",
            !isUser && !isAssistant && "text-slate-500 dark:text-slate-400",
          )}
        >
          {formatMessageRole(message.role)}
        </div>
        <MessageText value={message.text} />
      </article>
    </div>
  );
}

function formatMessageRole(role: string) {
  const normalized = role.trim().toLowerCase();

  if (normalized === "user") {
    return "You";
  }

  if (normalized === "assistant") {
    return "Assistant";
  }

  return normalized || "Message";
}

type MessagePart =
  | { type: "text"; value: string }
  | { type: "code"; value: string; language: string | null };

function MessageText({ value }: { value: string }) {
  const parts = splitFencedCodeBlocks(value);

  if (parts.length === 0) {
    return (
      <p className="text-sm italic text-slate-500 dark:text-slate-400">
        Empty message
      </p>
    );
  }

  return (
    <div className="space-y-3 text-sm leading-6 text-slate-700 dark:text-slate-200">
      {parts.map((part, index) =>
        part.type === "code" ? (
          <CodeBlock
            key={index}
            value={part.value}
            language={part.language}
          />
        ) : (
          <p key={index} className="whitespace-pre-wrap break-words">
            <InlineCodeText value={part.value} />
          </p>
        ),
      )}
    </div>
  );
}

function CodeBlock({
  value,
  language,
}: {
  value: string;
  language: string | null;
}) {
  return (
    <div className="overflow-hidden rounded-lg border border-slate-800 bg-slate-950 shadow-sm">
      <div className="border-b border-slate-800 px-3 py-1.5 font-mono text-[10px] uppercase tracking-wide text-slate-400">
        {language || "code"}
      </div>
      <pre className="max-h-80 overflow-auto p-3 font-mono text-xs leading-5 text-slate-100">
        <code>{value || " "}</code>
      </pre>
    </div>
  );
}

function InlineCodeText({ value }: { value: string }) {
  return (
    <>
      {splitInlineCode(value).map((part, index) =>
        part.code ? (
          <code
            key={index}
            className="rounded bg-slate-200 px-1 py-0.5 font-mono text-[0.92em] text-slate-800 dark:bg-slate-800 dark:text-slate-100"
          >
            {part.value}
          </code>
        ) : (
          <span key={index}>{part.value}</span>
        ),
      )}
    </>
  );
}

function splitFencedCodeBlocks(value: string): MessagePart[] {
  const normalized = value.replace(/\r\n/g, "\n").replace(/\r/g, "\n");
  const lines = normalized.split("\n");
  const parts: MessagePart[] = [];
  let textLines: string[] = [];
  let codeLines: string[] = [];
  let language: string | null = null;

  function pushText() {
    const text = textLines.join("\n").trim();

    if (text) {
      parts.push({ type: "text", value: text });
    }

    textLines = [];
  }

  function pushCode() {
    parts.push({
      type: "code",
      value: codeLines.join("\n"),
      language,
    });
    codeLines = [];
    language = null;
  }

  for (const line of lines) {
    const trimmedStart = line.trimStart();

    if (trimmedStart.startsWith("```")) {
      if (language !== null) {
        pushCode();
      } else {
        pushText();
        language = trimmedStart.slice(3).trim() || "text";
      }
      continue;
    }

    if (language !== null) {
      codeLines.push(line);
    } else {
      textLines.push(line);
    }
  }

  if (language !== null) {
    pushCode();
  }

  pushText();

  return parts;
}

function splitInlineCode(value: string) {
  const parts: Array<{ code: boolean; value: string }> = [];
  const pattern = /`([^`\n]+)`/g;
  let lastIndex = 0;
  let match: RegExpExecArray | null;

  while ((match = pattern.exec(value))) {
    if (match.index > lastIndex) {
      parts.push({ code: false, value: value.slice(lastIndex, match.index) });
    }

    parts.push({ code: true, value: match[1] });
    lastIndex = match.index + match[0].length;
  }

  if (lastIndex < value.length) {
    parts.push({ code: false, value: value.slice(lastIndex) });
  }

  return parts.length > 0 ? parts : [{ code: false, value }];
}

function SearchMatchPreview({ snippet }: { snippet: string }) {
  return (
    <div className="rounded-lg border border-amber-200 bg-amber-50/90 px-3 py-2 dark:border-amber-950 dark:bg-amber-950/35">
      <div className="mb-1 flex items-center gap-1.5 text-[10px] font-medium uppercase text-amber-700 dark:text-amber-300">
        <PremiumIcon icon={TextSearch} className="h-3 w-3" />
        Chat match
      </div>
      <p className="line-clamp-2 whitespace-pre-wrap break-words font-mono text-[11px] leading-4 text-slate-700 dark:text-slate-200">
        {snippet}
      </p>
    </div>
  );
}

function SessionPreview({
  firstUserInput,
  lastUserInput,
  lastMessagePreview,
  onOpenHistory,
}: {
  firstUserInput: string | null;
  lastUserInput: string | null;
  lastMessagePreview: string | null;
  onOpenHistory: () => void;
}) {
  const latestPreview = lastMessagePreview ?? lastUserInput;
  const hasDistinctLatest =
    Boolean(firstUserInput && latestPreview) && firstUserInput !== latestPreview;
  const hasPreview = Boolean(firstUserInput || latestPreview);

  return (
    <button
      type="button"
      aria-label="Open full chat history"
      title="Open full chat history"
      onClick={onOpenHistory}
      className="group block w-full overflow-hidden rounded-lg border border-indigo-200 bg-indigo-50/80 p-2 text-left shadow-sm shadow-indigo-950/[0.06] transition-all hover:-translate-y-px hover:border-indigo-300 hover:bg-indigo-50 hover:shadow-md focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-indigo-300 dark:border-indigo-950/80 dark:bg-indigo-950/25 dark:shadow-black/20 dark:hover:border-indigo-800 dark:hover:bg-indigo-950/35 dark:focus-visible:ring-indigo-700"
    >
      {hasPreview ? (
        <div className="space-y-1.5">
          {firstUserInput ? (
            <ChatPreviewLine value={firstUserInput} />
          ) : (
            latestPreview && (
              <ChatPreviewLine value={latestPreview} />
            )
          )}

          {hasDistinctLatest && (
            <>
              <div className="flex items-center gap-2 px-1" aria-hidden="true">
                <div className="h-px flex-1 bg-indigo-200/80 dark:bg-indigo-900/70" />
                <span className="font-mono text-[10px] leading-none text-indigo-300 dark:text-indigo-700">
                  ...
                </span>
                <div className="h-px flex-1 bg-indigo-200/80 dark:bg-indigo-900/70" />
              </div>
              <ChatPreviewLine value={latestPreview ?? ""} />
            </>
          )}
        </div>
      ) : (
        <div className="flex min-h-[5.25rem] items-center px-2 py-3 text-xs font-medium text-indigo-700/70 dark:text-indigo-200/70">
          No readable chat preview found.
        </div>
      )}
    </button>
  );
}

function ChatPreviewLine({
  value,
}: {
  value: string;
}) {
  return (
    <div className="rounded-md border border-indigo-200/80 bg-white/70 px-2.5 py-1.5 dark:border-indigo-900/70 dark:bg-slate-950/45">
      <p className="line-clamp-2 whitespace-pre-wrap break-words font-mono text-[11px] leading-5 text-slate-800 dark:text-indigo-50">
        {value}
      </p>
    </div>
  );
}

export default App;
