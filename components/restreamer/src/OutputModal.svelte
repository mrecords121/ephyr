<script lang="js">
  import { onDestroy } from 'svelte';
  import { mutation } from 'svelte-apollo';

  import { AddOutput } from './api/graphql/client.graphql';

  import { outputModal as value } from './stores.js';

  import { sanitize, showError } from './util';

  const addOutputMutation = mutation(AddOutput);

  let submitable = false;
  const unsubscribe = value.subscribe(v => {
    submitable = v.value !== "";
  });
  onDestroy(() => unsubscribe());

  function onAreaClick(event) {
    if (event.target.classList.contains('uk-modal')) {
      value.close();
    }
  }

  async function submit() {
    if (!submitable) return;
    const v = value.get();
    const vars = {variables: {input_id: v.input_id, url: sanitize(v.value)}};
    try {
      await addOutputMutation(vars);
      value.close();
    } catch (e) {
      showError(e.message);
    }
  }
</script>

<template>
<div class="uk-modal" class:uk-open="{$value.visible}" on:click={onAreaClick}>
  <div class="uk-modal-dialog uk-modal-body">
    <h2 class="uk-modal-title">Add new output destination for re-streaming</h2>
    <button class="uk-modal-close-outside" uk-close
            type="button" on:click={() => value.close()}></button>

    <fieldset>
      <input class="uk-input" type="text" bind:value={$value.value}
             placeholder="rtmp://...">
      <div class="uk-alert">
        Server will publish input RTMP stream to this address
      </div>
    </fieldset>

    <button class="uk-button uk-button-primary"
            disabled={!submitable}
            on:click={submit}>Add</button>
  </div>
</div>
</template>

<style lang="stylus">
  .uk-modal
    &.uk-open
      display: block

    .uk-modal-title
      font-size: 1.5rem

    fieldset
      border: none
      .uk-alert
        font-size: 14px
        margin-bottom: 0
</style>
