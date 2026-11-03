"""CPU/decoder stress only: artificial dense speech, never played through speakers."""
import array,json,pathlib,subprocess,time,wave,audioop
root=pathlib.Path.cwd()
with wave.open(str(root/'test-results/fixtures/realtime/reference.wav'),'rb') as wav: pcm=wav.readframes(wav.getnframes())
kept=[];quiet=0
for offset in range(0,len(pcm),320):
    block=pcm[offset:offset+320]
    if audioop.rms(block,2)<150:quiet+=1
    else:quiet=0
    if quiet<=3:kept.append(block)
dense=b''.join(kept)*5
results=[]
for level in ['fast','balanced','accurate']:
    child=subprocess.Popen([str(root/'src-tauri/target/debug/innote.exe'),'--caption-stream-worker',str(root/'src-tauri/target/debug/live-models/en'),'en',level],stdin=subprocess.PIPE,stdout=subprocess.PIPE,stderr=subprocess.DEVNULL,creationflags=subprocess.CREATE_NO_WINDOW)
    try:
        assert json.loads(child.stdout.readline()).get('ready') is True
        start=time.monotonic();output,_=child.communicate(dense,timeout=120);elapsed=time.monotonic()-start
    finally:
        if child.poll() is None:
            child.kill()
            child.communicate()
    values=[json.loads(line) for line in output.splitlines()]
    assert child.returncode==0 and not any('error' in v for v in values)
    assert max(v.get('processed',0) for v in values)==len(dense)//2
    results.append({'model':'moonshine-en','level':level,'audioSeconds':len(dense)/32000,'processingSeconds':round(elapsed,3),'realTimeFactor':round(elapsed/(len(dense)/32000),3),'updates':sum('update' in v for v in values),'allSamplesProcessed':True})
path=root/'test-results/speech-levels-continuous.json';path.write_text(json.dumps(results,indent=2),encoding='utf-8');print(json.dumps(results))
