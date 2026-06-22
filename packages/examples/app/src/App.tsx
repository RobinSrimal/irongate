import { FormEvent, useEffect, useState } from "react";
import "./App.css";
import { displayEmail, displaySubject, type AppSession, type Provider } from "./auth/session";
import {
  loginWithPassword,
  loginWithProvider,
  logoutSession,
  refreshSession,
  storedSessionStatus,
} from "./auth/tokenStore";

function App() {
  const [session, setSession] = useState<AppSession | null>(null);
  const [hasStoredSession, setHasStoredSession] = useState(false);
  const [email, setEmail] = useState("");
  const [password, setPassword] = useState("");
  const [status, setStatus] = useState("Signed out");
  const [busy, setBusy] = useState(false);

  useEffect(() => {
    storedSessionStatus()
      .then((stored) => setHasStoredSession(stored.has_refresh_token))
      .catch(() => setHasStoredSession(false));
  }, []);

  async function runAuthAction(action: () => Promise<AppSession>, label: string) {
    setBusy(true);
    setStatus(label);
    try {
      const nextSession = await action();
      setSession(nextSession);
      setHasStoredSession(true);
      setStatus("Signed in");
    } catch (error) {
      const message = error instanceof Error ? error.message : String(error);
      setStatus(message);
    } finally {
      setBusy(false);
    }
  }

  async function handleProvider(provider: Provider) {
    await runAuthAction(() => loginWithProvider(provider), `Opening ${provider} sign in`);
  }

  async function handlePasswordLogin(event: FormEvent<HTMLFormElement>) {
    event.preventDefault();
    await runAuthAction(() => loginWithPassword(email, password), "Signing in");
  }

  async function handleRefresh() {
    await runAuthAction(refreshSession, "Refreshing session");
  }

  async function handleLogout() {
    setBusy(true);
    setStatus("Signing out");
    try {
      await logoutSession();
      setSession(null);
      setHasStoredSession(false);
      setStatus("Signed out");
    } catch (error) {
      const message = error instanceof Error ? error.message : String(error);
      setStatus(message);
    } finally {
      setBusy(false);
    }
  }

  return (
    <main className="shell">
      <section className="panel">
        <header className="header">
          <div>
            <p className="eyebrow">Irongate native example</p>
            <h1>Desktop sign in</h1>
          </div>
          <span className={session ? "badge active" : "badge"}>{session ? "Active" : "Signed out"}</span>
        </header>

        {session ? (
          <section className="session">
            <dl>
              <div>
                <dt>Subject</dt>
                <dd>{displaySubject(session)}</dd>
              </div>
              <div>
                <dt>Email</dt>
                <dd>{displayEmail(session)}</dd>
              </div>
              <div>
                <dt>Scope</dt>
                <dd>{session.scope ?? "not returned"}</dd>
              </div>
            </dl>

            <div className="actions">
              <button type="button" onClick={handleRefresh} disabled={busy}>
                Refresh
              </button>
              <button type="button" className="secondary" onClick={handleLogout} disabled={busy}>
                Logout
              </button>
            </div>
          </section>
        ) : (
          <section className="login">
            <div className="provider-grid">
              <button type="button" onClick={() => handleProvider("google")} disabled={busy}>
                Continue with Google
              </button>
              <button type="button" onClick={() => handleProvider("apple")} disabled={busy}>
                Continue with Apple
              </button>
            </div>

            <form className="password-form" onSubmit={handlePasswordLogin}>
              <label>
                Email
                <input
                  type="email"
                  value={email}
                  autoComplete="email"
                  onChange={(event) => setEmail(event.currentTarget.value)}
                  required
                />
              </label>
              <label>
                Password
                <input
                  type="password"
                  value={password}
                  autoComplete="current-password"
                  onChange={(event) => setPassword(event.currentTarget.value)}
                  required
                />
              </label>
              <button type="submit" disabled={busy}>
                Sign in with password
              </button>
            </form>

            {hasStoredSession ? (
              <button type="button" className="secondary" onClick={handleRefresh} disabled={busy}>
                Restore stored session
              </button>
            ) : null}
          </section>
        )}

        <p className="status">{status}</p>
      </section>
    </main>
  );
}

export default App;
