<script>
  // Runs an HTML/JS widget in a sandboxed iframe and answers its SDK calls.
  import { onMount } from "svelte";
  import { api, widgetFile } from "../lib/api.js";

  let { manifest, instance, settings } = $props();
  let frame;
  let ready = $state(false);

  const plain = (v) => JSON.parse(JSON.stringify(v ?? null));
  const post = (msg) => frame?.contentWindow?.postMessage({ forge: 1, ...msg }, "*");

  async function answer(method, args) {
    switch (method) {
      case "system": return api.systemStats(instance.id);
      case "storage.get": return api.storageGet(instance.id, args.key);
      case "storage.set": return api.storageSet(instance.id, args.key, args.value);
      case "open": return api.openUrl(args.url);
      case "fetch": {
        const o = args.options ?? {};
        return api.netFetch(instance.id, { url: args.url, method: o.method, headers: o.headers, body: o.body });
      }
      default: throw `unknown call ${method}`;
    }
  }

  onMount(() => {
    const onMessage = async (e) => {
      if (!frame || e.source !== frame.contentWindow) return;
      const m = e.data;
      if (!m || m.forge !== 1) return;
      if (m.type === "hello") {
        post({ type: "init", ctx: plain({ manifest, settings, instance: { id: instance.id } }) });
        ready = true;
      } else if (m.type === "call") {
        try {
          post({ type: "reply", id: m.id, result: plain(await answer(m.method, m.args ?? {})) });
        } catch (err) {
          post({ type: "reply", id: m.id, error: String(err) });
        }
      }
    };
    window.addEventListener("message", onMessage);
    return () => window.removeEventListener("message", onMessage);
  });

  // Pass settings changes on to the widget.
  $effect(() => {
    const s = plain(settings);
    if (ready) post({ type: "settings", settings: s });
  });
</script>

<iframe
  bind:this={frame}
  title={manifest.name}
  sandbox="allow-scripts"
  src={widgetFile(manifest.id, manifest.ui.entry)}
></iframe>

<style>
  iframe {
    flex: 1;
    width: 100%;
    border: 0;
    background: transparent;
    color-scheme: normal;
  }
</style>
