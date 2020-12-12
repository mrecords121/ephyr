import UIkit from 'uikit';

export function showError(message) {
  UIkit.notification(message, {status: 'danger', pos: 'top-center'});
  return '';
}

export function sanitize(url) {
  return url.replace(/[\s\r\n\t]+/g, '');
}
