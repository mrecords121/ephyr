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
  $: allEnabled = value.outputs.every(o => o.enabled);

  $: onlineCount = value.outputs.filter(o => o.status === 'ONLINE').length;
  $: initCount = value.outputs.filter(o => o.status === 'INITIALIZING').length;
  $: offlineCount = value.outputs.filter(o => o.status === 'OFFLINE').length;
  $: presentBitmask = (onlineCount > 0 ? 1 : 0 ) +
                      2 * (initCount > 0 ? 1 : 0 ) +
                      4 * (offlineCount > 0 ? 1 : 0 );

  let enabledBitmask = 0;
  $: if (enabledBitmask === presentBitmask) {
    enabledBitmask = 0;
  }

  $: showAll = ((enabledBitmask & presentBitmask) === presentBitmask) ||
               ((enabledBitmask & presentBitmask) === 0);
  $: showFiltered = {
    ONLINE: !showAll && ((enabledBitmask & 1) === 1),
    INITIALIZING: !showAll && ((enabledBitmask & 2) === 2),
    OFFLINE: !showAll && ((enabledBitmask & 4) === 4),
  };

  function openEditInputModal() {
    inputModal.openEdit(
      value.id,
      isPull ? value.input.src : value.input.name,
      value.label,
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
      if (allEnabled) {
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

  {#if !!value.label}
    <span class="label">{value.label}</span>
  {/if}

  {#if value.outputs && value.outputs.length > 0}
    <span class="total">
      {#if offlineCount > 0}
        <a href="/" on:click|preventDefault={() => enabledBitmask ^= 4}
           class:enabled={showFiltered['OFFLINE']}
           class="count uk-alert-danger">{offlineCount}</a>
      {/if}
      {#if initCount > 0}
        <a href="/" on:click|preventDefault={() => enabledBitmask ^= 2}
           class:enabled={showFiltered['INITIALIZING']}
           class="count uk-alert-warning">{initCount}</a>
      {/if}
      {#if onlineCount > 0}
        <a href="/" on:click|preventDefault={() => enabledBitmask ^= 1}
           class:enabled={showFiltered['ONLINE']}
           class="count uk-alert-success">{onlineCount}</a>
      {/if}
      <Toggle id="all-outputs-toggle-{value.id}"
              checked={allEnabled}
              title="{allEnabled ? 'Disable' : 'Enable'} all outputs"
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
        <div class="uk-card uk-card-default uk-card-body uk-margin-left"
             class:hidden={!showAll && !showFiltered[output.status]}>
          <button type="button" class="uk-close" uk-close
                  on:click={removeOutput(output.id)}></button>

          {#if output.label}
            <span class="label">{output.label}</span>
          {/if}

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
    position: relative
    margin-top: 20px
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
        margin-right: 2px
        background-color: inherit
        padding: 1px 4px
        border-radius: 2px
        outline: none
        &:hover, &.enabled
          background-color: #cecece
        &:hover
          color: inherit
          text-decoration: none

    .fa-arrow-down, .fa-arrow-right
      font-size: 14px
      cursor: help
    .fa-circle, .fa-dot-circle
      font-size: 10px
      margin-top: -1px

    .uk-grid
      margin-top: 10px

      .uk-card
        margin-top: 15px !important

      .uk-margin-left
        margin-left: 15px !important

    .label
      position: absolute
      top: -12px
      left: 0
      padding: 2px 10px
      border-top-left-radius: 4px
      border-top-right-radius: 4px
      background-color: #f8f8f8

    .uk-card
      position: relative
      padding: 6px
      width: calc((100% - (15px * 2)) / 2)
      min-width 250px
      font-size: 13px
      @media screen and (max-width: 600px)
        width: 100%
      &.hidden
        display: none

      .uk-close
        margin-top: 3px

      .label
        top: -12px
        padding: 0 6px
        font-size: 13px
        background-color: #fff

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
