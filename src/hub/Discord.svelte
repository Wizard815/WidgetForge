<script>
  // Hub → Settings → Discord: sign in once; Discord widgets then use the account.
  // PLACEHOLDER wording.
  import { onMount } from "svelte";
  import { api } from "../lib/api.js";

  let { clientId = "", onsaved } = $props();
  let status = $state(null);
  let busy = $state(false);
  let message = $state("");
  let idInput = $state("");

  async function refresh() {
    try {
      status = await api.discordStatus();
    } catch (e) {
      message = String(e);
    }
  }

  async function run(fn) {
    busy = true;
    message = "";
    try {
      await fn();
    } catch (e) {
      message = String(e);
    }
    busy = false;
    await refresh();
  }

  function saveId(e) {
    e.preventDefault();
    run(async () => {
      await api.setDiscordClientId(idInput);
      onsaved?.();
      message = "Client ID saved.";
    });
  }

  onMount(() => {
    idInput = clientId ?? "";
    refresh();
  });
</script>

<h2>Discord</h2>
<div class="box">
  {#if status?.connected}
    <p>
      Signed in as <strong>{status.user.global_name ?? status.user.username}</strong>
      <span class="muted">({status.user.username})</span>
    </p>
    <button disabled={busy} onclick={() => run(api.discordLogout)}>Sign out</button>
  {:else}
    <p class="muted">
      Sign in so Discord widgets can show your servers. WidgetForge asks only for your name and server list;
      it cannot read your messages. The sign-in is kept in {status ? "your system keychain" : "…"}.
    </p>
    <button class="primary" disabled={busy || !status?.configured} onclick={() => run(api.discordLogin)}>
      {busy ? "Waiting for Discord in your browser…" : "Sign in with Discord"}
    </button>
    {#if status?.error}<p class="error">{status.error}</p>{/if}
  {/if}
  {#if message}<p class="note">{message}</p>{/if}

  <details open={!status?.configured}>
    <summary>Discord application (one-time setup)</summary>
    <ol>
      <li>Open the Discord Developer Portal → Applications → New Application.</li>
      <li>OAuth2 → Redirects → add <code>{status?.redirect ?? "http://127.0.0.1:47821/discord/callback"}</code></li>
      <li>OAuth2 → turn on <strong>Public Client</strong> (so no secret has to be stored in the app).</li>
      <li>Copy the <strong>Client ID</strong> and paste it here.</li>
    </ol>
    <form class="id" onsubmit={saveId}>
      <input placeholder="Client ID" bind:value={idInput} inputmode="numeric" />
      <button type="submit" disabled={busy}>Save</button>
    </form>
  </details>
</div>

<style>
  h2 { font-size: 15px; margin: 22px 0 8px; }
  .box { padding: 12px 14px; border: 1px solid #222a3a; border-radius: 10px; background: #141925; }
  .box p { margin: 0 0 10px; }
  .muted { opacity: 0.65; }
  .error { color: #ffb4b4; }
  .note { color: #9fe6b8; }
  details { margin-top: 12px; font-size: 13px; }
  summary { cursor: pointer; opacity: 0.8; }
  ol { padding-left: 20px; line-height: 1.7; }
  .id { display: flex; gap: 8px; }
  .id input { flex: 1; max-width: 320px; padding: 6px 10px; border: 1px solid #2c3548; border-radius: 8px; background: #1b2230; }
  button.primary { background: #5865f2; border-color: #6c78ff; }
</style>
