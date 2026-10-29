"""Character error rate on fresh public FLEURS rows (offset 3-4, not the old panel).

Uses the real native --asr-worker, local only. Reports per-clip and composite
CER plus any repeated-Thai anomaly; it does not hide recognition errors.
"""
import argparse
import json
import pathlib
import re
import subprocess
import unicodedata
import wave

p = argparse.ArgumentParser()
p.add_argument('--exe', required=True)
p.add_argument('--model', required=True)
p.add_argument('--fixtures', default='test-results/fixtures/fleurs-independent')
p.add_argument('--output', required=True)
p.add_argument('--before', help='Optional baseline exe for the same jobs')
a = p.parse_args()
out = pathlib.Path(a.output).resolve()
out.mkdir(parents=True, exist_ok=True)
fixtures = pathlib.Path(a.fixtures).resolve()
metadata = json.loads((fixtures / 'metadata.json').read_text(encoding='utf-8'))


def normalize(s):
    return ''.join(c for c in unicodedata.normalize('NFC', s.lower()) if c.isalnum())


def distance(a, b):
    row = list(range(len(b) + 1))
    for i, x in enumerate(a, 1):
        new = [i]
        for j, y in enumerate(b, 1):
            new.append(min(new[-1] + 1, row[j] + 1, row[j - 1] + (x != y)))
        row = new
    return row[-1]


def repeated_thai(text):
    previous, run = '', 0
    for c in text:
        run = run + 1 if c == previous else 1
        previous = c
        if run >= 2 and '\u0e00' <= c <= '\u0e7f':
            return True
    return False


def pcm(name):
    with wave.open(str(fixtures / (name + '.wav')), 'rb') as w:
        assert (w.getnchannels(), w.getsampwidth(), w.getframerate()) == (1, 2, 16000)
        return w.readframes(w.getnframes())


silence = bytes(16000)
composite = b''
for item in metadata:
    composite += pcm(item['name']) + silence
composite_path = out / 'independent-composite.wav'
with wave.open(str(composite_path), 'wb') as w:
    w.setparams((1, 2, 16000, 0, 'NONE', 'not compressed'))
    w.writeframes(composite)
composite_ref = ' '.join(item['reference'] for item in metadata)

jobs = [{'path': str((fixtures / (item['name'] + '.wav')).resolve()), 'language': 'multi', 'offset': 0}
        for item in metadata]
jobs.append({'path': str(composite_path), 'language': 'multi', 'offset': 0})


def run(label, exe):
    proc = subprocess.run([str(pathlib.Path(exe).resolve()), '--asr-worker', str(pathlib.Path(a.model).resolve())],
        input=''.join(json.dumps(j) + '\n' for j in jobs), capture_output=True, encoding='utf-8',
        timeout=600, creationflags=subprocess.CREATE_NO_WINDOW)
    (out / f'{label}-stderr.txt').write_text(proc.stderr, encoding='utf-8')
    assert proc.returncode == 0, proc.stderr[-2000:]
    responses = [json.loads(line) for line in proc.stdout.splitlines() if line.strip()]
    assert len(responses) == len(jobs), (len(responses), len(jobs))
    rows = []
    for item, response in zip(metadata, responses):
        assert not response['error'], response
        text = ' '.join(s['text'] for s in response['segments'])
        ref = normalize(item['reference'])
        rows.append({
            'name': item['name'], 'language': item['config'], 'reference_characters': len(ref),
            'edits': distance(ref, normalize(text)), 'cer': distance(ref, normalize(text)) / len(ref),
            'text': text, 'reference': item['reference'], 'elapsed_ms': response['elapsed_ms'],
            'repeated_thai': repeated_thai(text),
            'script_ok': bool(re.search(r'[가-힣]', text)) if item['config'] == 'ko_kr'
                else bool(re.search(r'[\u4e00-\u9fff]', text)) if item['config'] == 'cmn_hans_cn'
                else bool(re.search(r'[A-Za-zÀ-ỹ]', text)),
        })
    composite_response = responses[-1]
    assert not composite_response['error'], composite_response
    composite_text = ' '.join(s['text'] for s in composite_response['segments'])
    ref = normalize(composite_ref)
    composite_row = {
        'reference_characters': len(ref), 'edits': distance(ref, normalize(composite_text)),
        'cer': distance(ref, normalize(composite_text)) / len(ref), 'text': composite_text,
        'elapsed_ms': composite_response['elapsed_ms'], 'repeated_thai': repeated_thai(composite_text),
        'has_all_scripts': bool(re.search(r'[가-힣]', composite_text))
            and bool(re.search(r'[\u4e00-\u9fff]', composite_text))
            and bool(re.search(r'[A-Za-zÀ-ỹ]', composite_text)),
    }
    return {'build': label, 'clips': rows, 'composite': composite_row,
            'total_edits': sum(r['edits'] for r in rows) + composite_row['edits'],
            'total_characters': sum(r['reference_characters'] for r in rows) + composite_row['reference_characters']}


reports = [run('after', a.exe)]
if a.before:
    reports.insert(0, run('before', a.before))
for report in reports:
    total = report['total_edits'] / report['total_characters'] if report['total_characters'] else None
    report['total_cer'] = total
    report['clip_cer_mean'] = sum(r['cer'] for r in report['clips']) / len(report['clips'])
(out / 'results.json').write_text(json.dumps({'dataset': 'google/fleurs validation offsets 3,4; CC BY 4.0',
    'independence': 'Rows 0 and 1 were used by the original/held-out panel; these are different rows.',
    'reports': reports}, ensure_ascii=False, indent=2), encoding='utf-8')
print(json.dumps({r['build']: {'total_cer': r['total_cer'], 'clip_cer_mean': r['clip_cer_mean'],
    'composite_cer': r['composite']['cer'], 'repeated_thai': sum(x['repeated_thai'] for x in r['clips']) + r['composite']['repeated_thai']}
    for r in reports}, ensure_ascii=False))