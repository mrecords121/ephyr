<script lang="js">
  export let id;
  export let checked = false;
  export let size = '10px';
</script>

<template>
  <span class="toggle" style="font-size:{size}">
    <input type="checkbox" bind:checked="{checked}" id="{id}" on:change>
    <label for="{id}" class="toggle"></label>
  </span>
</template>

<style lang="stylus">
  input[type="checkbox"]
    display: none

    & + label
      cursor: pointer
      position: relative
      display: inline-block
      height: 2em
      width: 4em
      background: #cecece
      border-radius: 20em
      vertical-align: middle
      transition: .3s background
      user-select: none
      margin-top: -4px

      &, &::before, &::after
        box-sizing: border-box

      &::before, &::after
        position: absolute
        top: 0
        left: 0
      &::before
        content: ''
        display: inline-block
        background: rgba(0, 0, 0, .5)
        height: 1.5em
        width: 1.5em
        margin: .25em
        border-radius: 50%
        z-index: 200
        transition:.3s left
      &::after
        color: #fff
        content: '\f00d'
        font: 1.25em 'Font Awesome 5 Pro'
        top: 50%
        margin-top: -.5em
        left: 2em

    &:checked + label
      background: #08c

      &::before
        left: 2em
      &::after
        content: '\f00c'
        left: .5em
</style>
