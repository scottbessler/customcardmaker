// A stand-in for Openverse and OpenRouter, for the e2e suite: a few fixed records, the
// same shapes the real service answers with, and one query that is always
// rate-limited. Run by playwright.config.ts; nothing here touches the network.
import { createServer } from "node:http";

const PORT = 18081;
const BASE = `http://127.0.0.1:${PORT}`;
// A real 1x1 PNG, so both an <img> and the server's signature check accept it.
const PNG = Buffer.from("iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAYAAAAfFcSJAAAADUlEQVR42mP8z8BQDwAEhQGAhKmMIQAAAABJRU5ErkJggg==", "base64");

const RECORDS = {
  "aaaaaaaa-aaaa-4aaa-8aaa-aaaaaaaaaaaa": { title: "Misty Forest", creator: "Jane Painter", license: "by", version: "2.0" },
  "bbbbbbbb-bbbb-4bbb-8bbb-bbbbbbbbbbbb": { title: "Pine Ridge", creator: "Lee Gardner", license: "cc0", version: "1.0" },
  "cccccccc-cccc-4ccc-8ccc-cccccccccccc": { title: "Night Market", creator: "Sam Lantern", license: "by-nc", version: "2.0" },
};

const record = (id) => {
  const r = RECORDS[id];
  const path = r.license === "cc0" ? "publicdomain/zero" : `licenses/${r.license}`;
  return {
    id, title: r.title, creator: r.creator, license: r.license, license_version: r.version, mature: false,
    license_url: `https://creativecommons.org/${path}/${r.version}/`,
    thumbnail: `${BASE}/v1/images/${id}/thumb/`,
    foreign_landing_url: `https://example.org/photos/${id}`, width: 640, height: 480,
  };
};

createServer(async (request, response) => {
  const url = new URL(request.url, BASE);
  const json = (status, body) => { response.writeHead(status, { "content-type": "application/json" }); response.end(JSON.stringify(body)); };
  if (url.pathname === "/health") return json(200, { ok: true });
  if (url.pathname === "/api/v1/chat/completions" && request.method === "POST") {
    if (request.headers.authorization !== "Bearer e2e-only") return json(401, {});
    let body = "";
    request.setEncoding("utf8");
    for await (const chunk of request) body += chunk;
    const payload = JSON.parse(body);
    if (payload.modalities?.includes("image")) {
      const image = payload.messages[0].content[1].image_url.url;
      if (!image.startsWith("data:image/") || !payload.image_config.aspect_ratio) return json(400, {});
      return json(200, { choices: [{ message: { images: [{ image_url: { url: `data:image/png;base64,${PNG.toString("base64")}` } }] } }] });
    }
    const original = payload.messages[1].content;
    const content = original === "{T}: draw a card" ? "{T}: Draw a card." : "{W}: Draw a card.";
    return json(200, { choices: [{ message: { content } }] });
  }
  if (url.pathname === "/v1/images/") {
    const q = (url.searchParams.get("q") ?? "").toLowerCase();
    if (q.includes("busy")) return json(429, { detail: "throttled" });
    if (q.includes("nothing")) return json(200, { result_count: 0, page_count: 0, results: [] });
    const wanted = url.searchParams.get("license") ?? "";
    const ids = Object.keys(RECORDS).filter((id) => wanted.split(",").includes(RECORDS[id].license));
    return json(200, { result_count: ids.length, page_count: 1, results: ids.map(record) });
  }
  const detail = url.pathname.match(/^\/v1\/images\/([0-9a-f-]{36})\/$/);
  if (detail && RECORDS[detail[1]]) return json(200, record(detail[1]));
  const thumb = url.pathname.match(/^\/v1\/images\/([0-9a-f-]{36})\/thumb\/$/);
  if (thumb && RECORDS[thumb[1]]) {
    // `?delay=300` answers slowly, so a test can watch an import run.
    const delay = Number(url.searchParams.get("delay") ?? 0);
    const send = () => { response.writeHead(200, { "content-type": "image/png" }); response.end(PNG); };
    if (delay > 0) return void setTimeout(send, delay);
    return void send();
  }
  json(404, { detail: "not found" });
}).listen(PORT, "127.0.0.1");
