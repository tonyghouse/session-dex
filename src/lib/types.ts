export type Theme = "light" | "dark";
export type CollectionColorName =
  | "gray"
  | "red"
  | "orange"
  | "yellow"
  | "green"
  | "blue"
  | "purple";

export interface SessionRecord {
  provider: string;
  providerDisplayName: string;
  sessionId: string;
  title: string | null;
  friendlyName: string | null;
  collection: string | null;
  collectionColor: CollectionColorName | null;
  note: string | null;
  tags: string[];
  displayName: string;
  firstUserInput: string | null;
  lastUserInput: string | null;
  lastMessagePreview: string | null;
  lastMessageRole: string | null;
  workingDirectory: string | null;
  discoveredRepository: string | null;
  discoveredBranch: string | null;
  discoveredAt: number | null;
  resumeCommand: string;
  lastModified: number | null;
  lastResumed: number | null;
  canDelete: boolean;
  canResume: boolean;
  isHidden: boolean;
  isPinned: boolean;
  isFavoriteProject: boolean;
}

export interface SessionSearchResult {
  provider: string;
  sessionId: string;
  snippet: string;
}

export interface SessionMessage {
  role: string;
  text: string;
}

export interface SessionHistory {
  provider: string;
  sessionId: string;
  messages: SessionMessage[];
  unreadableLines: number;
}

export interface ProviderStatus {
  id: string;
  displayName: string;
  executable: string;
  sessionStore: string;
  available: boolean;
  sessionsPathExists: boolean;
  deleteSupported: boolean;
  capabilities: string[];
}

export interface AppSettings {
  theme: Theme;
  terminalExecutable: string | null;
  providerFilter: string;
  showHiddenSessions: boolean;
}

export interface DeleteResult {
  action: "deleted" | "hidden";
  message: string;
}

export interface UninstallResult {
  message: string;
  appRemovalAttempted: boolean;
  appRemoved: boolean;
}
