"""Stitch generated public speech with known pauses; no external dependencies."""
import audioop
import json
import pathlib
import wave
root = pathlib.Path('test-results/fixtures/realtime')
lines = json.loads((root/'reference.json').read_text(encoding='utf-8-sig'))
pcm = b'\0\0' * 8000
utterances = []
for index, text in enumerate(lines):
    with wave.open(str(root/f'part-{index}.wav'), 'rb') as wav:
        data = wav.readframes(wav.getnframes())
        width, channels, rate = wav.getsampwidth(), wav.getnchannels(), wav.getframerate()
    if channels == 2:
        data = audioop.tomono(data, width, 0.5, 0.5)
    data = audioop.lin2lin(data, width, 2)
    data, _ = audioop.ratecv(data, 2, 1, rate, 16000, None)
    frames = [data[i:i+320] for i in range(0,len(data),320)]
    voiced = [i for i,f in enumerate(frames) if len(f)>1 and audioop.rms(f,2)>80]
    start = len(pcm)//2
    utterances.append({'text':text, 'speechStartMs':(start+voiced[0]*160)/16,
                       'speechEndMs':(start+(voiced[-1]+1)*160)/16})
    pcm += data + b'\0\0'*12800
with wave.open(str(root/'reference.wav'),'wb') as wav:
    wav.setnchannels(1);wav.setsampwidth(2);wav.setframerate(16000);wav.writeframes(pcm)
(root/'reference-timing.json').write_text(json.dumps({'utterances':utterances,'durationMs':len(pcm)/32},indent=2),encoding='utf-8')
print(json.dumps({'durationMs':len(pcm)/32,'utterances':utterances}))
