// Sign-in and save sync for the web version, backed by Convex (see convex/).
//
// Sign-in is Convex Auth with Google, done by hand (the page has no React): auth:signIn returns
// Google's URL and a verifier; Google sends the browser back with ?code=, which auth:signIn
// trades for an access token (JWT, ~1 h) and a refresh token (kept in localStorage).
// Only allowlisted accounts get a session (convex/auth.ts).
//
// Sync: the game keeps its files in localStorage ("pokegame:<path>" = "<ms>\n<latin-1 bytes>",
// see src/store.rs). Before the game starts, `pull` merges the account's saves into
// localStorage, newest version of each file wins. While it runs, the game calls
// window.pokegameFileChanged for every file it writes, and save slots are pushed.
//
// `convex` is the global from convex.js (Convex's browser bundle).

const AUTH = 'pokegame-auth:';
const META = 'pokegame-cloud:';
const FILE = 'pokegame:';
const SYNCED = /^\/data\/saves\/[A-Za-z0-9_-]{1,64}\.pgs$/;

const api = convex.anyApi;
const ls = {
  get: (k) => { try { return localStorage.getItem(k); } catch { return null; } },
  set: (k, v) => { try { localStorage.setItem(k, v); } catch (e) { console.error('pokegame:', e); } },
  del: (k) => { try { localStorage.removeItem(k); } catch {} },
};

export function deviceId() {
  let id = ls.get(META + 'device');
  if (!id) {
    id = crypto.randomUUID();
    ls.set(META + 'device', id);
  }
  return id;
}

// ------------------------------------------------------------------ auth

export class Cloud {
  constructor(url) {
    this.url = url;
    this.http = new convex.ConvexHttpClient(url); // unauthenticated calls (auth:signIn)
    this.client = new convex.ConvexClient(url);
    this.device = deviceId();
  }

  get token() { return ls.get(AUTH + 'jwt'); }
  get hasSession() { return !!ls.get(AUTH + 'refresh'); }

  setTokens(tokens) {
    if (tokens) {
      ls.set(AUTH + 'jwt', tokens.token);
      ls.set(AUTH + 'refresh', tokens.refreshToken);
    } else {
      ls.del(AUTH + 'jwt');
      ls.del(AUTH + 'refresh');
    }
  }

  /** Go to Google (the page is left). */
  async signIn() {
    const back = location.origin + location.pathname;
    const res = await this.http.action(api.auth.signIn, { provider: 'google', params: { redirectTo: back } });
    ls.set(AUTH + 'verifier', res.verifier);
    location.href = res.redirect;
    return new Promise(() => {});
  }

  /** Back from Google with ?code=: trade it for tokens. Returns false if there was no code. */
  async finishSignIn() {
    const url = new URL(location.href);
    const code = url.searchParams.get('code');
    if (!code) return false;
    url.searchParams.delete('code');
    history.replaceState(null, '', url.pathname + url.search + url.hash);
    const verifier = ls.get(AUTH + 'verifier') ?? undefined;
    ls.del(AUTH + 'verifier');
    const res = await this.http.action(api.auth.signIn, { params: { code }, verifier });
    if (!res.tokens) throw new Error('Google sign-in did not complete.');
    this.setTokens(res.tokens);
    return true;
  }

  /** A new access token from the refresh token (one tab at a time: refresh tokens rotate). */
  async refresh() {
    const before = this.token;
    const run = async () => {
      if (this.token !== before) return this.token; // another tab refreshed meanwhile
      const refreshToken = ls.get(AUTH + 'refresh');
      if (!refreshToken) return null;
      try {
        const res = await this.http.action(api.auth.signIn, { refreshToken });
        this.setTokens(res.tokens ?? null);
      } catch (e) {
        // Offline: keep the session for later. Anything else: the session is gone.
        if (navigator.onLine && !/fetch|network/i.test(String(e))) this.setTokens(null);
        throw e;
      }
      return this.token;
    };
    return navigator.locks ? navigator.locks.request(AUTH + 'refresh', run) : run();
  }

  /** Authenticate the live client. Resolves to the signed-in account, or null (not signed in /
   *  not allowed). Throws when the server cannot be reached. */
  async connect() {
    if (!this.hasSession) return null;
    if (!this.token) await this.refresh();
    const authed = await Promise.race([
      new Promise((resolve) => {
        this.client.setAuth(
          // The server asks for a fresh token when the current one has expired.
          async ({ forceRefreshToken }) => (forceRefreshToken ? this.refresh().catch(() => null) : this.token),
          resolve,
        );
      }),
      new Promise((_, reject) => setTimeout(() => reject(new Error('The sync server did not answer (offline?).')), 10000)),
    ]);
    // A refused session was cleared by `refresh`; one still here means the server was unreachable.
    if (!authed && this.hasSession) throw new Error('Could not reach the sync server.');
    if (!authed) return null;
    const me = await this.client.query(api.saves.me, {});
    if (!me) this.setTokens(null);
    this.me = me;
    return me;
  }

  async signOut() {
    try { await this.client.action(api.auth.signOut, {}); } catch {}
    this.setTokens(null);
  }

  // ---------------------------------------------------------------- game files

  /** The game files kept in the account, as [exe, dat] bytes, or null. */
  async downloadGameFiles(progress) {
    const list = await this.client.query(api.gameFiles.get, {});
    const exe = list.find((f) => f.name === 'exe');
    const dat = list.find((f) => f.name === 'dat');
    if (!exe || !dat) return null;
    progress?.('Downloading the game files from your account…');
    const get = async (f) => new Uint8Array(await (await fetch(f.url)).arrayBuffer());
    return Promise.all([get(exe), get(dat)]);
  }

  async uploadGameFiles(exe, dat) {
    const have = await this.client.query(api.gameFiles.get, {});
    for (const [name, bytes] of [['exe', exe], ['dat', dat]]) {
      if (have.some((f) => f.name === name && f.size === bytes.length)) continue;
      const url = await this.client.mutation(api.gameFiles.uploadUrl, {});
      const res = await fetch(url, { method: 'POST', headers: { 'Content-Type': 'application/octet-stream' }, body: bytes });
      const { storageId } = await res.json();
      await this.client.mutation(api.gameFiles.register, { name, storageId });
    }
  }

  // ---------------------------------------------------------------- saves

  /** Merge the account's saves into localStorage (before the game starts). */
  async pull() {
    const owner = ls.get(META + 'owner');
    const email = this.me.email;
    // Another account's saves are in this browser: they are in that account already.
    if (owner && owner !== email) {
      for (const f of localFiles()) removeLocal(f.path);
      ls.del(META + 'seen');
    }
    ls.set(META + 'owner', email);
    const takeRemote = new Set(JSON.parse(sessionStorage.getItem(META + 'take-remote') || '[]'));
    sessionStorage.removeItem(META + 'take-remote');

    const remote = (await this.client.query(api.saves.pull, {})).filter((r) => SYNCED.test(r.path));
    const local = new Map(localFiles().map((f) => [f.path, f]));
    const seen = this.seen();
    for (const r of remote) {
      const l = local.get(r.path);
      local.delete(r.path);
      seen[r.path] = r.modified;
      if (takeRemote.has(r.path) || !l || r.modified > l.modified) {
        if (r.data) writeLocal(r.path, r.modified, new Uint8Array(r.data));
        else removeLocal(r.path);
      } else if (l.modified > r.modified) {
        this.push(r.path, l.modified, l.bytes);
      }
    }
    // Saves only this browser has.
    for (const l of local.values()) this.push(l.path, l.modified, l.bytes);
    this.setSeen(seen);
  }

  seen() { return JSON.parse(ls.get(META + 'seen') || '{}'); }
  setSeen(s) { ls.set(META + 'seen', JSON.stringify(s)); }

  push(path, modified, bytes) {
    const data = bytes ? bytes.slice().buffer : null; // own, unshared copy
    const seen = this.seen();
    seen[path] = Math.max(seen[path] ?? 0, modified);
    this.setSeen(seen);
    this.client.mutation(api.saves.push, { path, data, modified, device: this.device }).catch((e) => {
      console.error('pokegame: could not sync', path, e);
      this.onError?.(e);
    });
  }

  /** Install the hook the game calls when it writes a file. Pushes are coalesced per file. */
  watchLocal() {
    const pending = new Map();
    window.pokegameFileChanged = (path, ms, bytes) => {
      if (!SYNCED.test(path)) return;
      const entry = { ms: bytes ? ms : Date.now(), bytes: bytes ? new Uint8Array(bytes) : null };
      const had = pending.has(path);
      pending.set(path, entry);
      if (had) return;
      setTimeout(() => {
        const e = pending.get(path);
        pending.delete(path);
        this.push(path, e.ms, e.bytes);
      }, 800);
    };
  }

  /** Call `onChange(changes)` when another device saves while this one is open. */
  watchRemote(onChange) {
    this.client.onUpdate(api.saves.list, {}, (list) => {
      if (!list) return;
      const seen = this.seen();
      const changes = list.filter((r) => r.device !== this.device && r.modified > (seen[r.path] ?? 0));
      if (changes.length) onChange(changes);
    });
  }

  /** Accept other devices' versions: the page reloads and takes them. */
  takeRemote(paths) {
    sessionStorage.setItem(META + 'take-remote', JSON.stringify(paths));
    window.pokegameFileChanged = null;
    location.reload();
  }

  /** Keep this device's version: mark the others' as seen (the next save here wins). */
  keepLocal(changes) {
    const seen = this.seen();
    for (const c of changes) seen[c.path] = c.modified;
    this.setSeen(seen);
    for (const c of changes) {
      const l = readLocal(c.path);
      // Make this version the newest, so it replaces the other device's now.
      if (l) this.push(c.path, Math.max(Date.now(), c.modified + 1), l.bytes);
    }
  }
}

// ------------------------------------------------------------------ localStorage files

function localFiles() {
  const out = [];
  for (let i = 0; i < localStorage.length; i++) {
    const k = localStorage.key(i);
    if (!k?.startsWith(FILE)) continue;
    const path = k.slice(FILE.length);
    if (!SYNCED.test(path)) continue;
    const f = readLocal(path);
    if (f) out.push(f);
  }
  return out;
}

function readLocal(path) {
  const v = ls.get(FILE + path);
  if (v == null) return null;
  const nl = v.indexOf('\n');
  if (nl < 0) return null;
  const s = v.slice(nl + 1);
  const bytes = new Uint8Array(s.length);
  for (let i = 0; i < s.length; i++) bytes[i] = s.charCodeAt(i);
  return { path, modified: Number(v.slice(0, nl)) || 0, bytes };
}

function writeLocal(path, modified, bytes) {
  let s = '';
  for (let i = 0; i < bytes.length; i += 8192) s += String.fromCharCode.apply(null, bytes.subarray(i, i + 8192));
  ls.set(FILE + path, `${modified}\n${s}`);
}

function removeLocal(path) {
  ls.del(FILE + path);
}
