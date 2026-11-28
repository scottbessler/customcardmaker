// Getting a picture ready to send: read it, shrink it, re-encode it.
//
// Done in the browser so a 12-megapixel photo never travels, and because
// re-drawing onto a canvas drops metadata (location, camera) and applies the
// photo's rotation. The server still checks what arrives; this is courtesy.

/** The art box is about twice as wide as it is tall, so 1400px is generous. */
const MAX_EDGE = 1400;
const MAX_SOURCE_BYTES = 30 * 1024 * 1024;
const QUALITY = 0.9;

export async function prepareArt(file) {
  if (!file.type.startsWith("image/")) throw new Error("Choose an image file.");
  if (file.size > MAX_SOURCE_BYTES) throw new Error("That image is too large (30 MB at most).");
  let bitmap;
  try {
    bitmap = await createImageBitmap(file);
  } catch {
    throw new Error("That image could not be read. Try a PNG, JPEG or WebP.");
  }
  const scale = Math.min(1, MAX_EDGE / Math.max(bitmap.width, bitmap.height));
  const canvas = document.createElement("canvas");
  canvas.width = Math.max(1, Math.round(bitmap.width * scale));
  canvas.height = Math.max(1, Math.round(bitmap.height * scale));
  const context = canvas.getContext("2d");
  // JPEG has no transparency; a see-through PNG would otherwise turn black.
  context.fillStyle = "white";
  context.fillRect(0, 0, canvas.width, canvas.height);
  context.drawImage(bitmap, 0, 0, canvas.width, canvas.height);
  bitmap.close();
  const blob = await new Promise((resolve) => canvas.toBlob(resolve, "image/jpeg", QUALITY));
  if (!blob) throw new Error("That image could not be prepared.");
  return blob;
}

/** Where a card's art is served from. */
export function artUrl(id) {
  return `/api/art/${id}`;
}

/** CSS alignment for a cover crop, shared by the art dialog and card renderer. */
function artPosition(position) {
  const [x, y] = position ?? [50, 50];
  return `${x}% ${y}%`;
}

/** Background declarations for a crop in a frame or art dialog preview. */
export function artBackground({ art, art_position, art_zoom }) {
  const position = artPosition(art_position);
  return `background-image:url(${artUrl(art)});background-position:${position};--art-origin:${position};--art-scale:${(art_zoom ?? 100) / 100}`;
}
