import UIkit from 'uikit';

/**
 * Displays error UI popup with the given error `message`.
 *
 * @param message    Error message to be displayed.
 */
export function showError(message: string) {
  UIkit.notification(message, { status: 'danger', pos: 'top-center' });
}

/**
 * Sanitizes the given `label` by replacing any space-like characters sequences
 * with a single space.
 *
 * @param label    Label to be sanitized.
 *
 * @returns    Sanitized label.
 */
export function sanitizeLabel(label: string): string {
  return label.replace(/[\s]+/g, ' ').trim();
}

/**
 * Sanitizes the given `url` by removing any space-like characters from it.
 *
 * @param url    URL to be sanitized.
 *
 * @returns    Sanitized URL.
 */
export function sanitizeUrl(url: string): string {
  return url.replace(/[\s]+/g, '');
}

/**
 * Checks whether the given location page corresponds to
 * `/restream/:restream_id/output/:output_id` route.
 *
 * @param location    Location to be checked.
 */
export function isOutputPage(location: string): boolean {
  const p = location.split('/');
  return p[1] === 'restream' && p[3] === 'output';
}
