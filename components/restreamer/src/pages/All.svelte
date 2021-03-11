<script lang="js">
  import Restream from '../Restream.svelte';

  export let state;
  export let info;
</script>

<template>
  {#each $state.data.allRestreams as restream}
    <Restream public_host={$info.data.info.publicHost} value={restream} />
  {/each}
</template>
