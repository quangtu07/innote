"""Download two fresh public FLEURS validation rows per language.

Rows 0 and 1 were already used by the original/held-out panel, so offsets 3 and 4
are a new set. No API key, no private audio. FLEURS is CC BY 4.0.
"""
import argparse
import json
import pathlib
import struct
import urllib.parse
import urllib.request

p = argparse.ArgumentParser()
p.add_argument('--output', default='test-results/fixtures/fleurs-independent')
p.add_argument('--offsets', default='3,4')
a = p.parse_args()
out = pathlib.Path(a.output)
out.mkdir(parents=True, exist_ok=True)
offsets = [int(x) for x in a.offsets.split(',')]
configs = ['vi_vn', 'en_us', 'ko_kr', 'cmn_hans_cn']


def fetch_rows(config, offset):
    query = urllib.parse.urlencode({
        'dataset': 'google/fleurs', 'config': config, 'split': 'validation',
        'offset': offset, 'length': 1,
    })
    url = 'https://datasets-server.huggingface.co/rows?' + query
    with urllib.request.urlopen(url, timeout=45) as response:
        return json.load(response)['rows'][0]


def decode_wav(data):
    assert data[:4] == b'RIFF' and data[8:12] == b'WAVE'
    pos, fmt, pcm = 12, None, None
    while pos + 8 <= len(data):
        ident = data[pos:pos + 4]
        size = struct.unpack('<I', data[pos + 4:pos + 8])[0]
        body = data[pos + 8:pos + 8 + size]
        pos += 8 + size + (size & 1)
        if ident == b'fmt ':
            fmt = body
        elif ident == b'data':
            pcm = body
    tag, channels, rate, _, _, bits = struct.unpack('<HHIIHH', fmt[:16])
    assert rate == 16000 and pcm is not None, (rate, tag)
    if tag == 1:
        assert bits == 16
        values = struct.unpack('<%dh' % (len(pcm) // 2), pcm)
    elif tag == 3:
        assert bits == 32
        values = [max(-1.0, min(1.0, x)) for x in struct.unpack('<%df' % (len(pcm) // 4), pcm)]
    else:
        raise ValueError('unsupported wav tag %s' % tag)
    mono = []
    for i in range(0, len(values), channels):
        frame = values[i:i + channels]
        sample = sum(frame) / channels
        mono.append(int(round(sample * 32767)) if tag == 3 else int(round(sample)))
    return b''.join(struct.pack('<h', max(-32768, min(32767, x))) for x in mono)


metadata = []
for config in configs:
    for offset in offsets:
        item = fetch_rows(config, offset)
        row = item['row']
        name = f'{config}-v{offset}'
        audio_path = out / (name + '.wav')
        if not audio_path.exists():
            with urllib.request.urlopen(row['audio'][0]['src'], timeout=90) as response:
                raw = response.read()
            pcm = decode_wav(raw)
            (out / (name + '.wav')).write_bytes(
                b'RIFF' + struct.pack('<I', 36 + len(pcm)) + b'WAVEfmt ' +
                struct.pack('<IHHIIHH', 16, 1, 1, 16000, 32000, 2, 16) +
                b'data' + struct.pack('<I', len(pcm)) + pcm)
        (out / (name + '.txt')).write_text(row['raw_transcription'].strip(), encoding='utf-8')
        metadata.append({
            'name': name, 'config': config, 'offset': offset, 'row_id': row['id'],
            'reference': row['raw_transcription'], 'source': 'google/fleurs validation',
            'license': 'CC BY 4.0',
        })
        print(name, round((audio_path.stat().st_size - 44) / 32000, 3), 's')
(out / 'metadata.json').write_text(json.dumps(metadata, ensure_ascii=False, indent=2), encoding='utf-8')