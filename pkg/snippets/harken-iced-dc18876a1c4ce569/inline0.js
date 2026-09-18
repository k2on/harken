
export async function cover(url, bound) {
  let store = null;
  try { store = await caches.open('harken-covers-v1'); } catch (e) { store = null; }
  let response = store ? await store.match(url) : null;
  if (!response) {
    response = await fetch(url, { mode: 'cors' });
    if (!response.ok) throw new Error('HTTP ' + response.status);
    // Put the clone, read the original: a Response body is a stream and can be
    // consumed exactly once, so reading it first leaves the cache nothing.
    if (store) { try { await store.put(url, response.clone()); } catch (e) {} }
  }
  // Decoded from the bytes, never from the URL. A canvas that has drawn a
  // cross-origin image is *tainted* and `getImageData` on it throws a
  // SecurityError — but a blob we are already holding is same-origin whatever
  // it came from, so going through the fetched bytes is what makes the pixels
  // readable at all.
  const source = await createImageBitmap(await response.blob());
  const long = Math.max(source.width, source.height);
  const scale = long > bound ? bound / long : 1;
  const w = Math.max(1, Math.round(source.width * scale));
  const h = Math.max(1, Math.round(source.height * scale));
  const canvas = typeof OffscreenCanvas === 'function'
    ? new OffscreenCanvas(w, h)
    : Object.assign(document.createElement('canvas'), { width: w, height: h });
  const context = canvas.getContext('2d', { willReadFrequently: true });
  context.drawImage(source, 0, 0, w, h);
  source.close();
  return { width: w, height: h, data: context.getImageData(0, 0, w, h).data };
}
