// The `forge` SDK, injected into HTML widgets (see DESIGN.md §5.4).
// The widget runs in a sandboxed iframe; every call goes to the host window
// over postMessage, and the core checks the widget's permissions again.
(function () {
  let seq = 0;
  const pending = new Map();
  const listeners = [];
  let resolveReady;
  const ready = new Promise((r) => (resolveReady = r));

  function call(method, args) {
    return new Promise((resolve, reject) => {
      const id = ++seq;
      pending.set(id, { resolve, reject });
      parent.postMessage({ forge: 1, type: "call", id, method, args }, "*");
    });
  }

  window.addEventListener("message", (e) => {
    if (e.source !== parent) return;
    const m = e.data;
    if (!m || m.forge !== 1) return;
    if (m.type === "reply") {
      const p = pending.get(m.id);
      if (!p) return;
      pending.delete(m.id);
      if (m.error) p.reject(new Error(m.error));
      else p.resolve(m.result);
    } else if (m.type === "init") {
      forge.settings.current = m.ctx.settings;
      resolveReady(m.ctx);
    } else if (m.type === "settings") {
      forge.settings.current = m.settings;
      for (const f of listeners) {
        try { f(m.settings); } catch (err) { console.error(err); }
      }
    }
  });

  const forge = {
    ready,
    system: () => call("system"),
    fetch: (url, options) => call("fetch", { url, options }),
    open: (url) => call("open", { url }),
    storage: {
      get: (key) => call("storage.get", { key }),
      set: (key, value) => call("storage.set", { key, value }),
    },
    settings: {
      current: {},
      onChange: (f) => listeners.push(f),
    },
  };
  window.forge = forge;
  parent.postMessage({ forge: 1, type: "hello" }, "*");
})();
