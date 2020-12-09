<svelte:options immutable={true}/>

<script lang="js">
  import slugify from 'slugify';
  import { createEventDispatcher } from 'svelte';
  import { mutation } from 'svelte-apollo';

  import {
    DisableInput, EnableInput, RemoveInput,
    DisableOutput, EnableOutput, RemoveOutput,
  } from './api/graphql/client.graphql';

  import Toggle from "./Toggle.svelte";
  import {showError} from "./util";

  const disableInputMutation = mutation(DisableInput);
  const enableInputMutation = mutation(EnableInput);
  const removeInputMutation = mutation(RemoveInput);
  const disableOutputMutation = mutation(DisableOutput);
  const enableOutputMutation = mutation(EnableOutput);
  const removeOutputMutation = mutation(RemoveOutput);

  const dispatch = createEventDispatcher();

  export let id;
  export let public_host = "localhost";
  export let value;

  let isPull = value.input.__typename === 'PullInput';

  async function removeInput() {
    try {
      await removeInputMutation({ variables: {
          id: isPull ? value.input.src : value.input.name,
        } });
    } catch (e) {
      showError(e.message);
    }
  }

  async function toggleInput() {
    let vars = {variables: {id: isPull ? value.input.src : value.input.name}};
    try {
      if (value.enabled) {
        await disableInputMutation(vars);
      } else {
        await enableInputMutation(vars);
      }
    } catch (e) {
      showError(e.message);
    }
  }

  function openAddOutputModal() {
    dispatch('open_output_modal', {
      input_id: isPull ? value.input.src : value.input.name,
    });
  }

  function toggleOutput(url) {
    let vars = {variables: {
        id: isPull ? value.input.src : value.input.name,
        url: url,
      }};
    return async () => {
      let output = value.outputs.find(o => o.dst === url);
      try {
        if (output && output.enabled) {
          await disableOutputMutation(vars);
        } else {
          await enableOutputMutation(vars);
        }
      } catch (e) {
        showError(e.message);
      }
    }
  }

  function removeOutput(url) {
    let vars = {variables: {
        id: isPull ? value.input.src : value.input.name,
        url: url,
      }};
    return async () => {
      try {
        await removeOutputMutation(vars);
      } catch (e) {
        showError(e.message);
      }
    }
  }
</script>

<template>
<div class="uk-section uk-section-muted uk-section-xsmall">
  <button type="button" class="uk-close" uk-close
          on:click={removeInput}></button>

  <button class="uk-button uk-button-primary uk-button-small"
          on:click={openAddOutputModal}>
    <i class="fas fa-plus"></i>&nbsp;<span>Add output</span>
  </button>

  <Toggle id="input-toggle-{id}"
          checked={value.enabled}
          on:change={toggleInput}/>
  <span>
    <span class:uk-alert-danger={value.input.status === 'OFFLINE'}
          class:uk-alert-warning={value.input.status === 'INITIALIZING'}
          class:uk-alert-success={value.input.status === 'ONLINE'}>
      <i class="fas"
         class:fa-arrow-down={isPull}
         class:fa-arrow-right={!isPull}
         title="{ isPull ? 'Pulls' : 'Accepts'} RTMP stream"></i>
    </span>
    {#if isPull}
      { value.input.src }
    {:else}
      rtmp://{public_host}/{ value.input.name }/in
    {/if}
  </span>

  {#if value.outputs}
    <div class="uk-grid uk-grid-small" uk-grid>
      {#each value.outputs as output (output) }
        <div class="uk-card uk-card-default uk-card-body uk-margin-left">
          <button type="button" class="uk-close" uk-close
                  on:click={removeOutput(output.dst)}></button>

          <Toggle id="output-toggle-{slugify(output.dst)}" size="8px"
                  checked={output.enabled}
                  on:change={toggleOutput(output.dst)}/>
          {#if output.status === 'ONLINE'}
            <i class="fas fa-circle uk-alert-success"></i>
          {:else if output.status === 'INITIALIZING'}
            <i class="fas fa-dot-circle uk-alert-warning"></i>
          {:else}
            <i class="far fa-dot-circle uk-alert-danger"></i>
          {/if}
          <span>{output.dst}</span>
        </div>
      {/each}
    </div>
  {/if}
</div>
</template>

<style lang="stylus">
  .uk-section
    margin-top: 10px
    padding-left: 10px
    padding-right: @padding-left

    .uk-close
      float: right
      margin-top: 5px

    .uk-button-small
      float: right
      font-size: 0.7rem
      margin-top: -2px
      margin-right: 40px

    .fa-arrow-down, .fa-arrow-right
      font-size: 14px
      cursor: help
    .fa-circle, .fa-dot-circle
      font-size: 10px
      margin-top: -1px

    .uk-grid
      margin-top: 10px

      .uk-card
        margin-top: 10px !important

      .uk-margin-left
        margin-left: 15px !important

    .uk-card
      padding: 6px
      width: calc((100% - (15px * 2)) / 2)
      min-width 250px
      font-size: 13px
      @media screen and (max-width: 600px)
        width: 100%

      .uk-close
        margin-top: 3px
</style>
