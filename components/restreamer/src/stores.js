import { writable, get } from 'svelte/store';

function newInputModal() {
  const { subscribe, set, update } = writable({
    edit_id: null,
    prev: null,
    push_key: "",
    pull_url: "",
    visible: false,
    is_pull: false,
  });

  return {
    subscribe,
    set,
    get: () => get({subscribe}),
    openAdd: () => update(v => {
      v.visible = true;
      return v;
    }),
    openEdit: (id, val, is_pull) => update(v => {
      v.edit_id = id;
      v.prev = val;
      v.is_pull = is_pull;
      if (v.is_pull) {
        v.pull_url = val;
      } else {
        v.push_key = val;
      }
      v.visible = true;
      return v;
    }),
    setPullUrl: url => update(v => {
      v.pull_url = url;
      return v;
    }),
    setPushKey: key => update(v => {
      v.push_key = key;
      return v;
    }),
    switchPull: () => update(v => {
      v.is_pull = true;
      return v;
    }),
    switchPush: () => update(v => {
      v.is_pull = false;
      return v;
    }),
    close: () => update(v => {
      v.edit_id = null;
      v.prev = null;
      v.push_key = "";
      v.pull_url = "";
      v.visible = false;
      return v;
    }),
  };
}

function newOutputModal() {
  const { subscribe, set, update } = writable({
    input_id: null,
    value: "",
    visible: false,
  });

  return {
    subscribe,
    set,
    get: () => get({subscribe}),
    open: id => update(v => {
      v.input_id = id;
      v.visible = true;
      return v;
    }),
    close: () => update(v => {
      v.input_id = null;
      v.value = "";
      v.visible = false;
      return v;
    }),
  };
}

export const inputModal = newInputModal();
export const outputModal = newOutputModal();
