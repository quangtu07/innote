"""CJK resume across multiple 20 s windows with the real ASR worker.

The new exe runs the same window/look-back/commit loop inside Rust and returns
both the committed segments and each window's raw worker segments. The 0.5.1
commit rule is then replayed on those SAME raw segments, isolating dedupe from
recognition differences.

Public fixtures only; no network, no API key, no user recording.
"""
import argparse
import json
import pathlib
import subprocess
import wave

p = argparse.ArgumentParser()
p.add_argument('--after', required=True, help='Build that implements --resume-test')
p.add_argument('--model', required=True)
p.add_argument('--fixture', default='test-results/fixtures/multilingual/cjk-resume-public.wav')
p.add_argument('--output', required=True)
a = p.parse_args()
out = pathlib.Path(a.output).resolve()
out.mkdir(parents=True, exist_ok=True)
fixture = pathlib.Path(a.fixture).resolve()
model = str(pathlib.Path(a.model).resolve())

with wave.open(str(fixture), 'rb') as w:
    assert (w.getnchannels(), w.getsampwidth(), w.getframerate()) == (1, 2, 16000)
    duration = w.getnframes() / 16000
    frames = w.readframes(w.getnframes())
pcm_dir = out / 'pcm'
pcm_dir.mkdir(exist_ok=True)
(pcm_dir / 'system.pcm').write_bytes(frames)
windows = []
processed = 0.0
while processed < duration - 0.00001:
    windows.append((max(processed - 1.5, 0.0), min(processed + 20.0, duration)))
    processed = min(processed + 20.0, duration)

wav_dir = out / 'windows'
wav_dir.mkdir(exist_ok=True)
jobs = []
for i, (start, end) in enumerate(windows):
    path = wav_dir / f'w{i}.wav'
    with wave.open(str(path), 'wb') as w:
        w.setparams((1, 2, 16000, 0, 'NONE', 'not compressed'))
        w.writeframes(frames[int(start * 32000):int(end * 32000)])
    jobs.append({'path': str(path), 'offset': start, 'language': 'multi'})


def normalize(text):
    return ''.join(c.lower() for c in text if c.isalnum())


def old_commit(segments, incoming, end):
    """Exact 0.5.1 rule: whitespace tokens, min 3, previous segment not edited."""
    watermark = segments[-1]['watermark'] if segments else 0.0
    for x in incoming:
        if x['end'] <= watermark + 0.00001:
            continue
        if x['start'] < watermark and watermark > 0 and segments:
            previous = segments[-1]['segment']
            if not previous.get('edited', False) and previous['end'] >= watermark - 1.5:
                left = previous['text'].split()
                right = x['text'].split()
                for n in range(min(len(left), len(right) - 1, 12), 2, -1):
                    if [normalize(t) for t in left[-n:]] == [normalize(t) for t in right[:n]]:
                        x['text'] = ' '.join(right[n:])
                        break
        x['start'] = max(x['start'], watermark)
        x['end'] = min(x['end'], end)
        if x['end'] > x['start'] and x['text'].strip():
            segments.append({'segment': x, 'watermark': end})
        elif segments:
            segments[-1]['watermark'] = end
        else:
            segments.append({'segment': {'start': watermark, 'end': watermark, 'text': '', 'edited': False}, 'watermark': end})
    return end


def boundary_duplicate(previous, following):
    """Longest common suffix/prefix in normalized letters, as a real repeat risk."""
    left = normalize(previous)
    right = normalize(following)
    best = 0
    for n in range(min(len(left), len(right)), 3, -1):
        if left[-n:] == right[:n]:
            best = n
            break
    return best


def duplicate_report(segments):
    duplicates = []
    for previous, following in zip(segments, segments[1:]):
        if following['start'] - previous['end'] > 1.5 + 0.00001:
            continue
        overlap = boundary_duplicate(previous['text'], following['text'])
        if overlap >= 4:
            duplicates.append({'previous': previous['text'], 'following': following['text'],
                               'overlap_chars': overlap, 'gap_s': following['start'] - previous['end']})
    return duplicates
# New build: the real resume loop inside the exe (one worker process per window).
proc = subprocess.run([str(pathlib.Path(a.after).resolve()), '--resume-test', str(pcm_dir), model, 'multi', 'GIU-NGUYEN'],
    capture_output=True, timeout=300, creationflags=subprocess.CREATE_NO_WINDOW)
(out / 'after-stderr.txt').write_bytes(proc.stderr)
assert proc.returncode == 0, proc.stderr.decode('utf-8', errors='replace')[-1000:]
after = json.loads(proc.stdout.decode('utf-8'))
assert 'Ok' in after, after
new_segments = after['Ok']['segments']
raw_windows = after['Ok']['raw_segments']
assert len(raw_windows) == len(windows), (len(raw_windows), len(windows))

# Apply the 0.5.1 commit rule to the SAME raw worker segments. This isolates the
# dedupe change from any recognition difference between runs/builds.
old = []
for incoming, (_, end) in zip(raw_windows, windows):
    old_commit(old, incoming, end)
old_segments = [row['segment'] for row in old]

before_duplicates = duplicate_report(old_segments)
after_duplicates = duplicate_report(new_segments)
assert not after_duplicates, after_duplicates
assert after['Ok']['processed'] == duration
assert after['Ok']['windows'] == len(windows), after['Ok']
assert after['Ok']['edited_preserved'] and new_segments[0]['edited'], 'Edited text was not preserved'
assert new_segments[0]['text'] == 'GIU-NGUYEN'
assert all(s['end'] > s['start'] for s in new_segments)
assert all(new_segments[i]['start'] <= new_segments[i + 1]['start'] for i in range(len(new_segments) - 1))
report = {
    'fixture': str(fixture),
    'duration_s': duration,
    'windows': [{'start': s, 'end': e} for s, e in windows],
    'before': {'segments': len(old_segments), 'duplicates': before_duplicates,
               'text': ' '.join(s['text'] for s in old_segments)},
    'after': {'segments': len(new_segments), 'duplicates': after_duplicates,
              'text': ' '.join(s['text'] for s in new_segments)},
    'checks': {'no_adjacent_cjk_duplicate': not after_duplicates,
               'edited_text_preserved': True,
               'watermark_complete': after['Ok']['processed'] == duration,
               'same_public_fixture_for_both': True},
    'limitation': 'Core resume loop, not GUI clicks; Thai/Han repair on this new clip still fails and is reported in the accuracy panel.',
}
(out / 'results.json').write_text(json.dumps(report, ensure_ascii=False, indent=2), encoding='utf-8')
print(json.dumps({k: v for k, v in report.items() if k not in ('before', 'after')} | {
    'before_duplicates': len(before_duplicates), 'after_segments': len(new_segments)}, ensure_ascii=False))