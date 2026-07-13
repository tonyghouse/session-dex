import { invoke } from "@tauri-apps/api/core";
import type {
  AppSettings,
  CollectionColorName,
  DeleteResult,
  ProviderStatus,
  SessionHistory,
  SessionRecord,
  SessionSearchResult,
  UninstallResult,
} from "./types";

export const api = {
  listSessions: () => invoke<SessionRecord[]>("list_sessions"),
  searchSessions: (query: string, providerFilter: string | null) =>
    invoke<SessionSearchResult[]>("search_sessions", {
      query,
      providerFilter,
    }),
  getSessionHistory: (provider: string, sessionId: string) =>
    invoke<SessionHistory>("get_session_history", {
      provider,
      sessionId,
    }),
  listProviders: () => invoke<ProviderStatus[]>("list_providers"),
  listRepositoryBranches: (repositoryPath: string) =>
    invoke<string[]>("list_repository_branches", {
      repositoryPath,
    }),
  getSettings: () => invoke<AppSettings>("get_settings"),
  saveSettings: (settings: AppSettings) =>
    invoke<void>("save_settings", { settings }),
  renameSession: (
    provider: string,
    sessionId: string,
    friendlyName: string,
  ) =>
    invoke<void>("rename_session", {
      provider,
      sessionId,
      friendlyName,
    }),
  setSessionPinned: (
    provider: string,
    sessionId: string,
    isPinned: boolean,
  ) =>
    invoke<void>("set_session_pinned", {
      provider,
      sessionId,
      isPinned,
    }),
  setSessionDiscoveredBranch: (
    provider: string,
    sessionId: string,
    branchName: string,
  ) =>
    invoke<void>("set_session_discovered_branch", {
      provider,
      sessionId,
      branchName,
    }),
  setProjectFavorite: (workingDirectory: string, isFavorite: boolean) =>
    invoke<void>("set_project_favorite", {
      workingDirectory,
      isFavorite,
    }),
  setSessionCollection: (
    provider: string,
    sessionId: string,
    collectionName: string,
  ) =>
    invoke<void>("set_session_collection", {
      provider,
      sessionId,
      collectionName,
    }),
  setCollectionColor: (
    collectionName: string,
    colorName: CollectionColorName | "none",
  ) =>
    invoke<void>("set_collection_color", {
      collectionName,
      colorName,
    }),
  setSessionNote: (provider: string, sessionId: string, noteText: string) =>
    invoke<void>("set_session_note", {
      provider,
      sessionId,
      noteText,
    }),
  setSessionTags: (provider: string, sessionId: string, tags: string[]) =>
    invoke<void>("set_session_tags", {
      provider,
      sessionId,
      tags,
    }),
  deleteOrHideSession: (provider: string, sessionId: string) =>
    invoke<DeleteResult>("delete_or_hide_session", { provider, sessionId }),
  unhideSession: (provider: string, sessionId: string) =>
    invoke<void>("unhide_session", { provider, sessionId }),
  resetLocalData: () => invoke<void>("reset_local_data"),
  uninstallApp: () => invoke<UninstallResult>("uninstall_app"),
  resumeSession: (
    provider: string,
    sessionId: string,
    workingDirectory: string | null,
  ) => invoke<void>("resume_session", { provider, sessionId, workingDirectory }),
  openWorkingDirectory: (path: string) =>
    invoke<void>("open_working_directory", { path }),
};
