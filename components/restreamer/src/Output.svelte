<script lang="js">
  import { mutation } from 'svelte-apollo';

  import {
    DisableOutput,
    EnableOutput,
    RemoveOutput,
    TuneVolume,
  } from './api/graphql/client.graphql';

  import { showError } from './util';

  import Toggle from './Toggle.svelte';
  import Mixin from './Mixin.svelte';

  const disableOutputMutation = mutation(DisableOutput);
  const enableOutputMutation = mutation(EnableOutput);
  const removeOutputMutation = mutation(RemoveOutput);
  const tuneVolumeMutation = mutation(TuneVolume);

  export let value;
  export let restream_id;
  export let hidden = false;

  let volume = 100;
  $: {
    // Trigger Svelte reactivity watching.
    value.volume = value.volume;
    // Move `volume` to a separate function to omit triggering this block when
    // it is changed, as we're only interested in `value` changes here.
    update_volume();
  }

  // Last used non-zero volume.
  let last_volume = value.volume === 0 ? 100 : value.volume;

  function update_volume() {
    volume = value.volume;
  }

  async function toggle() {
    const variables = { restream_id, output_id: value.id };
    try {
      if (value.enabled) {
        await disableOutputMutation({ variables });
      } else {
        await enableOutputMutation({ variables });
      }
    } catch (e) {
      showError(e.message);
    }
  }

  async function remove() {
    const variables = { restream_id, output_id: value.id };
    try {
      await removeOutputMutation({ variables });
    } catch (e) {
      showError(e.message);
    }
  }

  async function tuneVolume() {
    if (volume !== 0) {
      last_volume = volume;
    }
    const variables = { restream_id, output_id: value.id, volume };
    try {
      await tuneVolumeMutation({ variables });
    } catch (e) {
      showError(e.message);
    }
  }

  async function toggleVolume() {
    volume = volume !== 0 ? 0 : last_volume;
    await tuneVolume();
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
        <a href="/" on:click|preventDefault={toggleVolume}>
          {#if volume > 0}
            <span><i class="fas fa-volume-up" title="Volume" /></span>
          {:else}
            <span><i class="fas fa-volume-mute" title="Muted" /></span>
          {/if}
        </a>
        <input
          class="uk-range"
          type="range"
          min="0"
          max="200"
          step="1"
          bind:value={volume}
          on:change={tuneVolume}
        />
        <span>{volume}%</span>
      </div>

      {#each value.mixins as mixin}
        <Mixin {restream_id} output_id={value.id} value={mixin} />
      {/each}
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
  .fa-volume-up, .fa-volume-mute
    font-size: 10px

  .volume
    padding-left: 17px
    font-size: 10px

    &.orig
      margin-left: 34px

    a
      color: #d9d9d9
      outline: none
      &:hover
        text-decoration: none
        color: #c4c4c4

    .uk-range::-moz-range-thumb, .uk-range::-webkit-slider-thumb
      width: 7px
      height: 12px
    .uk-range
      display: inline-block
      width: 70%
      margin-top: -1px
</style>
