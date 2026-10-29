"""Replay explicitly supplied local PCM through the real file-tail worker.
No playback, network, or private-audio upload. Intended for opt-in capture QA.
"""
import argparse,json,os,re,subprocess,time
from pathlib import Path
p=argparse.ArgumentParser();p.add_argument('--exe',required=True);p.add_argument('--before')
p.add_argument('--pcm',required=True);p.add_argument('--data-dir',required=True)
p.add_argument('--seconds',type=float,default=80);p.add_argument('--output',required=True)
a=p.parse_args();out=Path(a.output).resolve();out.mkdir(parents=True,exist_ok=True)
assert 0<a.seconds<=3600
with Path(a.pcm).open('rb') as f:pcm=f.read(round(a.seconds*16000)*2)
assert len(pcm)%2==0 and pcm
reports=[]
for label,exe in [('before',a.before),('after',a.exe)]:
    if not exe:continue
    directory=out/label;directory.mkdir(exist_ok=True);(directory/'system.pcm').write_bytes(pcm)
    proc=subprocess.Popen([str(Path(exe).resolve()),'--live-worker','multi',str(directory)],
        stdin=subprocess.PIPE,stdout=subprocess.PIPE,stderr=subprocess.PIPE,
        encoding='utf-8',creationflags=subprocess.CREATE_NO_WINDOW,
        env={**os.environ,'MAY_DATA_DIR':str(Path(a.data_dir).resolve()),'MAY_LATENCY_AUDIT':'1'})
    ready=proc.stdout.readline();assert json.loads(ready)=={'ready':True},ready
    start=time.perf_counter();proc.stdin.close();proc.stdin=None
    stdout,stderr=proc.communicate(timeout=120)
    (directory/'stderr.txt').write_text(stderr,encoding='utf-8')
    (directory/'updates.jsonl').write_text(stdout,encoding='utf-8')
    assert proc.returncode==0,stderr
    updates=[json.loads(line) for line in stdout.splitlines() if line.strip()]
    words=[w for u in updates if u['final_text'] for w in u['words']]
    text=' '.join(w['text'] for w in words)
    repeated=bool(re.search(r'([\u0e00-\u0e7f])\1+',text))
    if label=='after':
        assert not repeated,text
        assert re.search('[가-힣]',text) and 'engineering' in text.lower() and 'sản phẩm' in text,text
        assert text.count('因为')>=2,text
        assert all(0<=w['start']<w['end']<=len(pcm)/32000+.1 for w in words),words
    report={'build':label,'passed':label=='before' or not repeated,'repeated_thai':repeated,
            'duration_s':len(pcm)/32000,'unpaced_processing_s':time.perf_counter()-start,'text':text}
    reports.append(report);print(json.dumps(report,ensure_ascii=False),flush=True)
    (out/'results.json').write_text(json.dumps(reports,ensure_ascii=False,indent=2),encoding='utf-8')
