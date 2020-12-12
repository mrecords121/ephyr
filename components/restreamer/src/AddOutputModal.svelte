<script lang="js">
  import { mutation } from 'svelte-apollo';

  import { AddOutput } from './api/graphql/client.graphql';

  import { showError } from './util';

  const addOutputMutation = mutation(AddOutput);

  export let show = false;

  export let input_id = "";

  let dst_url = "";
  $: if (!show) {
    dst_url = "";
  }

  $: submitable = dst_url.trim() !== "";

  function onAreaClick(event) {
    if (event.target.classList.contains('uk-modal')) {
      show = false;
    }
  }

  async function submit() {
    if (!submitable) return;
    try {
      await addOutputMutation({variables: {id: input_id, url: dst_url.trim()}});
      show = false;
    } catch (e) {
      showError(e.message);
    }
  }
</script>

<template>
<div class="uk-modal" class:uk-open="{show}" on:click={onAreaClick}>
  <div class="uk-modal-dialog uk-modal-body">
    <h2 class="uk-modal-title">Add new output destination for re-streaming</h2>
    <button class="uk-modal-close-outside" uk-close
            type="button" on:click={() => show = false}></button>

    <fieldset>
      <input class="uk-input" type="text" bind:value={dst_url}
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
