"""Exercise all local levels without playing audio into an active YouTube stream."""
import json,pathlib,subprocess,time,wave,os
root=pathlib.Path.cwd();model=root/'src-tauri/target/debug/live-models/en'
with wave.open(str(root/'test-results/fixtures/realtime/reference.wav'),'rb') as wav: pcm=wav.readframes(wav.getnframes())
results=[]
for level in ['fast','balanced','accurate']:
    started=time.monotonic()
    child=subprocess.Popen([str(root/'src-tauri/target/debug/innote.exe'),'--caption-stream-worker',str(model),'en',level],stdin=subprocess.PIPE,stdout=subprocess.PIPE,stderr=subprocess.DEVNULL,creationflags=subprocess.CREATE_NO_WINDOW)
    ready=json.loads(child.stdout.readline());loaded=time.monotonic();assert ready.get('ready') is True,ready
    output,_=child.communicate(pcm,timeout=60);finished=time.monotonic()
    values=[json.loads(line) for line in output.splitlines()]
    errors=[v['error'] for v in values if 'error' in v];assert not errors,errors
    updates=[v['update'] for v in values if 'update' in v]
    processed=max((v.get('processed',0) for v in values),default=0)
    assert child.returncode==0 and processed==len(pcm)//2 and any(u['text'] for u in updates)
    results.append({'model':'moonshine-en','level':level,'setupMs':round((loaded-started)*1000),'decodeMs':round((finished-loaded)*1000),'audioMs':len(pcm)/32,'processedSamples':processed,'finalText':' '.join(u['text'] for u in updates if u['final_text']),'passed':True})
path=root/'test-results/speech-levels-offline.json';path.write_text(json.dumps(results,indent=2),encoding='utf-8');print(json.dumps(results))
