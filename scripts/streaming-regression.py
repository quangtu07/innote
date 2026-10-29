"""Exercise the real native streaming child with paced public PCM, never user audio."""
import argparse
import json
from pathlib import Path
import queue
import sys
import subprocess
import threading
import time
import wave

parser = argparse.ArgumentParser()
parser.add_argument("--exe", required=True)
parser.add_argument("--output", required=True)
parser.add_argument("--leading-only", action="store_true")
parser.add_argument("--multilingual-only", action="store_true")
args = parser.parse_args()
out = Path(args.output).resolve()
out.mkdir(parents=True, exist_ok=False)
exe = str(Path(args.exe).resolve())
fixtures = Path(__file__).resolve().parents[1] / "test-results" / "fixtures"

def pcm(name):
    with wave.open(str(fixtures / name)) as audio:
        assert audio.getframerate() == 16000 and audio.getnchannels() == 1 and audio.getsampwidth() == 2
        return audio.readframes(audio.getnframes())

results = []
def run(name, language, audio, *, paced=True, want_text=True, offset=None):
    directory = out / name
    directory.mkdir()
    process = subprocess.Popen([exe, "--live-worker", language, str(directory)],
        stdin=subprocess.PIPE, stdout=subprocess.PIPE, stderr=subprocess.PIPE,
        creationflags=subprocess.CREATE_NO_WINDOW)
    messages = queue.Queue()
    def receive():
        for line in process.stdout:
            messages.put((time.perf_counter(), json.loads(line)))
        messages.put((time.perf_counter(), None))
    threading.Thread(target=receive, daemon=True).start()
    _, ready = messages.get(timeout=30)
    assert ready == {"ready": True}, ready
    start = time.perf_counter()
    eof_at = [None]
    def feed():
        try:
            with (directory / "system.pcm").open("wb", buffering=0) as file:
                for at in range(0, len(audio), 640):
                    frame = audio[at:at+640]
                    if paced:
                        time.sleep(max(0, start + (at+len(frame))/32000 - time.perf_counter()))
                    file.write(frame)
        finally:
            eof_at[0] = time.perf_counter()
            process.stdin.close()
    feeder = threading.Thread(target=feed)
    feeder.start()
    updates = []
    try:
        while True:
            when, update = messages.get(timeout=30)
            if update is None:
                break
            updates.append({"wall_ms": round((when-start)*1000,2), **update})
        process.wait(timeout=5)
        feeder.join(timeout=5)
        assert process.returncode == 0, f"exit={process.returncode}: " + process.stderr.read().decode("utf-8", errors="replace")
        text_updates = [x for x in updates if x["text"]]
        words = [word for x in updates if x["final_text"] for word in x["words"]]
        duration = len(audio)/32000
        assert all(0 <= w["start"] < w["end"] <= duration+0.1 for w in words), words
        if want_text:
            assert text_updates and words, "No streaming/final text"
            if paced:
                assert text_updates[0]["wall_ms"] < (eof_at[0]-start)*1000, "Text arrived only after recording ended"
            if offset:
                assert any(w["start"] >= offset for w in words), "Paused timeline offset was lost"
        else:
            assert not text_updates, "Silence produced hallucinated text"
        result = {"case": name, "passed": True, "duration_s":duration,
            "first_text_ms":text_updates[0]["wall_ms"] if text_updates else None,
            "updates":len(updates), "drain_ms":round((time.perf_counter()-eof_at[0])*1000,2),
            "transcript":" ".join(w["text"] for w in words)}
        (directory/"updates.json").write_text(json.dumps(updates,ensure_ascii=False,indent=2),encoding="utf-8")
        results.append(result)
        print(json.dumps({k:v for k,v in result.items() if k!="transcript"}),flush=True)
    finally:
        (directory/"updates.json").write_text(json.dumps(updates,ensure_ascii=False,indent=2),encoding="utf-8")
        if process.poll() is None:
            process.kill()
            process.wait()

try:
    if args.multilingual_only:
        run("four-language-paced", "multi", pcm("multilingual/four-languages.wav"))
        import re
        text=results[-1]['transcript']
        assert 'engineering' in text.lower() and 'loài' in text and re.search(r'[가-힣]',text) and re.search(r'[一-鿿]',text),text
        run("multi-silence", "multi", bytes(32000*2), paced=False, want_text=False)
        run("multi-noise", "multi", pcm("latency-noise.wav"), paced=False, want_text=False)
        run("multi-short-stop", "multi", bytes(600), paced=False, want_text=False)
        ko = pcm("multilingual/ko_kr-heldout.wav")
        zh = pcm("multilingual/cmn_hans_cn-heldout.wav")
        run("multi-pause-gap", "multi", ko + bytes(32000*12) + zh,
            paced=False, offset=len(ko)/32000+12)
        sys.exit(0)
    vi = pcm("vietnamese-pcm.wav")
    if args.leading_only:
        run("leading-four-seconds", "vi", bytes(32000*4)+vi)
        sys.exit(0)
    run("Vietnamese-paced", "vi", vi)
    run("English-paced", "en", pcm("english.wav"))
    # Fast feed a long pause, then ensure it is skipped and timestamps stay absolute.
    run("pause-gap-offset", "vi", vi[:160000] + bytes(32000*12) + vi[160000:], paced=False, offset=17)
    run("silence", "vi", bytes(32000*2), paced=False, want_text=False)
    run("low-noise", "vi", pcm("latency-noise.wav"), paced=False, want_text=False)
    run("short-stop", "vi", bytes(600), paced=False, want_text=False)
    p = subprocess.run([exe,"--live-worker","invalid",str(out)],capture_output=True,timeout=10,
        creationflags=subprocess.CREATE_NO_WINDOW)
    assert p.returncode != 0
    results.append({"case":"invalid-language-fails-closed","passed":True})
finally:
    (out/"results.json").write_text(json.dumps(results,ensure_ascii=False,indent=2),encoding="utf-8")
