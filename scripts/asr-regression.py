"""Public-fixture worker tests. Standard library only; no uploads or private recordings."""
import argparse, json, math, random, struct, subprocess, time, wave, sys
from pathlib import Path
sys.stdout.reconfigure(encoding="utf-8")
sys.stderr.reconfigure(encoding="utf-8")

p=argparse.ArgumentParser()
p.add_argument('--exe',required=True)
p.add_argument('--model',required=True)
p.add_argument('--output',default='test-results/asr-regression')
args=p.parse_args()
out=Path(args.output);out.mkdir(parents=True,exist_ok=False)
fixtures=Path('test-results/fixtures')
with wave.open(str(fixtures/'vietnamese-pcm.wav')) as f:
    speech=f.readframes(f.getnframes())
def wav(name,data,rate=16000,channels=1):
    path=out/name
    with wave.open(str(path),'wb') as f:
        f.setnchannels(channels);f.setsampwidth(2);f.setframerate(rate);f.writeframes(data)
    return path
silence=bytes(12*32000)
leading=wav('leading-12s.wav',silence+speech)
repeated=wav('repeated-with-12s-gap.wav',speech+silence+speech)
silent=wav('silence.wav',bytes(20*32000))
rng=random.Random(1594)
noise=wav('noise.wav',b''.join(struct.pack('<h',rng.randint(-500,500)) for _ in range(20*16000)))
tiny=wav('short.wav',bytes(1600))
bad=wav('wrong-rate.wav',speech,rate=8000)
turns=wav('short-turns-with-pauses.wav',speech[:4*32000]+bytes(3*32000)+speech[:4*32000]+bytes(3*32000)+speech[:4*32000])
rows=[]
flags=getattr(subprocess,'CREATE_NO_WINDOW',0)
def run(path,language='vi',ok=True):
    started=time.perf_counter()
    proc=subprocess.run([args.exe,'--transcribe-test',args.model,str(path),language],capture_output=True,text=True,encoding='utf-8',timeout=180,creationflags=flags)
    message=json.loads(proc.stdout)
    if ok:
        assert proc.returncode==0 and not message['Ok']['error'],message
        result=message['Ok']
    else:
        assert proc.returncode!=0,message
        assert message.get('Err') or message.get('Ok',{}).get('error'),message
        result=message
    rows.append(dict(case=str(path),result=result,wall_seconds=time.perf_counter()-started,passed=True))
    return result
try:
    for name,language in [('vietnamese-pcm.wav','vi'),('english.wav','en')]:
        assert run(fixtures/name,language)['segments']
    result=run(leading)
    assert result['segments'] and min(s['start'] for s in result['segments'])>=11.5,result
    result=run(repeated)
    assert any(s['start']<16 for s in result['segments']),result
    assert any(s['start']>=26 for s in result['segments']),result
    result=run(turns)
    assert result['segments'] and result['vad_windows']>=2,result
    assert result['inference_windows']==1,result
    for path in [silent,noise,tiny]:
        assert not run(path)['segments']
    run(bad,ok=False)
    jobs=[dict(path=str(fixtures/'vietnamese-pcm.wav'),offset=0,language='invalid'),dict(path=str(fixtures/'vietnamese-pcm.wav'),offset=50,language='vi')]
    proc=subprocess.run([args.exe,'--asr-worker',args.model],input=''.join(json.dumps(j)+'\n' for j in jobs),capture_output=True,text=True,encoding='utf-8',timeout=180,creationflags=flags)
    assert proc.returncode==0,proc.stderr[-1000:]
    messages=[json.loads(x) for x in proc.stdout.splitlines() if x.strip()]
    assert len(messages)==2 and messages[0]['error'] and messages[1]['segments'],messages
    assert min(s['start'] for s in messages[1]['segments'])>=50,messages
    rows.append(dict(case='worker recovers from invalid job and keeps offset',result=messages,passed=True))
except Exception as e:
    rows.append(dict(passed=False,error=str(e)))
    raise
finally:
    (out/'results.json').write_text(json.dumps(rows,ensure_ascii=False,indent=2),encoding='utf-8')
print(f'PASS {len(rows)} ASR/worker cases. Results: {out}')
