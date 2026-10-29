"""Estimate per-word latency using an independent, constrained HMM alignment.

This is a diagnostic proxy, NOT a manually annotated latency acceptance set.
Audio onset -> first text, decoder compute, and word-end -> text are different.
"""
import argparse
import ctypes as c
import json
import os
from pathlib import Path
import sys
import wave
from benchmark import normalize, distance

sys.stdout.reconfigure(encoding="utf-8")
p=argparse.ArgumentParser()
p.add_argument("--result",action="append",required=True)
p.add_argument("--output",required=True)
p.add_argument("--audio",default="test-results/fixtures/vietnamese-pcm.wav")
p.add_argument("--reference",default="test-results/fixtures/vietnamese.txt")
a=p.parse_args()
reference=normalize(Path(a.reference).read_text(encoding="utf-8-sig"))
directory=Path('.tools/vosk/runtime/vosk').resolve()
cookie=os.add_dll_directory(str(directory))
d=c.CDLL(str(directory/'libvosk.dll'))
def bind(name,params,result):
    f=getattr(d,'vosk_'+name);f.argtypes=params;f.restype=result;return f
bind('set_log_level',[c.c_int],None)(-1)
model_new=bind('model_new',[c.c_char_p],c.c_void_p)
recognizer_new=bind('recognizer_new_grm',[c.c_void_p,c.c_float,c.c_char_p],c.c_void_p)
accept=bind('recognizer_accept_waveform',[c.c_void_p,c.c_char_p,c.c_int],c.c_int)
result=bind('recognizer_result',[c.c_void_p],c.c_char_p)
finish=bind('recognizer_final_result',[c.c_void_p],c.c_char_p)
free=bind('recognizer_free',[c.c_void_p],None)
free_model=bind('model_free',[c.c_void_p],None)
model=model_new(str(Path('.tools/vosk/models/vosk-model-small-vn-0.4').resolve()).encode())
assert model
recognizer=recognizer_new(model,16000,json.dumps([' '.join(reference)],ensure_ascii=False).encode())
assert recognizer
bind('recognizer_set_words',[c.c_void_p,c.c_int],None)(recognizer,1)
words=[]
try:
    with wave.open(a.audio) as audio:
        assert audio.getframerate()==16000 and audio.getnchannels()==1 and audio.getsampwidth()==2
        while frame:=audio.readframes(1600):
            status=accept(recognizer,frame,len(frame));assert status>=0
            if status:words.extend(json.loads(result(recognizer)).get('result',[]))
    words.extend(json.loads(finish(recognizer)).get('result',[]))
finally:free(recognizer);free_model(model)
def pairs(hypothesis, complete=False):
    # Align a partial hypothesis to the reference prefix it has reached, rather
    # than let a repeated word jump to a much later occurrence in the reference.
    n,m=len(reference),len(hypothesis)
    dp=[list(range(m+1))]
    for i in range(1,n+1):
        row=[i]
        for j in range(1,m+1):
            row.append(min(row[-1]+1,dp[-1][j]+1,dp[-1][j-1]+(reference[i-1]!=hypothesis[j-1])))
        dp.append(row)
    i=n if complete else min(range(n+1),key=lambda i:(dp[i][m],abs(i-m),i));j=m;out={}
    while i and j:
        if reference[i-1]==hypothesis[j-1] and dp[i][j]==dp[i-1][j-1]:out[i-1]=j-1;i-=1;j-=1
        elif dp[i][j]==dp[i-1][j-1]+1:i-=1;j-=1
        elif dp[i][j]==dp[i-1][j]+1:i-=1
        else:j-=1
    return out

def matches(hypothesis):return set(pairs(hypothesis))

alignment_pairs=pairs([w['word'] for w in words],complete=True)
aligned={i:words[j] for i,j in alignment_pairs.items()}
assert len(aligned)>=len(reference)/2, 'Too few reference words aligned for even a diagnostic proxy'

def stats(values):
    values=sorted(values)
    percentile=lambda p:values[min(len(values)-1,int((len(values)-1)*p))] if values else None
    return {'count':len(values),'p50_ms':percentile(.5),'p95_ms':percentile(.95),'max_ms':max(values,default=None),'under_500_fraction':sum(v<500 for v in values)/len(values) if values else None}

reports=[]
for name in a.result:
    data=json.loads(Path(name).read_text(encoding='utf-8-sig'))
    first={};stable={};previous=set()
    for row in data['rows']:
        current=matches(normalize(row['text']))
        for i in current:
            first.setdefault(i,row['wall_ms'])
            if i not in previous:stable[i]=row['wall_ms']
        previous=current
    final=matches(normalize(data['summary']['transcript']))
    details=[{'word':reference[i],'start_ms':aligned[i]['start']*1000 if i in aligned else None,'end_ms':aligned[i]['end']*1000 if i in aligned else None,'first_correct_ms':first.get(i),'stable_ms':stable.get(i) if i in final else None,'correct_in_final':i in final} for i in range(len(reference))]
    reports.append({'result':name,'wer':distance(reference,normalize(data['summary']['transcript']))/len(reference),
        'final_reference_coverage':len(final)/len(reference),
        'alignment_reference_coverage':len(aligned)/len(reference),
        'first_correct_word_delay_proxy':stats([first[i]-aligned[i]['end']*1000 for i in final if i in first and i in aligned]),
        'stable_word_delay_proxy':stats([stable[i]-aligned[i]['end']*1000 for i in final if i in stable and i in aligned]),'words':details})
out={'alignment_kind':'independent Vosk HMM vocabulary constrained to reference; incorrect/missing alignment words excluded; not human ground truth','alignment':words,'reports':reports}
Path(a.output).write_text(json.dumps(out,ensure_ascii=False,indent=2),encoding='utf-8')
for report in reports:print(json.dumps({k:v for k,v in report.items() if k!='words'},ensure_ascii=False))
