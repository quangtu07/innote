"""Replay the owned QA archive through the actual meeting worker, no speaker output."""
import json,pathlib,subprocess,time,os
root=pathlib.Path.cwd();directory=pathlib.Path(os.environ['LOCALAPPDATA'])/'Innote/sessions/4f617220-be47-4f1e-a383-2c45f96c12ba'
model=pathlib.Path(os.environ['LOCALAPPDATA'])/'Innote/models/nemotron-3.5-q8.gguf'
assert directory.is_dir() and model.is_file()
results=[]
for name in ['innote-before-level-fix.exe','innote.exe']:
    child=subprocess.Popen([str(root/'src-tauri/target/debug'/name),'--live-worker','en',str(directory),'0',str(model),'accurate'],stdin=subprocess.PIPE,stdout=subprocess.PIPE,stderr=subprocess.DEVNULL,creationflags=subprocess.CREATE_NO_WINDOW)
    ready=child.stdout.readline();assert json.loads(ready).get('ready') is True
    started=time.monotonic();out,_=child.communicate(b'',timeout=90)
    rows=[json.loads(line) for line in out.splitlines()]
    finals=[r for r in rows if r.get('final_text') and r.get('text','').strip()]
    assert child.returncode==0 and finals
    results.append({'binary':name,'model':'nemotron-3.5-q8','level':'accurate','processingSeconds':round(time.monotonic()-started,3),'finals':len(finals),'lastAudioEnd':max(r['end'] for r in rows),'allFinalText':' '.join(r['text'] for r in finals),'passed':True})
path=root/'test-results/speech-levels-meeting-worker.json';path.write_text(json.dumps(results,indent=2),encoding='utf-8');print(json.dumps(results))
