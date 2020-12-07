<script lang="js">
  import { mutation } from 'svelte-apollo';

  import { AddPullInput, AddPushInput } from './api/graphql/client.graphql';

  import { showError } from './util';

  const addPullInputMutation = mutation(AddPullInput);
  const addPushInputMutation = mutation(AddPushInput);

  export let public_address = "localhost";

  let is_pull = false;

  let pull_url = "";
  let push_key = "";

  export let show = false;
  $: if (!show) {
    pull_url = "";
    push_key = "";
  }

  $: submitable = (is_pull && pull_url.trim() !== "") ||
                  (!is_pull && push_key.trim() !== "");

  function onAreaClick(event) {
    if (event.target.classList.contains('uk-modal')) {
      show = false;
    }
  }

  async function submit() {
    if (!submitable) return;
    try {
      if (is_pull) {
        await addPullInputMutation({variables: {url: pull_url.trim()}});
      } else {
        await addPushInputMutation({variables: {key: push_key.trim()}});
      }
      show = false;
    } catch (e) {
      showError(e.message);
    }
  }
</script>

<template>
<div class="uk-modal" class:uk-open="{show}" on:click={onAreaClick}>
  <div class="uk-modal-dialog uk-modal-body" class:is-pull={is_pull}>
    <h2 class="uk-modal-title">Add new source for re-streaming</h2>
    <button class="uk-modal-close-outside" uk-close
            type="button" on:click={() => show = false}></button>

    <ul uk-tab>
      <li class="uk-active">
        <a href="/" on:click={() => is_pull = false}>Push</a>
      </li>
      <li>
        <a href="/" on:click={() => is_pull = true}>Pull</a>
      </li>
    </ul>

    <fieldset class="pull-form">
      <input class="uk-input" type="text" bind:value={pull_url}
             placeholder="rtmp://...">
      <div class="uk-alert">
        Server will pull RTMP stream from this address
      </div>
    </fieldset>

    <fieldset class="push-form">
      <label>rtmp://{public_address}/<input class="uk-input" type="text"
                                            placeholder="<stream-name>"
                                            bind:value={push_key}>/in</label>
      <div class="uk-alert">
        Server will await RTMP stream to be published onto this address
      </div>
    </fieldset>

    <button class="uk-button uk-button-primary"
            disabled="{!submitable}"
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
    .pull-form
      display: none
    .push-form
      display: block
      input
        display: inline
        width: auto
        margin-top: -5px
      label
        padding-left: 15px
    .is-pull
      .pull-form
        display: block
      .push-form
        display: none
</style>

