<script lang="js">
  import { mutation } from 'svelte-apollo';

  import { DisableInput, EnableInput } from './api/graphql/client.graphql';

  import { showError, copyToClipboard } from './util';

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
    {#each value.endpoints as endpoint}
      <span class="endpoint">
        <span
          class:uk-alert-danger={endpoint.status === 'OFFLINE'}
          class:uk-alert-warning={endpoint.status === 'INITIALIZING'}
          class:uk-alert-success={endpoint.status === 'ONLINE'}
        >
          {#if isFailover || endpoint.kind !== 'RTMP'}
            {#if endpoint.status === 'ONLINE'}
              <span
                ><i
                  class="fas fa-circle"
                  title="Serves {isFailover
                    ? 'failover '
                    : ''}live {endpoint.kind} stream"
                /></span
              >
            {:else if endpoint.status === 'INITIALIZING'}
              <span
                ><i
                  class="fas fa-dot-circle"
                  title="Serves {isFailover
                    ? 'failover '
                    : ''} live {endpoint.kind} stream"
                /></span
              >
            {:else}
              <span
                ><i
                  class="far fa-dot-circle"
                  title="Serves {isFailover
                    ? 'failover '
                    : ''} live {endpoint.kind} stream"
                /></span
              >
            {/if}
          {:else if isPull}
            <span
              ><i
                class="fas fa-arrow-down"
                title="Pulls {value.key} live {endpoint.kind} stream"
              />
            </span>
          {:else}
            <span
              ><i
                class="fas fa-arrow-right"
                title="Accepts {value.key} live {endpoint.kind} stream"
              />
            </span>
          {/if}
        </span>
        <span
          on:dblclick|preventDefault={(e) =>
            copyToClipboard(e.target.innerText)}
          title="Double-click to copy"
        >
          {#if endpoint.kind === 'HLS'}
            http://{public_host}:8000/hls/{restream_key}/{value.key}.m3u8
          {:else if isPull}
            {value.src.url}
          {:else}
            rtmp://{public_host}/{restream_key}/{value.key}
          {/if}
        </span>
      </span>
    {/each}
  </div>
</template>

<style lang="stylus">
  .fa-arrow-down, .fa-arrow-right
    font-size: 14px
    cursor: help
  .fa-circle, .fa-dot-circle
    font-size: 13px
    cursor: help

  .endpoint + .endpoint
    display: block
    margin-left: 45px
</style>
