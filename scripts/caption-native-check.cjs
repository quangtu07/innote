// Opt-in QA for a freshly built development backend (WebView2 debug port).
const assert = require("node:assert/strict");
const fs = require("node:fs");
const path = require("node:path");
const { chromium } = require(process.env.PLAYWRIGHT_MODULE || "playwright");
(async () => {
  const browser = await chromium.connectOverCDP("http://127.0.0.1:9223");
  try {
    const main = browser
      .contexts()
      .flatMap((c) => c.pages())
      .find((p) => p.url().endsWith(":1420/"));
    assert(
      main,
      "Start the freshly built Tauri development backend with WebView2 port 9223",
    );
    const invoke = (cmd, args) =>
      main.evaluate(
        ({ cmd, args }) => window.__TAURI_INTERNALS__.invoke(cmd, args),
        { cmd, args },
      );
    const results = [];
    const profiles = await invoke("list_ai_profiles");
    let rejected;
    try {
      await invoke("test_ai_connection", {
        input: {
          provider: "soniox",
          apiKey: "invalid-key-for-innote-qa",
          baseUrl: "",
          model: "",
          profileId: null,
        },
      });
    } catch (e) {
      rejected = String(e);
    }
    assert(rejected && /401|key|quyền/.test(rejected), rejected);
    assert(!rejected.includes("invalid-key-for-innote-qa"));
    results.push({
      name: "Real Soniox rejects invalid key without exposing it",
      error: rejected,
    });
    try {
      await invoke("save_ai_profile", {
        input: {
          id: null,
          name: "QA must not save",
          provider: "soniox",
          apiKey: "invalid-key-for-innote-qa",
          baseUrl: "",
        },
      });
      assert.fail("Unverified key saved");
    } catch (e) {
      assert(/kiểm tra/.test(String(e)));
    }
    assert.equal((await invoke("list_ai_profiles")).length, profiles.length);
    results.push({
      name: "Backend refuses unverified profile; user profiles unchanged",
      passed: true,
    });
    assert.equal(await invoke("caption_running"), false);
    console.log(JSON.stringify(results,null,2));
    await require("./caption-polish-check.cjs")();
  } finally {
    await browser.close();
  }
})().catch((e) => {
  console.error(e);
  process.exitCode = 1;
});
