<script lang="js">
  import { mutation } from 'svelte-apollo';

  import { TuneDelay, TuneVolume } from './api/graphql/client.graphql';

  import { showError, copyToClipboard } from './util';

  const tuneDelayMutation = mutation(TuneDelay);
  const tuneVolumeMutation = mutation(TuneVolume);

  export let value;
  export let restream_id;
  export let output_id;

  let volume = 100;
  let delay = 0;
  $: {
    // Trigger Svelte reactivity watching.
    value.volume = value.volume;
    value.delay = value.delay;
    // Move `volume` and `delay` to a separate function to omit triggering this
    // block when they are changed, as we're only interested in `value` changes
    // here.
    update_volumes_and_delay();
  }

  // Last used non-zero volume.
  let last_volume = value.volume === 0 ? 100 : value.volume;

  function update_volumes_and_delay() {
    volume = value.volume;
    delay = value.delay / 1000;
  }

  async function tuneVolume() {
    if (volume !== 0) {
      last_volume = volume;
    }
    const variables = {
      restream_id,
      output_id,
      mixin_id: value.id,
      volume,
    };
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

  async function tuneDelay() {
    const variables = {
      restream_id,
      output_id,
      mixin_id: value.id,
      delay: Math.round(delay * 1000),
    };
    try {
      await tuneDelayMutation({ variables });
    } catch (e) {
      showError(e.message);
    }
  }
</script>

<template>
  <div class="mixin">
    <i class="fas fa-wave-square" title="Mixed audio" />
    <span
      on:dblclick|preventDefault={() => copyToClipboard(value.src)}
      title="Double-click to copy">{value.src}</span
    >
    <div class="volume">
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
        max={value.src.startsWith('ts://') ? 1000 : 200}
        step="1"
        bind:value={volume}
        on:change={tuneVolume}
      />
      <span>{volume}%</span>
    </div>
    <div class="delay">
      <i class="far fa-clock" title="Delay" />
      <input
        class="uk-input"
        type="number"
        min="0"
        step="0.1"
        bind:value={delay}
        on:change={tuneDelay}
      />
      <span>s</span>
    </div>
  </div>
</template>

<style lang="stylus">
  .fa-volume-up, .fa-volume-mute
    font-size: 10px
  .fa-wave-square, .fa-clock
    font-size: 10px
    color: #d9d9d9

  .mixin
    margin-top: 6px
    padding-left: 34px

  .volume
    padding-left: 17px
    font-size: 10px

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
