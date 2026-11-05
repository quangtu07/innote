"""Opt-in offline QA, existing local models and public fixture files; no capture/API."""
import ctypes, json, os, subprocess, sys, threading, time, wave
from pathlib import Path
from ctypes import wintypes
sys.stdout.reconfigure(encoding="utf-8")
ROOT=Path(__file__).resolve().parent.parent
OUT=ROOT/"test-results/local-file-correction"
OUT.mkdir(parents=True,exist_ok=True)
EXE=ROOT/"src-tauri/target/debug/innote.exe"
class Counters(ctypes.Structure):
    _fields_=[("cb",wintypes.DWORD),("faults",wintypes.DWORD)]+[(key,ctypes.c_size_t) for key in ["peak_working_set","working_set","peak_paged","paged","peak_nonpaged","nonpaged","pagefile","peak_pagefile"]]
memory=ctypes.windll.psapi.GetProcessMemoryInfo
memory.argtypes=[wintypes.HANDLE,ctypes.POINTER(Counters),wintypes.DWORD]
times=ctypes.windll.kernel32.GetProcessTimes
times.argtypes=[wintypes.HANDLE]+[ctypes.POINTER(wintypes.FILETIME)]*4
def cpu_seconds(proc):
    values=[wintypes.FILETIME() for _ in range(4)]
    if not times(int(proc._handle),*(ctypes.byref(v) for v in values)):return None
    return sum((v.dwHighDateTime<<32)+v.dwLowDateTime for v in values[2:])/10_000_000
models=[("moonshine-en",EXE.parent/"live-models/en","en","test-results/fixtures/realtime/reference.wav"),("moonshine-vi",EXE.parent/"live-models/vi","vi","test-results/fixtures/vietnamese-pcm.wav")]
for language,fixture in [("en","test-results/fixtures/realtime/reference.wav"),("vi","test-results/fixtures/vietnamese-pcm.wav")]:
    models.append((f"nemotron-3.5-q8-{language}",Path(os.environ["LOCALAPPDATA"])/"Innote/models/nemotron-3.5-q8.gguf",language,fixture))
results=[]
for name,model,language,fixture in models:
    if not model.exists():results.append({"model":name,"available":False});continue
    with wave.open(str(ROOT/fixture),"rb") as audio:
        duration=audio.getnframes()/audio.getframerate();pcm=audio.readframes(audio.getnframes())
    env=os.environ.copy();env["OPENBLAS_NUM_THREADS"]="1";env["OMP_NUM_THREADS"]="1"
    began=time.perf_counter()
    proc=subprocess.Popen([str(EXE),"--caption-stream-worker",str(model),language,"accurate","file-correction"],stdin=subprocess.PIPE,stdout=subprocess.PIPE,stderr=subprocess.PIPE,creationflags=0x08000000|0x00004000,env=env)
    metrics={"peak_rss_mb_sampled":0.0}
    def sample():
        while proc.poll() is None:
            info=Counters();info.cb=ctypes.sizeof(info)
            if memory(int(proc._handle),ctypes.byref(info),info.cb):metrics["peak_rss_mb_sampled"]=max(metrics["peak_rss_mb_sampled"],info.peak_working_set/1048576)
            time.sleep(.02)
    monitor=threading.Thread(target=sample);monitor.start()
    try:stdout,stderr=proc.communicate(pcm,timeout=90)
    except subprocess.TimeoutExpired:proc.kill();stdout,stderr=proc.communicate()
    monitor.join();wall=time.perf_counter()-began
    notices=[]
    for line in stdout.decode("utf-8",errors="replace").splitlines():
        try:notices.append(json.loads(line))
        except json.JSONDecodeError:pass
    updates=[v["update"] for v in notices if v.get("update",{}).get("final_text")]
    result={"model":name,"language":language,"duration_s":duration,"wall_s":round(wall,3),"rtf":round(wall/duration,3),"cpu_s":cpu_seconds(proc),"exit_code":proc.returncode,"text":" ".join(v["text"] for v in updates),"words":sum(len(v.get("words",[])) for v in updates),"errors":[v["error"] for v in notices if "error" in v],**metrics}
    results.append(result)
    (OUT/(name+".json")).write_text(json.dumps({"result":result,"updates":updates},ensure_ascii=False,indent=2),encoding="utf-8")
    print(json.dumps(result,ensure_ascii=False),flush=True)
(OUT/"results.json").write_text(json.dumps(results,ensure_ascii=False,indent=2),encoding="utf-8")
