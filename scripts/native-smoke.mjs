// Opt-in Windows integration check. Records local audio; never uploads it.
import assert from 'node:assert/strict';
import { spawnSync } from 'node:child_process';
import { createHash } from 'node:crypto';
import { mkdirSync, readFileSync, writeFileSync } from 'node:fs';
import { dirname, join, resolve } from 'node:path';
import { fileURLToPath } from 'node:url';
import { parseArgs } from 'node:util';

const root = resolve(dirname(fileURLToPath(import.meta.url)), '..');
const { values } = parseArgs({ options: {
  exe: { type: 'string', default: 'src-tauri/target/release/innote.exe' },
  seconds: { type: 'string', default: '5' },
  model: { type: 'string' },
  output: { type: 'string' },
} });
const seconds = Number(values.seconds);
assert(Number.isFinite(seconds) && seconds >= 0.1 && seconds <= 3600, 'Invalid --seconds');
const exe = resolve(root, values.exe);
const output = resolve(root, values.output ?? `test-results/native-smoke-${Date.now()}`);
mkdirSync(output, { recursive: false });
const checks = [];

function run(args, success, timeout = 30_000) {
  const result = spawnSync(exe, args, {
    cwd: root, encoding: 'utf8', windowsHide: true, timeout,
  });
  if (result.error) throw result.error;
  assert.equal(result.signal, null, `Process terminated by ${result.signal}`);
  const message = JSON.parse(result.stdout.trim());
  if (success) {
    assert.equal(result.status, 0, JSON.stringify(message));
    assert('Ok' in message, JSON.stringify(message));
    assert(!message.Ok?.error, JSON.stringify(message));
    return message.Ok;
  }
  assert.notEqual(result.status, 0, 'Error must produce a nonzero exit code');
  assert(typeof message.Err === 'string' && message.Err.length > 0, JSON.stringify(message));
  return message.Err;
}

function check(name, test) {
  test();
  checks.push({ name, passed: true });
  console.log(`PASS ${name}`);
}

function digest(path) {
  return createHash('sha256').update(readFileSync(path)).digest('hex');
}

try {
  check('Default system and microphone endpoints exist', () => {
    const devices = run(['--devices'], true);
    for (const kind of ['system', 'microphone']) {
      assert(devices.some(d => d.kind === kind && d.default), `Missing ${kind}`);
    }
  });
  check('Malformed CLI arguments return JSON errors without panic', () => {
    run(['--capture-test'], false);
    run(['--transcribe-test'], false);
    for (const duration of ['-1', 'NaN', 'inf', '0', '3601', 'invalid']) {
      run(['--capture-test', join(output, 'invalid'), duration, 'both'], false);
    }
    run(['--capture-test', join(output, 'invalid'), '1', 'invalid'], false);
  });
  const capture = join(output, 'capture');
  let report;
  check('Capture both sources and finalize a mono 16 kHz PCM16 WAV', () => {
    report = run(['--capture-test', capture, String(seconds), 'both'], true,
      Math.max(30_000, (seconds + 15) * 1000));
    writeFileSync(join(output, 'capture.json'), JSON.stringify(report, null, 2));
    assert.equal(report.sources.length, 2);
    assert(Math.abs(report.duration - seconds) < 1, 'Unexpected capture duration');
    for (const kind of ['system', 'microphone']) {
      const source = report.sources.find(s => s.kind === kind);
      assert(source, `Missing ${kind} report`);
      const pcm = readFileSync(join(capture, `${kind}.pcm`));
      assert.equal(pcm.length, source.bytes);
      assert.equal(pcm.length % 2, 0, 'Truncated PCM16 sample');
      assert(Number.isFinite(source.peak_rms) && source.peak_rms >= 0);
    }
    const wav = readFileSync(join(capture, 'recording.wav'));
    assert.equal(wav.toString('ascii', 0, 4), 'RIFF');
    assert.equal(wav.toString('ascii', 8, 12), 'WAVE');
    assert.equal(wav.readUInt32LE(4), wav.length - 8);
    let formatFound = false;
    let audioBytes = 0;
    for (let offset = 12; offset + 8 <= wav.length;) {
      const tag = wav.toString('ascii', offset, offset + 4);
      const size = wav.readUInt32LE(offset + 4);
      assert(offset + 8 + size <= wav.length, 'Truncated WAV chunk');
      if (tag === 'fmt ') {
        assert(size >= 16);
        assert.equal(wav.readUInt16LE(offset + 8), 1);
        assert.equal(wav.readUInt16LE(offset + 10), 1);
        assert.equal(wav.readUInt32LE(offset + 12), 16000);
        assert.equal(wav.readUInt16LE(offset + 22), 16);
        formatFound = true;
      }
      if (tag === 'data') audioBytes += size;
      offset += 8 + size + (size % 2);
    }
    assert(formatFound);
    assert(Math.abs(audioBytes / 32000 - report.duration) < 0.001);
  });
  check('A repeated capture cannot alter existing audio', () => {
    const paths = ['system.pcm', 'microphone.pcm', 'recording.wav'].map(p => join(capture, p));
    const before = paths.map(digest);
    run(['--capture-test', capture, '1', 'both'], false);
    assert.deepEqual(paths.map(digest), before);
  });
  if (values.model) {
    check('Worker transcribes the public Vietnamese fixture', () => {
      const result = run(['--transcribe-test', resolve(root, values.model),
        join(root, 'test-results/fixtures/vietnamese-pcm.wav'), 'vi'], true, 180_000);
      assert(result.segments.some(s => s.text.trim()), 'Empty transcript');
      for (const segment of result.segments) {
        assert(Number.isFinite(segment.start) && Number.isFinite(segment.end));
        assert(segment.end > segment.start && segment.start >= 0);
      }
      writeFileSync(join(output, 'vietnamese.json'), JSON.stringify(result, null, 2));
    });
  }
  console.log(`Results: ${output}`);
} catch (error) {
  checks.push({ passed: false, error: String(error) });
  console.error(error);
  process.exitCode = 1;
} finally {
  writeFileSync(join(output, 'checks.json'), JSON.stringify(checks, null, 2));
}
