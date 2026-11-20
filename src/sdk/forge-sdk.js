/* WidgetForge SDK — injected into HTML widgets. Talks to the host window over
 * postMessage; the host checks every call against the widget's permissions.
 * Docs: docs/DESIGN.md §5.4 */
(function () {
  "use strict";
  if (window.forge) return;
  var pending = new Map();
  var seq = 0;
  var settingsListeners = [];

  function call(method, params) {
    return new Promise(function (resolve, reject) {
      var id = ++seq;
      pending.set(id, { resolve: resolve, reject: reject });
      window.parent.postMessage({ __forge: 1, id: id, method: method, params: params || {} }, "*");
    });
  }

  window.addEventListener("message", function (e) {
    if (e.source !== window.parent) return;
    var m = e.data;
    if (!m || m.__forge !== 1) return;
    if (m.id && pending.has(m.id)) {
      var p = pending.get(m.id);
      pending.delete(m.id);
      if (m.error) p.reject(new Error(m.error));
      else p.resolve(m.result);
    } else if (m.event === "settings") {
      forge.settings.current = m.data;
      settingsListeners.forEach(function (f) {
        try { f(m.data); } catch (err) { console.error(err); }
      });
    }
  });

  var forge = {
    /** Resolves with { instance, manifest, settings } once the host is ready. */
    ready: null,
    context: null,
    /** System stats (needs permissions.system). */
    system: function () { return call("system"); },
    /** Like window.fetch, limited to hosts in permissions.network (https only). */
    fetch: function (url, opts) {
      opts = opts || {};
      return call("fetch", { url: String(url), method: opts.method, headers: opts.headers, body: opts.body }).then(function (r) {
        return {
          status: r.status,
          ok: r.status >= 200 && r.status < 300,
          headers: { "content-type": r.content_type },
          text: function () { return Promise.resolve(r.body); },
          json: function () { return Promise.resolve(JSON.parse(r.body)); },
        };
      });
    },
    /** Per-instance storage (needs permissions.storage). */
    storage: {
      get: function (key) { return call("storage.get", { key: key }); },
      set: function (key, value) { return call("storage.set", { key: key, value: value === undefined ? null : value }); },
      all: function () { return call("storage.all"); },
    },
    settings: {
      current: {},
      onChange: function (fn) { settingsListeners.push(fn); },
    },
    /** Opens an http(s) link in the user's browser. */
    openUrl: function (url) { return call("openUrl", { url: String(url) }); },
  };

  forge.ready = call("context").then(function (ctx) {
    forge.context = ctx;
    forge.settings.current = ctx.settings;
    return ctx;
  });
  window.forge = forge;
})();
