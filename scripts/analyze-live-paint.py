"""Local QA only: analyze one explicitly selected public-playback session.
No network. rAF is a browser paint opportunity, not optical display measurement.
"""
import argparse,json,wave
from pathlib import Path
p=argparse.ArgumentParser();p.add_argument('--session-dir',required=True);p.add_argument('--output',required=True);p.add_argument('--until-ms',type=float);p.add_argument('--audio-seconds',type=float,default=20)
a=p.parse_args();session=Path(a.session_dir);out=Path(a.output);out.parent.mkdir(parents=True,exist_ok=True)
rows=[json.loads(x) for x in (session/'live-paint-audit.jsonl').read_text(encoding='utf-8').splitlines()]
rows=sorted([r for r in rows if a.until_ms is None or r['wall_ms']<=a.until_ms],key=lambda r:r['wall_ms'])
assert rows,'No visible caption trace; MAY_LATENCY_AUDIT=1 required on a public QA session'
summary={'transcript':rows[-1]['text'],'measurement':'capture epoch to visible DOM after two rAF; not human/optical ground truth','observations':len(rows)}
def stats(values):
 values=sorted(values)
 return {'count':len(values),'p50_ms':values[len(values)//2],'p95_ms':values[int((len(values)-1)*.95)],'max_ms':max(values)} if values else None
phases=[r for r in rows if 'received_ms' in r]
summary['backend_lag']=stats([r['emitted_ms']-r['audio_ms'] for r in phases if not r['final']])
summary['event_delivery']=stats([r['received_ms']-r['emitted_ms'] for r in phases])
summary['render_opportunity']=stats([r['wall_ms']-r['received_ms'] for r in phases])
out.write_text(json.dumps({'summary':summary,'rows':rows},ensure_ascii=False,indent=2),encoding='utf-8')
with wave.open(str(out.with_suffix('.wav')),'wb') as w:
 w.setnchannels(1);w.setsampwidth(2);w.setframerate(16000);w.writeframes((session/'system.pcm').read_bytes()[:int(a.audio_seconds*32000)])
print(json.dumps({k:v for k,v in summary.items() if k!='transcript'},ensure_ascii=False))
