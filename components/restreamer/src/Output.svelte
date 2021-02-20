<script lang="js">
  import { mutation } from 'svelte-apollo';

  import {
    DisableOutput,
    EnableOutput,
    RemoveOutput,
    TuneDelay,
    TuneVolume,
  } from './api/graphql/client.graphql';

  import { showError } from './util';

  import Toggle from './Toggle.svelte';

  const disableOutputMutation = mutation(DisableOutput);
  const enableOutputMutation = mutation(EnableOutput);
  const removeOutputMutation = mutation(RemoveOutput);
  const tuneDelayMutation = mutation(TuneDelay);
  const tuneVolumeMutation = mutation(TuneVolume);

  export let value;
  export let input_id;
  export let hidden = false;

  let orig_volume = 100;
  let mix_volume = 100;
  let mix_delay = 0;

  $: {
    // Trigger Svelte reactivity watching.
    value.volume = value.volume;
    if (value.mixins.length > 0) {
      value.mixins[0].volume = value.mixins[0].volume;
      value.mixins[0].delay = value.mixins[0].delay;
    }
    // Move `orig_volume`, `mix_volume` and `mix_delay` to a separate function
    // to omit triggering this block when they are changed, as we're only
    // interested in `value` changes here.
    justify_volumes_and_delay();
  }

  function justify_volumes_and_delay() {
    orig_volume = value.volume;
    if (value.mixins.length > 0) {
      mix_volume = value.mixins[0].volume;
      mix_delay = value.mixins[0].delay / 1000;
    }
  }

  async function toggle() {
    const vars = { input_id: input_id, output_id: value.id };
    try {
      if (value.enabled) {
        await disableOutputMutation({ variables: vars });
      } else {
        await enableOutputMutation({ variables: vars });
      }
    } catch (e) {
      showError(e.message);
    }
  }

  async function remove() {
    const vars = { input_id: input_id, output_id: value.id };
    try {
      await removeOutputMutation({ variables: vars });
    } catch (e) {
      showError(e.message);
    }
  }

  async function tuneDelay(mixin_id) {
    const vars = {
      input_id: input_id,
      output_id: value.id,
      mixin_id: mixin_id,
      delay: Math.round(mix_delay * 1000),
    };
    try {
      await tuneDelayMutation({ variables: vars });
    } catch (e) {
      showError(e.message);
    }
  }

  async function tuneVolume(mixin_id) {
    let vars = { input_id: input_id, output_id: value.id };
    if (!!mixin_id) {
      vars.mixin_id = mixin_id;
      vars.volume = mix_volume;
    } else {
      vars.volume = orig_volume;
    }
    try {
      await tuneVolumeMutation({ variables: vars });
    } catch (e) {
      showError(e.message);
    }
  }
</script>

<template>
  <div class="uk-card uk-card-default uk-card-body uk-margin-left" class:hidden>
    <button type="button" class="uk-close" uk-close on:click={remove} />

    {#if value.label}
      <span class="label">{value.label}</span>
    {/if}

    <Toggle
      id="output-toggle-{value.id}"
      classes="small"
      checked={value.enabled}
      on:change={toggle}
    />
    {#if value.status === 'ONLINE'}
      <span><i class="fas fa-circle uk-alert-success" /></span>
    {:else if value.status === 'INITIALIZING'}
      <span><i class="fas fa-dot-circle uk-alert-warning" /></span>
    {:else}
      <span><i class="far fa-dot-circle uk-alert-danger" /></span>
    {/if}
    <span>{value.dst}</span>

    {#if value.mixins.length > 0}
      <div class="volume orig">
        <i class="fas fa-volume-up" title="Volume" />
        <input
          class="uk-range"
          type="range"
          min="0"
          max="200"
          step="1"
          bind:value={orig_volume}
          on:change={() => tuneVolume(null)}
        />
        <span>{orig_volume}%</span>
      </div>

      <div class="mixin">
        <i class="fas fa-wave-square" title="Mixed audio" />
        <span>{value.mixins[0].src}</span>
        <div class="volume">
          <i class="fas fa-volume-up" title="Volume" />
          <input
            class="uk-range"
            type="range"
            min="0"
            max="1000"
            step="1"
            bind:value={mix_volume}
            on:change={() => tuneVolume(value.mixins[0].id)}
          />
          <span>{mix_volume}%</span>
        </div>
        <div class="delay">
          <i class="far fa-clock" title="Delay" />
          <input
            class="uk-input"
            type="number"
            min="0"
            step="0.1"
            bind:value={mix_delay}
            on:change={() => tuneDelay(value.mixins[0].id)}
          />
          <span>s</span>
        </div>
      </div>
    {/if}
  </div>
</template>

<style lang="stylus">
  .uk-margin-left
    margin-left: 15px !important

  .uk-card
    position: relative
    padding: 6px
    margin-top: 15px !important
    width: calc((100% - (15px * 2)) / 2)
    min-width 250px
    font-size: 13px
    @media screen and (max-width: 600px)
      width: 100%
    &.hidden
      display: none

    .uk-close
      float: right
      margin-top: 3px

    .label
      position: absolute
      top: -12px
      left: 0
      padding: 0 6px
      font-size: 13px
      border-top-left-radius: 4px
      border-top-right-radius: 4px
      background-color: #fff

  .fa-circle, .fa-dot-circle
    font-size: 10px
    margin-top: -1px

  .fa-volume-up, .fa-wave-square, .fa-clock
    font-size: 10px
    color: #d9d9d9

  .mixin
    margin-top: 6px
    padding-left: 34px

  .volume
    padding-left: 17px
    font-size: 10px

    &.orig
      margin-left: 34px

    .uk-range::-moz-range-thumb, .uk-range::-webkit-slider-thumb
      width: 7px
      height: 12px
    .uk-range
      display: inline-block
      width: 70%
      margin-top: -1px

  .delay
    padding-left: 17px
    font-size: 10px

    .uk-input
      height: auto
      width: 40px
      padding: 0
      border: none
      margin-top: -2px
      text-align: right
</style>
