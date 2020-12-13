<script lang="js">
  import { onDestroy } from 'svelte';
  import { mutation } from 'svelte-apollo';

  import { AddOutput } from './api/graphql/client.graphql';

  import { outputModal as value } from './stores.js';

  import { sanitize, showError } from './util';

  const addOutputMutation = mutation(AddOutput);

  let submitable = false;
  let invalidLine;
  const unsubscribe = value.subscribe(v => {
    submitable = (!v.multi && v.url !== "") ||
                 (v.multi && v.list !== "" && !invalidLine);
  });
  onDestroy(() => unsubscribe());

  function sanitizeList(list) {
    if (list === '') return list;
    return list.trim().split(/\r\n|\r|\n/)
      .map(line => line.trim().split(',')
        .map(v => sanitize(v))
        .filter(v => v !== '')
        .join(','))
      .filter(line => line !== '')
      .join("\n");
  }

  function revalidateList() {
    value.update(v => {
      v.list = sanitizeList(v.list);

      invalidLine = v.list.split(/\r\n|\r|\n/)
        .find(line => line.split(',').length > 2);

      return v;
    })
  }

  function onAreaClick(event) {
    if (event.target.classList.contains('uk-modal')) {
      value.close();
    }
  }

  async function submit() {
    revalidateList();
    if (!submitable) return;

    let submit = [];
    const v = value.get();
    if (v.multi) {
      v.list.split(/\r\n|\r|\n/)
        .forEach(line => {
          const vs = line.split(',');
          let vars = {input_id: v.input_id, url: vs[vs.length - 1]};
          if (vs.length > 1) {
            vars.label = vs[0];
          }
          submit.push(vars);
        });
    } else {
      let vars = {input_id: v.input_id, url: sanitize(v.url)};
      const label = sanitize(v.label);
      if (label !== '') {
        vars.label = label;
      }
      submit.push(vars);
    }
    if (submit.length < 1) return;

    let failed = [];
    await Promise.all(submit.map(async (vars) => {
      try {
        await addOutputMutation({variables: vars});
      } catch (e) {
        showError("Failed to add " + vars.url + ":\n" + e.message);
        failed.push(vars);
      }
    }))
    if (failed.length < 1) {
      value.close();
      return;
    }
    value.update(v => {
      v.list = failed.map(vars => {
        return (vars.label ? vars.label + ',' : '') + vars.url;
      }).join("\n");
      return v;
    });
  }
</script>

<template>
<div class="uk-modal" class:uk-open="{$value.visible}" on:click={onAreaClick}>
  <div class="uk-modal-dialog uk-modal-body" class:is-multi={$value.multi}>
    <h2 class="uk-modal-title">
      Add new output destination{$value.multi ? 's' : ''} for re-streaming
    </h2>
    <button class="uk-modal-close-outside" uk-close
            type="button" on:click={() => value.close()}></button>

    <ul class="uk-tab">
      <li class:uk-active={!$value.multi}>
        <a href="/"
           on:click|preventDefault={() => value.switchSingle()}>Single</a>
      </li>
      <li class:uk-active={$value.multi}>
        <a href="/"
           on:click|preventDefault={() => value.switchMulti()}>Multiple</a>
      </li>
    </ul>

    <fieldset class="single-form">
      <input class="uk-input uk-form-small" type="text" bind:value={$value.label}
             placeholder="optional label">
      <input class="uk-input" type="text" bind:value={$value.url}
             placeholder="rtmp://...">
      <div class="uk-alert">
        Server will publish input RTMP stream to this address
      </div>
    </fieldset>

    <fieldset class="multi-form">
      {#if !!invalidLine}
        <span class="uk-form-danger line-err">Invalid line: {invalidLine}</span>
      {/if}
      <textarea class="uk-textarea" class:uk-form-danger={!!invalidLine}
                bind:value={$value.list}
                on:change={revalidateList}
                placeholder="One line - one address (with optional label):
label1,rtmp://1...
rtmp://2...
label3,rtmp://3..."></textarea>
      <div class="uk-alert">
        Server will publish input RTMP stream to these addresses
      </div>
    </fieldset>

    <button class="uk-button uk-button-primary"
            disabled={!submitable}
            on:click={submit}>Add</button>
  </div>
</div>
</template>

<style lang="stylus">
  .uk-modal
    &.uk-open
      display: block

    .uk-modal-title
      font-size: 1.5rem

    fieldset
      border: none

      .uk-form-small
        width: auto
        margin-bottom: 5px

      .uk-textarea
        min-height: 200px
        resize: none

      .uk-alert
        font-size: 14px
        margin-bottom: 0

      .line-err
        display: block
        font-size: 11px

    .single-form
      display: block
    .multi-form
      display: none
    .is-multi
      .single-form
        display: none
      .multi-form
        display: block
</style>
