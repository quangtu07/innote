// Opt-in local-speaker capture test. No microphone, API key, or cloud upload.
const { chromium } = require(process.env.PLAYWRIGHT_MODULE || "playwright");
const { spawn } = require("node:child_process");
const fs = require("node:fs"),
  path = require("node:path");
(async () => {
  const browser = await chromium.launch({
    channel: "chrome",
    headless: false,
    args: ["--autoplay-policy=no-user-gesture-required"],
  });
  const page = await browser.newPage();
  const output = path.resolve("test-results/caption-youtube-" + Date.now());
  fs.mkdirSync(output, { recursive: true });
  let report;
  try {
    await page.goto("https://www.youtube.com/watch?v=UF8uR6Z6KLc", {
      waitUntil: "domcontentloaded",
      timeout: 30000,
    });
    await page.locator("video").waitFor({ timeout: 20000 });
    await page.evaluate(async () => {
      const v = document.querySelector("video");
      v.muted = false;
      v.volume = 0.5;
      v.currentTime = 30;
      await v.play();
    });
    await page.waitForFunction(
      () => {
        const v = document.querySelector("video");
        return !v.paused && v.currentTime > 32;
      },
      { timeout: 15000 },
    );
    const before = await page.evaluate(() => {
      const v = document.querySelector("video");
      return { time: v.currentTime, muted: v.muted, volume: v.volume };
    });
    const audio = await new Promise((resolve, reject) => {
      const p = spawn(
        path.resolve("src-tauri/target/debug/innote.exe"),
        ["--capture-test", path.join(output, "system"), "8", "system"],
        { windowsHide: true },
      );
      let stdout = "",
        stderr = "";
      p.stdout.on("data", (b) => (stdout += b));
      p.stderr.on("data", (b) => (stderr += b));
      p.on("error", reject);
      p.on("close", (code) =>
        code
          ? reject(new Error(stderr || stdout))
          : resolve(JSON.parse(stdout)),
      );
    });
    const after = await page.evaluate(() => ({
      time: document.querySelector("video").currentTime,
      title: document.title,
    }));
    await page.screenshot({ path: path.join(output, "youtube.png") });
    report = {
      url: page.url(),
      before,
      after,
      audio,
      cloudTranslationTested: false,
    };
  } catch (e) {
    report = { passed: false, error: String(e), cloudTranslationTested: false };
  } finally {
    fs.writeFileSync(
      path.join(output, "report.json"),
      JSON.stringify(report, null, 2),
    );
    console.log(JSON.stringify(report, null, 2));
    await browser.close();
  }
})().catch((e) => {
  console.error(e);
  process.exitCode = 1;
});
