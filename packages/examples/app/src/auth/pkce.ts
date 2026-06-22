const pkceAlphabet = "ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789._~-";
const urlSafeAlphabet = "ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789-_";

export interface PkcePair {
  verifier: string;
  challenge: string;
}

export function base64UrlEncode(bytes: Uint8Array): string {
  let binary = "";
  for (const byte of bytes) {
    binary += String.fromCharCode(byte);
  }

  return btoa(binary).replace(/\+/g, "-").replace(/\//g, "_").replace(/=/g, "");
}

export function randomUrlSafeString(length: number): string {
  return randomString(length, urlSafeAlphabet);
}

export async function createPkcePair(): Promise<PkcePair> {
  const verifier = randomString(64, pkceAlphabet);
  const digest = await crypto.subtle.digest("SHA-256", new TextEncoder().encode(verifier));

  return {
    verifier,
    challenge: base64UrlEncode(new Uint8Array(digest)),
  };
}

function randomString(length: number, alphabet: string): string {
  const bytes = new Uint8Array(length);
  crypto.getRandomValues(bytes);

  let result = "";
  for (const byte of bytes) {
    result += alphabet[byte % alphabet.length];
  }
  return result;
}
