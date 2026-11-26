// The one place the islands talk to the server.

async function failure(response) {
  try {
    return (await response.json()).error;
  } catch {
    return `Request failed (${response.status})`;
  }
}

export async function api(method, path, body) {
  const init = { method };
  if (body !== undefined) {
    init.headers = { "content-type": "application/json" };
    init.body = JSON.stringify(body);
  }
  const response = await fetch(path, init);
  if (!response.ok) throw new Error(await failure(response));
  return response.status === 204 ? null : response.json();
}

/** Sends an image's bytes as the body; the server decides what it really is. */
export async function uploadArt(blob) {
  const response = await fetch("/api/art", { method: "POST", headers: { "content-type": blob.type }, body: blob });
  if (!response.ok) {
    throw new Error(response.status === 413 ? "That image is too large." : await failure(response));
  }
  return response.json();
}
