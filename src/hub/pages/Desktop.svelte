<script lang="ts">
  import Icon from "../components/Icon.svelte";
  import InstanceCard from "../components/InstanceCard.svelte";
  import { app } from "../store.svelte";

  const instances = $derived(app.state?.instances ?? []);
</script>

<div class="page">
  <div class="page-head">
    <div>
      <h1>My desktop</h1>
      <p>The widgets you've placed. Drag them around on your desktop; change how they behave here.</p>
    </div>
    <div class="actions">
      <button class="btn primary" onclick={() => (app.page = "library")}><Icon name="plus" size={16} /> Add widget</button>
    </div>
  </div>

  {#if instances.length === 0}
    <div class="empty card">
      <img src="/logo.svg" alt="" width="56" height="56" />
      <h2>Your desktop is empty</h2>
      <p class="muted">Pick a clock, a system monitor or some sticky notes from the library to get started.</p>
      <button class="btn primary" onclick={() => (app.page = "library")}>Browse the library</button>
    </div>
  {:else}
    <div class="list">
      {#each instances as inst (inst.id)}
        <InstanceCard {inst} />
      {/each}
    </div>
    <p class="tip faint small">
      Tip: hover a widget on your desktop for its toolbar — close, settings, layer and lock. Right where you need them.
    </p>
  {/if}
</div>

<style>
  .list {
    display: flex;
    flex-direction: column;
    gap: 10px;
  }
  .empty {
    display: flex;
    flex-direction: column;
    align-items: center;
    text-align: center;
    gap: 10px;
    padding: 56px 24px;
  }
  .empty p {
    max-width: 380px;
    margin-bottom: 8px;
  }
  .tip {
    margin-top: 18px;
  }
</style>
