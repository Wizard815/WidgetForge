<script lang="ts">
  import { hub } from "../../lib/api";
  import type { AppSettings } from "../../lib/types";
  import Icon from "../components/Icon.svelte";
  import Segmented from "../components/Segmented.svelte";
  import Toggle from "../components/Toggle.svelte";
  import { act, app, refresh } from "../store.svelte";

  const s = $derived(app.state!.settings);

  async function update(patch: Partial<AppSettings>) {
    await act(() => hub.updateSettings(patch));
    await refresh();
  }

  async function quit() {
    await act(() => hub.quit());
  }

  const PLATFORM: Record<string, string> = { windows: "Windows", macos: "macOS", linux: "Linux" };
</script>

<div class="page">
  <div class="page-head">
    <div>
      <h1>Settings</h1>
      <p>How WidgetForge starts and looks.</p>
    </div>
  </div>

  <section class="card group">
    <h2>Startup</h2>
    <Toggle
      label="Start WidgetForge when I sign in"
      hint="Your widgets appear at their saved positions. Runs quietly in the tray — Steam doesn't need to be open."
      checked={s.launch_on_startup}
      onchange={(v) => update({ launch_on_startup: v })} />
    <Toggle
      label="Show this window when WidgetForge starts"
      hint="Doesn't apply to sign-in starts, which only show your widgets."
      checked={s.show_hub_on_launch}
      onchange={(v) => update({ show_hub_on_launch: v })} />
    <p class="muted small note">Choose which widgets open at startup with “Start with WidgetForge” on each widget in My desktop.</p>
  </section>

  <section class="card group">
    <h2>Appearance</h2>
    <div class="row">
      <div>
        <div>Theme</div>
        <div class="muted small">For this window. Widgets have their own styles.</div>
      </div>
      <Segmented
        value={s.theme}
        options={[
          { value: "system", label: "System" },
          { value: "dark", label: "Dark" },
          { value: "light", label: "Light" },
        ]}
        onchange={(v) => update({ theme: v })} />
    </div>
  </section>

  <section class="card group">
    <h2>About</h2>
    <dl>
      <dt>Version</dt>
      <dd>{app.state?.version}</dd>
      <dt>Platform</dt>
      <dd>{PLATFORM[app.state?.platform ?? ""] ?? app.state?.platform}</dd>
      <dt>Add-ons folder</dt>
      <dd class="mono">{app.state?.addons_dir}</dd>
    </dl>
    <div class="actions" style="margin-top:14px">
      <button class="btn danger" onclick={quit}><Icon name="power" size={15} /> Quit WidgetForge</button>
    </div>
    <p class="muted small note">Closing this window keeps your widgets running. Quit from here or from the tray icon.</p>
  </section>
</div>

<style>
  .group {
    padding: 16px 20px;
    margin-bottom: 14px;
  }
  .group h2 {
    margin-bottom: 6px;
  }
  .row {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 16px;
    padding: 8px 0;
  }
  .note {
    margin-top: 6px;
  }
  dl {
    display: grid;
    grid-template-columns: 140px 1fr;
    gap: 6px 12px;
    margin: 8px 0 0;
  }
  dt {
    color: var(--muted);
  }
  dd {
    margin: 0;
    overflow-wrap: anywhere;
  }
</style>
