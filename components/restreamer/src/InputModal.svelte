<script lang="js">
  import { onDestroy } from 'svelte';
  import { get } from 'svelte/store';
  import { mutation } from 'svelte-apollo';

  import { AddPullInput, AddPushInput } from './api/graphql/client.graphql';

  import { sanitizeLabel, sanitizeUrl, showError } from './util';

  import { inputModal as value } from './stores.js';

  const addPullInputMutation = mutation(AddPullInput);
  const addPushInputMutation = mutation(AddPushInput);

  export let public_host = "localhost";

  let submitable = false;
  onDestroy(value.subscribe(v => {
    const val = v.is_pull ? v.pull_url : v.push_key;
    if (!v.edit_id) {
      submitable = (val !== "");
    } else {
      submitable = (val !== "") &&
                   ((val !== v.prevValue) || (v.label !== v.prevLabel));
    }
  }));

  function onAreaClick(event) {
    if (event.target.classList.contains('uk-modal')) {
      value.close();
    }
  }

  async function submit() {
    if (!submitable) return;
    const v = get(value);
    let p = {variables: v.edit_id ? {replace_id: v.edit_id} : {}};
    const label = sanitizeLabel(v.label);
    if (label !== '') {
      p.variables.label = label;
    }
    try {
      if (v.is_pull) {
        p.variables.url = sanitizeUrl(v.pull_url);
        await addPullInputMutation(p);
      } else {
        p.variables.key = sanitizeUrl(v.push_key);
        await addPushInputMutation(p);
      }
      value.close();
    } catch (e) {
      showError(e.message);
    }
  }
</script>

<template>
<div class="uk-modal" class:uk-open={$value.visible} on:click={onAreaClick}>
  <div class="uk-modal-dialog uk-modal-body">
    <h2 class="uk-modal-title">
      {#if $value.edit_id}Edit{:else}Add new{/if} input source for re-streaming
    </h2>
    <button class="uk-modal-close-outside" uk-close
            type="button" on:click={() => value.close()}></button>

    <ul class="uk-tab">
      <li class:uk-active={!$value.is_pull}>
        <a href="/" on:click|preventDefault={() => value.switchPush()}>Push</a>
      </li>
      <li class:uk-active={$value.is_pull}>
        <a href="/" on:click|preventDefault={() => value.switchPull()}>Pull</a>
      </li>
    </ul>

    <fieldset class:is-push={!$value.is_pull}>
      <input class="uk-input uk-form-small" type="text"
             bind:value={$value.label}
             on:change={() => value.sanitizeLabel()}
             placeholder="optional label">
      {#if $value.is_pull}
        <input class="uk-input" type="text" bind:value={$value.pull_url}
               placeholder="rtmp://...">
        <div class="uk-alert">
          Server will pull RTMP stream from this address
        </div>
      {:else}
        <label>rtmp://{public_host}/<input class="uk-input" type="text"
                                        placeholder="<stream-name>"
                                        bind:value={$value.push_key}>/in</label>
        <div class="uk-alert">
          Server will await RTMP stream to be published onto this address
        </div>
      {/if}
    </fieldset>

    <button class="uk-button uk-button-primary"
            disabled={!submitable}
            on:click={submit}>{#if $value.edit_id}Edit{:else}Add{/if}</button>
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

      .uk-form-small
        display: block
        width: auto
        margin-bottom: 10px

      &.is-push
        display: block

        input:not(.uk-form-small)
          display: inline
          width: auto
          margin-top: -5px

        .uk-form-small
          margin-bottom: 15px

        label
          padding-left: 15px
</style>

