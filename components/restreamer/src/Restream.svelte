<svelte:options immutable={true}/>

<script lang="js">
  import { createEventDispatcher } from 'svelte';
  import { mutation } from 'svelte-apollo';

  import {
    DisableInput, EnableInput, RemoveInput,
    DisableOutput, EnableOutput, RemoveOutput,
    DisableAllOutputs, EnableAllOutputs,
  } from './api/graphql/client.graphql';

  import {showError} from "./util";

  import { inputModal, outputModal } from './stores.js';

  import Toggle from "./Toggle.svelte";

  const disableInputMutation = mutation(DisableInput);
  const enableInputMutation = mutation(EnableInput);
  const removeInputMutation = mutation(RemoveInput);
  const disableAllOutputsMutation = mutation(DisableAllOutputs);
  const disableOutputMutation = mutation(DisableOutput);
  const enableAllOutputsMutation = mutation(EnableAllOutputs);
  const enableOutputMutation = mutation(EnableOutput);
  const removeOutputMutation = mutation(RemoveOutput);

  const dispatch = createEventDispatcher();

  export let public_host = "localhost";
  export let value;

  $: isPull = value.input.__typename === 'PullInput';

  function openEditInputModal() {
    inputModal.openEdit(
      value.id,
      isPull ? value.input.src : value.input.name,
      isPull,
    );
  }

  async function removeInput() {
    try {
      await removeInputMutation({variables: {id: value.id}});
    } catch (e) {
      showError(e.message);
    }
  }

  async function toggleInput() {
    const vars = {variables: {id: value.id}};
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
    outputModal.open(value.id);
  }

  function toggleOutput(id) {
    const vars = {variables: {input_id: value.id, output_id: id}};
    return async () => {
      let output = value.outputs.find(o => o.id === id);
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

  async function toggleAllOutputs() {
    if (value.outputs.length < 1) return;
    try {
      if (value.outputs.every(o => o.enabled)) {
        await disableAllOutputsMutation({variables: {input_id: value.id}});
      } else {
        await enableAllOutputsMutation({variables: {input_id: value.id}});
      }
    } catch (e) {
      showError(e.message);
    }
  }

  function removeOutput(id) {
    const vars = {variables: {input_id: value.id, output_id: id}};
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
    <i class="fas fa-plus"></i>&nbsp;<span>Output</span>
  </button>

  {#if value.outputs && value.outputs.length > 0}
    <span class="total">
      <span class="count">{value.outputs.length}</span>
      <Toggle id="all-outputs-toggle-{value.id}"
              checked={value.outputs.every(o => o.enabled)}
              title="Toggle all outputs"
              on:change={toggleAllOutputs}/>
    </span>
  {/if}

  <Toggle id="input-toggle-{value.id}"
          checked={value.enabled}
          on:change={toggleInput}/>
  <span>
    <span class:uk-alert-danger={value.input.status === 'OFFLINE'}
          class:uk-alert-warning={value.input.status === 'INITIALIZING'}
          class:uk-alert-success={value.input.status === 'ONLINE'}>
      {#key isPull}
        <span>
          <i class="fas"
             class:fa-arrow-down={isPull}
             class:fa-arrow-right={!isPull}
             title="{ isPull ? 'Pulls' : 'Accepts'} RTMP stream"></i>
        </span>
      {/key}
    </span>
    <span>
      {#if isPull}
        { value.input.src }
      {:else}
        rtmp://{public_host}/{ value.input.name }/in
      {/if}
    </span>
    <a class="edit-input" href="/" on:click|preventDefault={openEditInputModal}>
      <i class="far fa-edit" title="Edit input"></i>
    </a>
  </span>

  {#if value.outputs && value.outputs.length > 0}
    <div class="uk-grid uk-grid-small" uk-grid>
      {#each value.outputs as output (output) }
        <div class="uk-card uk-card-default uk-card-body uk-margin-left">
          <button type="button" class="uk-close" uk-close
                  on:click={removeOutput(output.id)}></button>

          <Toggle id="output-toggle-{output.id}" classes="small"
                  checked={output.enabled}
                  on:change={toggleOutput(output.id)}/>
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
      margin-right: 30px

    .total
      float: right
      margin-right: 30px
      .count
        text-align: right
        margin-right: 5px

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

  .edit-input
    margin-left: 6px
    color: #666
    opacity: 0
    transition: opacity .3s ease
    &:hover
      color: #444
  .uk-section:hover
    .edit-input
      opacity: 1
</style>
