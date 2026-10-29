"""Developer-only smoke benchmark. Not shipped or needed by the desktop app."""
import argparse, json, re, subprocess, time, unicodedata, wave, sys
from pathlib import Path
sys.stdout.reconfigure(encoding="utf-8")

def normalize(value):
    return re.sub(r"[^\w\s]", "", unicodedata.normalize("NFC", value).lower(), flags=re.UNICODE).split()

def distance(a, b):
    previous = list(range(len(b) + 1))
    for i, item in enumerate(a, 1):
        row = [i]
        for j, other in enumerate(b, 1):
            row.append(min(row[-1] + 1, previous[j] + 1, previous[j-1] + (item != other)))
        previous = row
    return previous[-1]

def main():
    p = argparse.ArgumentParser()
    p.add_argument("--exe", required=True)
    p.add_argument("--models", required=True)
    p.add_argument("--output", default="test-results/benchmark.json")
    args = p.parse_args()
    fixtures = Path("test-results/fixtures")
    cases = [
        ("Vietnamese FLEURS validation row 0", "vietnamese-pcm.wav", "vietnamese.txt", "vi"),
        ("English synthetic SAPI", "english.wav", "english.txt", "en"),
        ("Vietnamese-English concatenated", "mixed.wav", "mixed.txt", "auto"),
    ]
    rows = []
    for model in ("base-q5_1", "small-q5_1", "large-v3-turbo-q5_0"):
        for label, audio, reference, language in cases:
            if not (fixtures / audio).exists():
                continue
            with wave.open(str(fixtures / audio)) as f:
                duration = f.getnframes() / f.getframerate()
            start = time.perf_counter()
            try:
                run = subprocess.run([args.exe, "--transcribe-test", str(Path(args.models) / f"ggml-{model}.bin"), str(fixtures / audio), language], capture_output=True, text=True, encoding="utf-8", timeout=180)
                message = json.loads(run.stdout)
                result = message.get("Ok")
                if not result or result.get("error"):
                    raise RuntimeError(message)
                text = " ".join(x["text"] for x in result["segments"])
                expected = (fixtures / reference).read_text(encoding="utf-8-sig").strip()
                a, b = normalize(expected), normalize(text)
                ca, cb = " ".join(a), " ".join(b)
                row = dict(model=model, case=label, duration=duration, inference_seconds=result["elapsed_ms"] / 1000, wall_seconds=time.perf_counter()-start, rtf=result["elapsed_ms"] / 1000 / duration, wer=distance(a,b)/max(1,len(a)), cer=distance(ca,cb)/max(1,len(ca)), reference=expected, transcript=text)
            except Exception as e:
                row = dict(model=model, case=label, error=str(e))
            rows.append(row)
            print(json.dumps(row, ensure_ascii=False), flush=True)
            Path(args.output).write_text(json.dumps(rows, ensure_ascii=False, indent=2), encoding="utf-8")
    print("Small smoke set only; not an accuracy guarantee or a meeting benchmark.", flush=True)

if __name__ == "__main__":
    main()
