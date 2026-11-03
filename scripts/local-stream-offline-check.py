"""Public-reference pipe diagnostic: measures decoder work independently of playback."""
import json, os, pathlib, subprocess, time, wave
model=pathlib.Path(os.environ['LOCALAPPDATA'])/'Innote/models/nemotron-3.5-q8.gguf'
with wave.open('test-results/fixtures/realtime/reference.wav','rb') as wav:
    pcm=wav.readframes(wav.getnframes())
start=time.monotonic()
child=subprocess.Popen(['src-tauri/target/debug/innote.exe','--caption-stream-worker',str(model),'en'],stdin=subprocess.PIPE,stdout=subprocess.PIPE,stderr=subprocess.DEVNULL,creationflags=subprocess.CREATE_NO_WINDOW)
ready=child.stdout.readline(); loaded=time.monotonic()
assert json.loads(ready).get('ready') is True
out,_=child.communicate(pcm,timeout=60)
updates=[v['update'] for line in out.splitlines() if 'update' in (v:=json.loads(line))]
result={'setupMs':(loaded-start)*1000,'decodeMs':(time.monotonic()-loaded)*1000,'audioMs':len(pcm)/32,'maxDecoderEnd':max((u['end'] for u in updates),default=0),'finalText':' '.join(u['text'] for u in updates if u['final_text']),'updates':len(updates)}
pathlib.Path('test-results/realtime-performance/local-offline.json').write_text(json.dumps(result,indent=2),encoding='utf-8')
print(json.dumps(result))
assert child.returncode==0
