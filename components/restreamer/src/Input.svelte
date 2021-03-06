<script lang="js">
  import { mutation } from 'svelte-apollo';

  import { DisableInput, EnableInput } from './api/graphql/client.graphql';

  import { showError } from './util';

  import Toggle from './Toggle.svelte';

  const disableInputMutation = mutation(DisableInput);
  const enableInputMutation = mutation(EnableInput);

  export let public_host = 'localhost';
  export let restream_id;
  export let restream_key;
  export let value;

  $: isPull = !!value.src && value.src.__typename === 'RemoteInputSrc';
  $: isFailover = !!value.src && value.src.__typename === 'FailoverInputSrc';

  async function toggle() {
    const variables = { restream_id, input_id: value.id };
    try {
      if (value.enabled) {
        await disableInputMutation({ variables });
      } else {
        await enableInputMutation({ variables });
      }
    } catch (e) {
      showError(e.message);
    }
  }
</script>

<template>
  <div class="input">
    <Toggle
      id="input-toggle-{value.id}"
      checked={value.enabled}
      on:change={toggle}
    />
    <span
      class:uk-alert-danger={value.status === 'OFFLINE'}
      class:uk-alert-warning={value.status === 'INITIALIZING'}
      class:uk-alert-success={value.status === 'ONLINE'}
    >
      {#if isFailover}
        {#if value.status === 'ONLINE'}
          <span
            ><i
              class="fas fa-circle"
              title="Serves failover live stream"
            /></span
          >
        {:else if value.status === 'INITIALIZING'}
          <span
            ><i
              class="fas fa-dot-circle"
              title="Serves failover live stream"
            /></span
          >
        {:else}
          <span
            ><i
              class="far fa-dot-circle"
              title="Serves failover live stream"
            /></span
          >
        {/if}
      {:else if isPull}
        <span
          ><i class="fas fa-arrow-down" title="Pulls {value.key} live stream" />
        </span>
      {:else}
        <span
          ><i
            class="fas fa-arrow-right"
            title="Accepts {value.key} live stream"
          />
        </span>
      {/if}
    </span>
    <span>
      {#if isPull}
        {value.src.url}
      {:else}
        rtmp://{public_host}/{restream_key}/{value.key}
      {/if}
    </span>
  </div>
</template>

<style lang="stylus">
  .fa-arrow-down, .fa-arrow-right
    font-size: 14px
    cursor: help
  .fa-circle, .fa-dot-circle
    font-size: 12px
    cursor: help
</style>
