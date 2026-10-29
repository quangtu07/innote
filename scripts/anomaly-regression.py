"""Opt-in native regression for observed Chinese/Thai repetition after a turn.
Requires public fixtures created by multilingual-quality.py; never uploads audio.
"""
import argparse,json,os,re,subprocess,wave
from pathlib import Path
p=argparse.ArgumentParser();p.add_argument('--exe',required=True);p.add_argument('--model',required=True);p.add_argument('--output',required=True)
a=p.parse_args();out=Path(a.output).resolve();out.mkdir(parents=True,exist_ok=True)
fixture=Path('test-results/fixtures/multilingual/cmn_hans_cn-heldout.wav').resolve()
with wave.open(str(fixture),'rb') as w:pcm=w.readframes(w.getnframes())
padded=out/'padded-chinese.wav'
with wave.open(str(padded),'wb') as w:
    w.setparams((1,2,16000,0,'NONE','not compressed'));w.writeframes(bytes(3200)+pcm+bytes(25600))
jobs=[{'path':str(path.resolve()),'language':lang,'offset':7.5} for path,lang in [
    (Path('test-results/quality-0.5.1/heldout-korean-first.wav'),'multi'),(padded,'zh'),(fixture,'zh')]]
env={**os.environ,'MAY_LATENCY_AUDIT':'1'}
proc=subprocess.run([str(Path(a.exe).resolve()),'--asr-worker',str(Path(a.model).resolve())],
    input=''.join(json.dumps(j)+'\n' for j in jobs),capture_output=True,encoding='utf-8',
    timeout=120,creationflags=subprocess.CREATE_NO_WINDOW,env=env)
(out/'stderr.txt').write_text(proc.stderr,encoding='utf-8')
assert proc.returncode==0,proc.stderr
rows=[json.loads(line) for line in proc.stdout.splitlines() if line.strip()]
(out/'responses.json').write_text(json.dumps(rows,ensure_ascii=False,indent=2),encoding='utf-8')
assert len(rows)==3,rows
results=[]
for i,(job,r) in enumerate(zip(jobs,rows)):
    assert not r['error'],r
    text=' '.join(s['text'] for s in r['segments'])
    assert not re.search(r'([\u0e00-\u0e7f])\1+',text),text
    assert '因为' in text and '动物' in text,text
    if i==0:assert re.search('[가-힣]',text) and 'sản phẩm' in text,text
    with wave.open(job['path'],'rb') as w:duration=w.getnframes()/16000
    assert all(7.5<=s['start']<s['end']<=7.5+duration+.1 for s in r['segments']),r
    result={'case':['mixed-korean-vietnamese-chinese','padded-fixed-chinese','next-job-after-repair'][i],
            'passed':True,'text':text,'elapsed_ms':r['elapsed_ms']}
    results.append(result);print(json.dumps(result,ensure_ascii=False),flush=True)
(out/'results.json').write_text(json.dumps(results,ensure_ascii=False,indent=2),encoding='utf-8')
