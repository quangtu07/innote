"""Build a public bilingual/CJK resume fixture from tracked FLEURS/SAPI clips."""
import json
import pathlib
import wave

fixtures = pathlib.Path('test-results/fixtures')
multi = fixtures / 'multilingual'
turns = [
    (fixtures / 'vietnamese-pcm.wav', 0.0, 12.0),
    (multi / 'cmn_hans_cn.wav', 0.0, 18.38),
    (multi / 'ko_kr.wav', 0.0, 10.56),
    (multi / 'cmn_hans_cn-heldout.wav', 0.0, 7.44),
]
silence = bytes(16000)  # 0.5 s exact digital silence at 16 kHz PCM16
pcm = b''
order = []
for path, start, end in turns:
    with wave.open(str(path), 'rb') as audio:
        assert (audio.getnchannels(), audio.getsampwidth(), audio.getframerate()) == (1, 2, 16000)
        audio.setpos(int(start * 16000))
        frames = audio.readframes(int((end - start) * 16000))
    pcm += frames + silence
    order.append({'file': path.name, 'start_s': start, 'duration_s': end - start, 'source': 'public fixture already tracked in test-results/fixtures (FLEURS CC BY 4.0 / project test clip)'})
out = fixtures / 'multilingual' / 'cjk-resume-public.wav'
with wave.open(str(out), 'wb') as audio:
    audio.setparams((1, 2, 16000, 0, 'NONE', 'not compressed'))
    audio.writeframes(pcm)
(out.with_suffix('.json')).write_text(json.dumps({
    'purpose': 'Resume windows must not duplicate Chinese/Korean text at 20 s watermark + 1.5 s look-back. Public fixtures only; no user audio.',
    'turns': order,
    'added_gap_s': 0.5,
    'duration_s': len(pcm) / 32000,
}, ensure_ascii=False, indent=2), encoding='utf-8')
pcm_dir = pathlib.Path('test-results/cjk-resume-0.5.2/pcm')
pcm_dir.mkdir(parents=True, exist_ok=True)
(pcm_dir / 'system.pcm').write_bytes(pcm)
print(out.resolve(), len(pcm) / 32000)