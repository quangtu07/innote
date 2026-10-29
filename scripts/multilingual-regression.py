"""Opt-in native multilingual batch/recovery and hard-switch fixture checks.

Uses public fixtures only; no network or API. Run after streaming-regression.py.
"""
import argparse
import json
import re
import subprocess
import wave
from pathlib import Path

p = argparse.ArgumentParser()
p.add_argument('--exe', required=True)
p.add_argument('--model', required=True)
p.add_argument('--output', required=True)
a = p.parse_args()
out = Path(a.output).resolve()
out.mkdir(parents=True, exist_ok=True)
fixtures = Path('test-results/fixtures').resolve()
results = []

def check_text(text):
    assert 'engineering' in text.lower() and 'loài' in text, text
    assert re.search('[가-힣]', text) and re.search('[一-鿿]', text), text

jobs = [
    {'path': str(fixtures/'missing.wav'), 'language': 'multi', 'offset': 0},
    {'path': str(fixtures/'multilingual/four-languages.wav'), 'language': 'multi', 'offset': 7.5},
    {'path': str(fixtures/'multilingual/ko_kr.wav'), 'language': 'ko', 'offset': 80},
    {'path': str(fixtures/'multilingual/cmn_hans_cn.wav'), 'language': 'zh', 'offset': 100},
]
proc = subprocess.run([str(Path(a.exe).resolve()), '--asr-worker', str(Path(a.model).resolve())],
    input=''.join(json.dumps(j, ensure_ascii=False)+'\n' for j in jobs), text=True,
    encoding='utf-8', capture_output=True, timeout=240,
    creationflags=subprocess.CREATE_NO_WINDOW)
(out/'stderr.txt').write_text(proc.stderr, encoding='utf-8')
assert proc.returncode == 0, proc.stderr
responses = [json.loads(line) for line in proc.stdout.splitlines() if line.strip()]
(out/'responses.json').write_text(json.dumps(responses, ensure_ascii=False, indent=2), encoding='utf-8')
assert len(responses) == len(jobs), responses
assert responses[0]['error'], responses[0]
results.append({'case': 'invalid-job-recovery', 'passed': True})
for i, pattern in [(1, None), (2, '[가-힣]'), (3, '[一-鿿]')]:
    r = responses[i]
    assert not r['error'], r
    text = ' '.join(s['text'] for s in r['segments'])
    if pattern:
        assert re.search(pattern, text), text
    else:
        check_text(text)
    duration = [0, 57.4975, 10.56, 18.38][i]
    assert all(jobs[i]['offset'] <= s['start'] < s['end'] <= jobs[i]['offset']+duration+.1
               for s in r['segments']), r
    results.append({'case': ['','mixed-batch-offset','fixed-korean','fixed-mandarin'][i],
                    'passed': True, 'text': text, 'elapsed_ms': r['elapsed_ms']})
(out/'results.json').write_text(json.dumps(results, ensure_ascii=False, indent=2), encoding='utf-8')
print(json.dumps(results, ensure_ascii=False))

# Remove only the added .8 s gaps. Original utterance silences are retained.
meta = json.loads((fixtures/'multilingual/four-languages.json').read_text(encoding='utf-8'))
audio = b''
for turn in meta['turns']:
    with wave.open(turn['file'], 'rb') as wav:
        assert wav.getparams()[:3] == (1, 2, 16000)
        audio += wav.readframes(wav.getnframes())
hard = out/'four-no-added-gap.wav'
with wave.open(str(hard), 'wb') as wav:
    wav.setparams((1, 2, 16000, 0, 'NONE', 'not compressed'))
    wav.writeframes(audio)
proc = subprocess.run([str(Path(a.exe).resolve()), '--transcribe-test',
    str(Path(a.model).resolve()), str(hard), 'multi'], capture_output=True,
    encoding='utf-8', timeout=180, creationflags=subprocess.CREATE_NO_WINDOW)
(out/'no-added-gap.json').write_text(proc.stdout, encoding='utf-8')
assert proc.returncode == 0, proc.stderr
r = json.loads(proc.stdout)['Ok']
text = ' '.join(s['text'] for s in r['segments'])
try:
    check_text(text)
    coverage = True
except AssertionError:
    coverage = False
results.append({'case': 'no-added-gap-observation', 'all_four_scripts_retained': coverage,
    'text': text, 'limitation': 'Public utterances concatenated; not same-speaker mid-sentence code-switch or overlapping speech'})
(out/'results.json').write_text(json.dumps(results, ensure_ascii=False, indent=2), encoding='utf-8')
print(json.dumps(results[-1], ensure_ascii=False))
