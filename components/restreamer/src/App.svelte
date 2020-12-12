<script lang="js">
  import { InMemoryCache } from '@apollo/client/cache';
  import { ApolloClient } from '@apollo/client/core';
  import { WebSocketLink } from '@apollo/client/link/ws';
  import { SubscriptionClient } from 'subscriptions-transport-ws';
  import { setClient, subscribe, query } from 'svelte-apollo';

  import { Info, State } from './api/graphql/client.graphql';

  import { showError } from './util';

  import UIkit from 'uikit';
  import Icons from 'uikit/dist/js/uikit-icons';

  import { inputModal } from './stores.js';

  import InputModal from './InputModal.svelte';
  import OutputModal from './OutputModal.svelte';
  import Restream from './Restream.svelte';

  UIkit.use(Icons);

  let isOnline = true;

  const wsClient = new SubscriptionClient(
    //'ws://127.0.0.1/api',
    'ws' + (window.location.protocol === 'https:' ? 's' : '') + '://' +
           window.location.host +
           window.location.pathname.replace(/\/?$/g, '') + '/api',
    { reconnect: true },
  );
  wsClient.onConnected(() => {
    isOnline = true;
    refetchInfo();
  });
  wsClient.onReconnected(() => {
    isOnline = true;
    refetchInfo();
  });
  wsClient.onDisconnected(() => isOnline = false);
  const gqlClient = new ApolloClient({
    link: new WebSocketLink(wsClient),
    cache: new InMemoryCache(),
  });
  setClient(gqlClient);

  const info = query(Info);
  const state = subscribe(State, {errorPolicy: 'all'});

  function refetchInfo() {
    info.refetch();
  }
</script>

<template>
  <header class="uk-container">
    {#if isOnline && $info.data}
      <button class="uk-button uk-button-primary"
              on:click={() => inputModal.openAdd()}>
        <i class="fas fa-plus"></i>&nbsp;<span>Input</span>
      </button>
      <InputModal public_host="{$info.data.info.publicHost}"/>
      <OutputModal/>
    {:else if $info.error}
      {showError($info.error.message)}
    {/if}

    <img class="logo" src="logo.jpg" alt="Logo">

    <h3>Ephyr re-streamer v0.1.0-beta.2</h3>
  </header>

  <main class="uk-container">
    {#if !isOnline || $state.loading}
      <div class="uk-alert uk-alert-warning loading">Loading...</div>
    {:else if isOnline && $state.data && $info.data}
      {#each $state.data.state.restreams as restream}
        <Restream public_host="{$info.data.info.publicHost}"
                  value="{restream}"/>
      {/each}
    {/if}
    {#if $info.error}
      {showError($info.error.message)}
    {/if}
  </main>
</template>

<style lang="stylus" global>
  @require "../node_modules/uikit/dist/css/uikit.min.css"

  h2, h3
    color: #666

  header
    padding: 10px

    button
      float: right
    h3
      margin: 4px 44px 4px 52px
    .logo
      width: 44px
      height: @width
      float: left

  main
    > .loading
      text-align: center

  .uk-button-primary
    background-color: #08c
    &:not([disabled]):hover
      background-color: #046
</style>
