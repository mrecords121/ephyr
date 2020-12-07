<svelte:options immutable={true}/>

<script lang="js">
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
  export let public_address;
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

  function toggleOutput(index) {
    let vars = {variables: {
        id: isPull ? value.input.src : value.input.name,
        url: value.outputs[index].dst,
      }};
    return async () => {
      try {
        if (value.outputs[index].enabled) {
          await disableOutputMutation(vars);
        } else {
          await enableOutputMutation(vars);
        }
      } catch (e) {
        showError(e.message);
      }
    }
  }

  function removeOutput(index) {
    let vars = {variables: {
        id: isPull ? value.input.src : value.input.name,
        url: value.outputs[index].dst,
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
    <i class="fas"
       class:fa-arrow-down={isPull}
       class:fa-arrow-right={!isPull}
       class:uk-alert-danger={value.input.status === 'OFFLINE'}
       class:uk-alert-warning={value.input.status === 'INITIALIZING'}
       class:uk-alert-success={value.input.status === 'OFFLINE'}
       title="{ isPull ? 'Pulls' : 'Accepts'} RTMP stream"></i>
    {#if isPull}
      { value.input.src }
    {:else}
      rtmp://{public_address}/{ value.input.name }/in
    {/if}
  </span>

  {#if value.outputs}
    <div class="uk-grid uk-grid-small" uk-grid>
      {#each value.outputs as output, i}
        <div class="uk-card uk-card-default uk-card-body uk-margin-left">
          <button type="button" class="uk-close" uk-close
                  on:click={removeOutput(i)}></button>

          <Toggle id="output-toggle-{i}" size="8px"
                  checked={output.enabled}
                  on:change={toggleOutput(i)}/>

          <i class="fa-dot-circle"
             class:uk-alert-danger={['OFFLINE'].includes(output.status)}
             class:uk-alert-warning={['INITIALIZING'].includes(output.status)}
             class:uk-alert-success={['ONLINE'].includes(output.status)}
             class:far={['OFFLINE'].includes(output.status)}
             class:fas={['INITIALIZING', 'ONLINE'].includes(output.status)}
             class:fa-dot-circle={['OFFLINE', 'INITIALIZING'].includes(output.status)}
             class:fa-circle={['ONLINE'].includes(output.status)}></i>
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
      font-size: 12px

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
