import { invoke } from "@tauri-apps/api/core";
import type { AppSession, Provider, StoredSessionStatus } from "./session.js";

export async function loginWithProvider(provider: Provider): Promise<AppSession> {
  return await invoke<AppSession>("login_with_provider", { provider });
}

export async function loginWithPassword(email: string, password: string): Promise<AppSession> {
  return await invoke<AppSession>("login_with_password", { email, password });
}

export async function refreshSession(): Promise<AppSession> {
  return await invoke<AppSession>("refresh_session");
}

export async function logoutSession(): Promise<void> {
  await invoke("logout");
}

export async function storedSessionStatus(): Promise<StoredSessionStatus> {
  return await invoke<StoredSessionStatus>("stored_session_status");
}
