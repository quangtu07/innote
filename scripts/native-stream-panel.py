"""Sequential paced public-fixture tests against the packaged native decoder.
No Python ML dependency and no user recordings. Row clocks start after model load.
"""
import argparse, json, subprocess
from pathlib import Path
from benchmark import normalize, distance
p=argparse.ArgumentParser()
p.add_argument('--exe',required=True);p.add_argument('--output',required=True)
a=p.parse_args();out=Path(a.output);out.mkdir(parents=True,exist_ok=False)
fixtures=Path('test-results/fixtures');reports=[]
for name,ref,lang in [('vietnamese-pcm','vietnamese','vi'),('fleurs-vi-1','fleurs-vi-1','vi'),('fleurs-vi-2','fleurs-vi-2','vi'),('fleurs-vi-3','fleurs-vi-3','vi'),('english','english','en')]:
    run=subprocess.run([str(Path(a.exe).resolve()),'--stream-test',lang,str((fixtures/(name+'.wav')).resolve())],capture_output=True,timeout=90,creationflags=subprocess.CREATE_NO_WINDOW)
    assert run.returncode==0,run.stderr.decode('utf-8',errors='replace')[-1000:]
    (out/(name+'.jsonl')).write_bytes(run.stdout)
    committed=[];rows=[]
    for raw in run.stdout.splitlines():
        event=json.loads(raw);u=event['update']
        if u['final_text']:
            committed.extend(w['text'] for w in u['words']);partial=''
        else:partial=u['text']
        rows.append({'wall_ms':event['wall_ms'],'audio_ms':event['audio_ms'],'text':' '.join(committed+([partial] if partial else [])),'final':u['final_text']})
    hypothesis=rows[-1]['text'];reference=normalize((fixtures/(ref+'.txt')).read_text(encoding='utf-8-sig'))
    nonempty=[r for r in rows if r['text']]
    summary={'engine':'native packaged Moonshine','transcript':hypothesis,'reference_words':len(reference),'edits':distance(reference,normalize(hypothesis)),'wer':distance(reference,normalize(hypothesis))/len(reference),'first_text_ms':nonempty[0]['wall_ms'],'max_emit_backlog_ms':max(r['wall_ms']-r['audio_ms'] for r in rows)}
    (out/(name+'.json')).write_text(json.dumps({'summary':summary,'rows':rows},ensure_ascii=False,indent=2),encoding='utf-8')
    reports.append({'case':name,**summary});print(json.dumps({k:v for k,v in reports[-1].items() if k!='transcript'}),flush=True)
(out/'results.json').write_text(json.dumps(reports,ensure_ascii=False,indent=2),encoding='utf-8')
