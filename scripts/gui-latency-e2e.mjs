// End-to-end word-end -> DOM render for the real Windows app.
// Launches the packaged/debug exe with WebView2 CDP, selects system loopback,
// plays a public WAV, and reads live-paint-audit.jsonl (MAY_LATENCY_AUDIT=1).
import { spawn } from 'node:child_process';
import { existsSync, linkSync, mkdirSync, readdirSync, readFileSync, statSync, writeFileSync } from 'node:fs';
import { join, resolve } from 'node:path';
import { pathToFileURL } from 'node:url';
const modulePath = process.env.PLAYWRIGHT_MODULE;
const { chromium } = await import(modulePath ? pathToFileURL(modulePath).href : 'playwright');

const args = Object.fromEntries(process.argv.slice(2).reduce((all, item, i, list) => {
  if (item.startsWith('--')) all.push([item.slice(2), list[i + 1] && !list[i + 1].startsWith('--') ? list[i + 1] : 'true']);
  return all;
}, []));
for (const key of ['exe', 'data', 'audio', 'output', 'model']) {
  if (!args[key]) throw new Error(`Missing --${key}`);
}
const exe = resolve(args.exe);
const data = resolve(args.data);
const audio = resolve(args.audio);
const output = resolve(args.output);
const modelSource = resolve(args.model);
const port = args.port ?? '9222';
mkdirSync(output, { recursive: true });
mkdirSync(join(data, 'models'), { recursive: true });
mkdirSync(join(data, 'sessions'), { recursive: true });
const modelTarget = join(data, 'models', 'nemotron-3.5-q8.gguf');
if (!existsSync(modelTarget)) linkSync(modelSource, modelTarget);

const app = spawn(exe, [], {
  env: { ...process.env, MAY_DATA_DIR: data, MAY_LATENCY_AUDIT: '1', WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS: `--remote-debugging-port=${port}` },
  stdio: ['ignore', 'pipe', 'pipe'],
});
let appLog = '';
app.stdout.on('data', (x) => { appLog += x.toString('utf8'); });
app.stderr.on('data', (x) => { appLog += x.toString('utf8'); });
const sleep = (ms) => new Promise((r) => setTimeout(r, ms));
async function waitFor(fn, timeout, label) {
  const until = Date.now() + timeout;
  while (Date.now() < until) {
    try { const value = await fn(); if (value) return value; } catch { /* retry */ }
    await sleep(250);
  }
  throw new Error(`Timeout: ${label}`);
}
let browser;
try {
  await waitFor(async () => {
    const response = await fetch(`http://127.0.0.1:${port}/json/version`);
    return response.ok;
  }, 30000, 'WebView2 CDP');
  browser = await chromium.connectOverCDP(`http://127.0.0.1:${port}`);
  const page = await waitFor(() => {
    const pages = browser.contexts().flatMap((c) => c.pages());
    return pages.find((p) => p.url().includes('tauri.localhost')) ?? pages[0];
  }, 15000, 'app page');
  await page.setViewportSize({ width: 1180, height: 800 });
  const source = page.getByLabel('Thiết bị phát âm thanh cuộc họp');
  await source.waitFor({ timeout: 20000 });
  const systemOptions = await source.locator('option').evaluateAll((options) => options.map((o) => ({ value: o.value, text: o.textContent })));
  const system = systemOptions.find((o) => o.value && !/ngắt|Không thu/i.test(o.text));
  if (!system) throw new Error('No system loopback device: ' + JSON.stringify(systemOptions));
  await source.selectOption(system.value);
  await page.getByLabel('Micro của bạn').selectOption('');
  await page.getByLabel('Ngôn ngữ', { exact: true }).selectOption('multi');
  const start = page.getByRole('button', { name: 'Bắt đầu ghi', exact: true });
  await start.waitFor({ timeout: 15000 });
  if (await start.isDisabled()) throw new Error('Start disabled: ' + (await page.locator('main').innerText()).slice(0, 1200));
  const startedAt = Date.now();
  await start.click();
  await page.getByRole('button', { name: 'Tạm dừng', exact: true }).waitFor({ timeout: 30000 });
  const recordStartMs = Date.now() - startedAt;
  const player = spawn('powershell.exe', ['-NoProfile', '-NonInteractive', '-Command',
    `$ErrorActionPreference='Stop'; (New-Object System.Media.SoundPlayer '${audio.replace(/'/g, "''")}').PlaySync()`],
    { stdio: ['ignore', 'ignore', 'pipe'] });
  let playerErr = '';
  player.stderr.on('data', (x) => { playerErr += x.toString('utf8'); });
  const playCode = await new Promise((r) => player.on('exit', r));
  const playbackMs = Date.now() - startedAt;
  if (playCode !== 0) throw new Error('Playback failed: ' + playerErr);
  await sleep(2500);
  await page.getByRole('button', { name: 'Kết thúc', exact: true }).click();
const session = await waitFor(() => {
    const dirs = readdirSync(join(data, 'sessions')).map((name) => join(data, 'sessions', name))
      .sort((a, b) => statSync(b).mtimeMs - statSync(a).mtimeMs);
    return dirs.find((dir) => existsSync(join(dir, 'recording.wav')) && statSync(join(dir, 'recording.wav')).mtimeMs >= startedAt);
  }, 60000, 'recording.wav');
  await waitFor(() => existsSync(join(session, 'live-paint-audit.jsonl')) && statSync(join(session, 'live-paint-audit.jsonl')).size > 0,
    30000, 'live-paint-audit.jsonl');
  const rows = readFileSync(join(session, 'live-paint-audit.jsonl'), 'utf8').trim().split(/\n+/).filter(Boolean).map((line) => JSON.parse(line));
  const usable = rows.filter((r) => Number.isFinite(r.word_end_ms) && r.text && r.wall_ms >= r.word_end_ms);
  const stat = (values) => {
    const v = values.filter(Number.isFinite).sort((a, b) => a - b);
    if (!v.length) return { count: 0 };
    const q = (p) => v[Math.min(v.length - 1, Math.floor((v.length - 1) * p))];
    return { count: v.length, p50_ms: +q(.5).toFixed(1), p95_ms: +q(.95).toFixed(1), max_ms: +v[v.length - 1].toFixed(1), under_500_fraction: +(v.filter((x) => x < 500).length / v.length).toFixed(4) };
  };
  const scriptOf = (text) => /[\u4e00-\u9fff]/.test(text) ? 'Han' : /[\uac00-\ud7af]/.test(text) ? 'Hangul' : /[A-Za-zÀ-ỹ]/.test(text) ? 'Latin' : 'other';
  const report = {
    measurement: 'capture epoch -> native word end -> worker stdout -> Rust IPC -> Svelte two rAF -> report_live_paint; real GUI, public audio, local only',
    exe, audio, system_device: system.text, session, record_start_ms: recordStartMs, playback_end_ms: playbackMs,
    session_duration_s: statSync(join(session, 'system.pcm')).size / 32000,
    audit_rows: rows.length, usable_rows: usable.length,
    word_end_to_render: stat(usable.map((r) => r.wall_ms - r.word_end_ms)),
    word_end_to_emit: stat(usable.map((r) => r.emitted_ms - r.word_end_ms)),
    emit_to_receive: stat(usable.map((r) => r.received_ms - r.emitted_ms)),
    receive_to_render: stat(usable.map((r) => r.wall_ms - r.received_ms)),
    final_word_end_to_render: stat(usable.filter((r) => r.final).map((r) => r.wall_ms - r.word_end_ms)),
    interim_word_end_to_render: stat(usable.filter((r) => !r.final).map((r) => r.wall_ms - r.word_end_ms)),
    first_text_ms: usable.length ? Math.min(...usable.map((r) => r.wall_ms)) : null,
    scripts: [...new Set(usable.map((r) => scriptOf(r.text)))],
    samples: usable.map((r) => ({ text: r.text.slice(0, 120), word_end_ms: r.word_end_ms, emitted_ms: r.emitted_ms, received_ms: r.received_ms, wall_ms: r.wall_ms, final: r.final, word_end_to_render_ms: +(r.wall_ms - r.word_end_ms).toFixed(1) })),
    limitation: 'Public fixture playback through the real default output; silence/gaps and this machine only. Native words without word times are absent; no human word alignment.',
    app_log_tail: appLog.slice(-2000),
  };
  writeFileSync(join(output, 'results.json'), JSON.stringify(report, null, 2), 'utf8');
  console.log(JSON.stringify({ word_end_to_render: report.word_end_to_render, word_end_to_emit: report.word_end_to_emit, emit_to_receive: report.emit_to_receive, receive_to_render: report.receive_to_render, usable_rows: report.usable_rows, first_text_ms: report.first_text_ms, scripts: report.scripts }, null, 2));
} finally {
  try { await browser?.close(); } catch { /* app may already be closing */ }
  await sleep(1000);
  if (!app.killed) app.kill();
}