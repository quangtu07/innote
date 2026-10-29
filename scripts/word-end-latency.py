"""Real word-end -> worker-emit latency on paced public audio.

The anchor is the native decoder's last word-end (`speech_end`), not the fed frame
cursor or a candidate count. Requires MAY_LATENCY_AUDIT=1 for wall timestamps.
Still not DOM render or human ground truth; those are measured separately.
"""
import argparse
import json
import os
import pathlib
import queue
import subprocess
import threading
import time
import wave

p = argparse.ArgumentParser()
p.add_argument('--exe', required=True)
p.add_argument('--output', required=True)
p.add_argument('--case', action='append', default=[])
a = p.parse_args()
out = pathlib.Path(a.output).resolve()
out.mkdir(parents=True, exist_ok=True)
fixtures = pathlib.Path('test-results/fixtures').resolve()
cases = a.case or [
    f'multi:{fixtures / "multilingual/four-languages.wav"}',
    f'multi:{fixtures / "multilingual/cjk-resume-public.wav"}',
]


def stats(values):
    values = sorted(v for v in values if v is not None)
    if not values:
        return {'count': 0}
    pick = lambda q: values[min(len(values) - 1, int((len(values) - 1) * q))]
    return {'count': len(values), 'p50_ms': round(pick(.5), 1), 'p95_ms': round(pick(.95), 1),
            'max_ms': round(values[-1], 1),
            'under_500_fraction': round(sum(v < 500 for v in values) / len(values), 4)}


def run(language, path):
    directory = out / path.stem
    directory.mkdir(exist_ok=True)
    with wave.open(str(path), 'rb') as w:
        assert (w.getnchannels(), w.getsampwidth(), w.getframerate()) == (1, 2, 16000)
        pcm = w.readframes(w.getnframes())
    env = dict(os.environ)
    env['MAY_LATENCY_AUDIT'] = '1'
    process = subprocess.Popen([str(pathlib.Path(a.exe).resolve()), '--live-worker', language, str(directory)],
        stdin=subprocess.PIPE, stdout=subprocess.PIPE, stderr=subprocess.DEVNULL,
        creationflags=subprocess.CREATE_NO_WINDOW, env=env)
    messages = queue.Queue()
    def receive():
        for line in process.stdout:
            messages.put((time.perf_counter(), json.loads(line)))
        messages.put((time.perf_counter(), None))
    threading.Thread(target=receive, daemon=True).start()
    _, ready = messages.get(timeout=60)
    assert ready == {'ready': True}, ready
    start_perf, start_unix_ms = time.perf_counter(), time.time() * 1000
    eof = [None]
    def feed():
        try:
            with (directory / 'system.pcm').open('wb', buffering=0) as file:
                for at in range(0, len(pcm), 640):
                    frame = pcm[at:at + 640]
                    time.sleep(max(0, start_perf + (at + len(frame)) / 32000 - time.perf_counter()))
                    file.write(frame)
        finally:
            eof[0] = time.perf_counter()
            process.stdin.close()
    feeder = threading.Thread(target=feed)
    feeder.start()
    rows, previous_end, scripts = [], None, []
    while True:
        when, update = messages.get(timeout=120)
        if update is None:
            break
        if not update.get('text'):
            continue
        speech_end = update.get('speech_end')
        if speech_end is None:
            continue
        if previous_end is not None and speech_end < previous_end + 0.15:
            continue
        previous_end = speech_end
        emit = update.get('worker_unix_ms')
        if emit is None:
            continue
        word_end_abs = start_unix_ms + speech_end * 1000
        text = update['text']
        if any('\u4e00' <= c <= '\u9fff' for c in text):
            script = 'Han'
        elif any('\uac00' <= c <= '\ud7af' for c in text):
            script = 'Hangul'
        elif any(c.isascii() and c.isalpha() for c in text):
            script = 'Latin'
        else:
            script = 'other'
        rows.append({'audio_ms': round(speech_end * 1000, 1), 'emit_ms': round(emit - start_unix_ms, 1),
                     'word_end_to_emit_ms': round(emit - word_end_abs, 1), 'final': update['final_text'],
                     'text': text, 'script': script, 'worker_wall_ms': round(update.get('worker_unix_ms', 0))})
        if not scripts or scripts[-1] != script:
            scripts.append(script)
    process.wait(timeout=10)
    feeder.join(timeout=5)
    assert process.returncode == 0, process.returncode
    # First text after the model is ready is a different metric from word latency.
    first_text = rows[0] if rows else None
    script_switch = [r for previous, r in zip(rows, rows[1:]) if previous['script'] != r['script']]
    return {'case': path.name, 'language': language, 'duration_s': len(pcm) / 32000,
            'ready_before_feed_ms': 0.0, 'first_text_voice_ms': first_text['emit_ms'] if first_text else None,
            'word_end_to_emit': stats([r['word_end_to_emit_ms'] for r in rows]),
            'final_word_end_to_emit': stats([r['word_end_to_emit_ms'] for r in rows if r['final']]),
            'interim_word_end_to_emit': stats([r['word_end_to_emit_ms'] for r in rows if not r['final']]),
            'script_switch_word_end_to_emit': stats([r['word_end_to_emit_ms'] for r in script_switch]),
            'observations': len(rows), 'rows': rows,
            'limitation': 'Native word-end anchor -> worker stdout on paced public audio; excludes IPC, DOM render, model load and human alignment.'}


reports = []
for case in cases:
    language, path = case.split(':', 1)
    report = run(language, pathlib.Path(path))
    reports.append(report)
    print(json.dumps({k: v for k, v in report.items() if k != 'rows'}, ensure_ascii=False), flush=True)
(out / 'results.json').write_text(json.dumps(reports, ensure_ascii=False, indent=2), encoding='utf-8')