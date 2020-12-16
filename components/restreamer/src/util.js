import UIkit from 'uikit';

export function showError(message) {
  UIkit.notification(message, {status: 'danger', pos: 'top-center'});
  return '';
}

export function sanitizeLabel(url) {
  return url.replace(/[\s]+/g, ' ').trim();
}

export function sanitizeUrl(url) {
  return url.replace(/[\s]+/g, '');
}
