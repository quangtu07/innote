"""Compare two native builds on public multilingual turns; character error rate.

NFC/lowercase alphanumeric normalization ignores punctuation and whitespace for
comparable Han/Hangul/Latin scoring. This is a small panel, not general accuracy.
"""
import argparse, json, subprocess, unicodedata, wave
from pathlib import Path

def normalize(s):
    return ''.join(c for c in unicodedata.normalize('NFC', s.lower()) if c.isalnum())

def distance(a, b):
    row = list(range(len(b)+1))
    for i, x in enumerate(a, 1):
        new = [i]
        for j, y in enumerate(b, 1):
            new.append(min(new[-1]+1, row[j]+1, row[j-1]+(x != y)))
        row = new
    return row[-1]

def main():
    p=argparse.ArgumentParser()
    p.add_argument('--before',required=True);p.add_argument('--after',required=True)
    p.add_argument('--model',required=True);p.add_argument('--output',required=True)
    p.add_argument('--baseline-results',help='Reuse a previously measured baseline with identical references')
    a=p.parse_args();out=Path(a.output).resolve();out.mkdir(parents=True,exist_ok=True)
    fixtures=Path('test-results/fixtures');multi=fixtures/'multilingual'
    cases=[('original',[(fixtures/'vietnamese-pcm.wav',fixtures/'vietnamese.txt'),
        (fixtures/'english.wav',fixtures/'english.txt'),(multi/'ko_kr.wav',multi/'ko_kr.txt'),
        (multi/'cmn_hans_cn.wav',multi/'cmn_hans_cn.txt')]),
        ('heldout-chinese-first',[(multi/'cmn_hans_cn-heldout.wav',multi/'cmn_hans_cn-heldout.txt'),
        (fixtures/'fleurs-vi-1.wav',fixtures/'fleurs-vi-1.txt'),
        (multi/'ko_kr-heldout.wav',multi/'ko_kr-heldout.txt'),(fixtures/'english.wav',fixtures/'english.txt')]),
        ('heldout-korean-first',[(multi/'ko_kr-heldout.wav',multi/'ko_kr-heldout.txt'),
        (fixtures/'fleurs-vi-2.wav',fixtures/'fleurs-vi-2.txt'),
        (multi/'cmn_hans_cn-heldout.wav',multi/'cmn_hans_cn-heldout.txt')])]
    jobs=[];references=[]
    for name,turns in cases:
        pcm=b'';ref=[]
        for audio,text in turns:
            with wave.open(str(audio),'rb') as w:
                assert (w.getnchannels(),w.getsampwidth(),w.getframerate())==(1,2,16000)
                pcm+=w.readframes(w.getnframes())+bytes(25600)
            ref.append(text.read_text(encoding='utf-8-sig').strip())
        path=out/(name+'.wav')
        with wave.open(str(path),'wb') as w:
            w.setparams((1,2,16000,0,'NONE','not compressed'));w.writeframes(pcm)
        jobs.append({'path':str(path),'language':'multi','offset':0})
        references.append(' '.join(ref))
    reports=[]
    for label,exe in [('before',a.before),('after',a.after)]:
        if label=='before' and a.baseline_results:
            cached=[r for r in json.loads(Path(a.baseline_results).read_text(encoding='utf-8')) if r['build']=='before']
            assert [(r['case'],r['reference']) for r in cached]==[(name,ref) for (name,_),ref in zip(cases,references)]
            reports.extend(cached)
            (out/'baseline-reused.txt').write_text(str(Path(a.baseline_results).resolve()),encoding='utf-8')
            continue
        proc=subprocess.run([str(Path(exe).resolve()),'--asr-worker',str(Path(a.model).resolve())],
            input=''.join(json.dumps(j)+'\n' for j in jobs),capture_output=True,encoding='utf-8',
            timeout=240,creationflags=subprocess.CREATE_NO_WINDOW)
        (out/(label+'-stderr.txt')).write_text(proc.stderr,encoding='utf-8')
        assert proc.returncode==0,proc.stderr
        rows=[json.loads(line) for line in proc.stdout.splitlines() if line.strip()]
        assert len(rows)==len(jobs)
        for (name,_),ref,r in zip(cases,references,rows):
            assert not r['error'],r
            text=' '.join(s['text'] for s in r['segments'])
            n=normalize(ref);edits=distance(n,normalize(text))
            report={'build':label,'case':name,'reference_characters':len(n),'edits':edits,
                    'cer':edits/len(n),'elapsed_ms':r['elapsed_ms'],'text':text,'reference':ref}
            reports.append(report);print(json.dumps(report,ensure_ascii=False),flush=True)
        (out/'results.json').write_text(json.dumps(reports,ensure_ascii=False,indent=2),encoding='utf-8')

if __name__=='__main__':main()
