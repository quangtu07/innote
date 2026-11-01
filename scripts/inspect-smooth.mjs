// Screenshot the smoothing view end-to-end so we can inspect "thu bé / mất phân đoạn" + duplicate buttons.
import { readFileSync, mkdirSync } from "node:fs";
import { resolve } from "node:path";
import { pathToFileURL } from "node:url";
const modulePath = process.env.PLAYWRIGHT_MODULE;
const { chromium } = await import(modulePath ? pathToFileURL(modulePath).href : "playwright");
const output = resolve(process.env.UI_TEST_OUTPUT ?? "test-results/ui-smooth-inspect");
mkdirSync(output, { recursive: true });
export const browser = await chromium.launch({ channel: process.env.PLAYWRIGHT_CHANNEL ?? "chrome", headless: true });
const mocks = readFileSync("node_modules/@tauri-apps/api/mocks.js", "utf8").replace(/export \{[^}]+};/g, "");
const ids = ["aaaaaaaa-aaaa-4aaa-8aaa-aaaaaaaaaaaa", "bbbbbbbb-bbbb-4bbb-8bbb-bbbbbbbbbbbb", "cccccccc-cccc-4ccc-8ccc-cccccccccccc"];
const init = ({ ids }) => {
  const make = (id, title) => ({
    id, title, notes: "", created: "2026-10-25T12:00:00+07:00", duration: 120, processed: 120, status: "completed",
    language: "vi", model: "phowhisper-small",
    segments: [
      { id: "s1", start: 1, end: 8, text: "Xin chào mọi người, hôm nay chúng ta sẽ chốt phương án ra mắt sản phẩm mới.", edited: false },
      { id: "s2", start: 9, end: 16, text: "Về mặt công nghệ, Innote xử lý 100% trên máy tính bằng PhoWhisper của VinAI nên dữ liệu cực kỳ an toàn.", edited: false },
      { id: "s3", start: 17, end: 26, text: "Tuyệt vời! Anh Tuấn phụ trách gửi báo cáo tài chính trước 17h thứ Sáu nhé.", edited: false },
    ],
    warnings: [], error: null,
  });
  window.qa = {
    calls: [],
    snapshot: {
      sessions: [make(ids[0], "Cuộc họp Alpha")],
      devices: [{ id: "d", name: "Loa thử", kind: "system", default: true }],
      models: [{ id: "phowhisper-small", name: "PhoWhisper", bytes: 487601984, installed: true }],
      active_id: null,
      data_dir: "C:\\QA",
    },
    exports: [],
  };
  localStorage.setItem("may.ui_locale", "vi");
  mockConvertFileSrc("windows");
  mockIPC(async (cmd, args) => {
    const qa = window.qa;
    qa.calls.push({ cmd, args });
    if (cmd === "snapshot") return structuredClone(qa.snapshot);
    if (cmd === "audio_path") return args.id + ".wav";
    if (cmd === "set_llm_key") { qa.snapshot.llm_configured = !!args.key; return; }
    if (cmd === "test_llm_connection") return "ok";
    if (cmd === "smooth_transcript") {
      const s = qa.snapshot.sessions.find((x) => x.id === args.id);
      s.smoothed = { text: "Xin chào mọi người. Hôm nay chúng ta sẽ chốt phương án ra mắt sản phẩm mới. Về mặt công nghệ, Innote xử lý 100% trên máy tính bằng PhoWhisper của VinAI nên dữ liệu cực kỳ an toàn. Tuyệt vời. Anh Tuấn phụ trách gửi báo cáo tài chính trước 17h thứ Sáu nhé.", provider: args.provider, model: args.model, prompt: args.prompt, created: "2026-10-30T00:00:00Z" };
      return structuredClone(s);
    }
    if (cmd === "refresh_devices") return;
  }, { shouldMockEvents: true });
};
const context = await browser.newContext({ viewport: { width: 1180, height: 800 } });
await context.addInitScript({ content: `${mocks}\n(${init.toString()})(${JSON.stringify({ ids })});` });
const p = await context.newPage();
p.setDefaultTimeout(15000);
await p.route("http://asset.localhost/**", (r) => r.fulfill({ status: 200, body: Buffer.alloc(320044), contentType: "audio/wav" }));
await p.goto(process.env.UI_TEST_URL ?? "http://127.0.0.1:1420", { timeout: 30000 });
await p.getByRole("button", { name: "Danh sách cuộc họp", exact: true }).waitFor();
await p.getByRole("button", { name: "Danh sách cuộc họp", exact: true }).click();
await p.locator("dialog.meetings-dialog .meeting-card").filter({ hasText: "Cuộc họp Alpha" }).click();
await p.waitForTimeout(700);
await p.screenshot({ path: resolve(output, "1-original-view.png") });

// configure + smooth
await p.getByRole("button", { name: "Cài đặt", exact: true }).click();
await p.getByLabel(/API key cho AI/).fill("sk-test");
await p.getByRole("button", { name: "Kiểm tra API", exact: true }).click();
await p.locator(".ai-settings").getByRole("button", { name: "Lưu API key", exact: true }).click();
await p.getByRole("button", { name: "Danh sách cuộc họp", exact: true }).click();
await p.locator("dialog.meetings-dialog .meeting-card").filter({ hasText: "Cuộc họp Alpha" }).click();
await p.waitForTimeout(500);
await p.getByRole("button", { name: "Làm mịn", exact: true }).click();
const d = p.getByRole("dialog");
const testBtn = d.getByRole("button", { name: "Kiểm tra API", exact: true });
if (await testBtn.isVisible().catch(() => false)) await testBtn.click();
await d.getByRole("button", { name: "Làm mịn", exact: true }).click();
await p.waitForTimeout(1500);
await p.screenshot({ path: resolve(output, "2-smoothed-view.png") });

// back to original, then tab again
await p.getByRole("button", { name: "Về Bản gốc", exact: true }).click();
await p.waitForTimeout(400);
await p.screenshot({ path: resolve(output, "3-back-original.png") });
await p.locator(".smooth-tabs .transcript-tab").filter({ hasText: "Bản làm mịn" }).click();
await p.waitForTimeout(400);
await p.screenshot({ path: resolve(output, "4-smoothed-again.png") });

console.log("ALL SCREENSHOTS DONE");
await browser.close();