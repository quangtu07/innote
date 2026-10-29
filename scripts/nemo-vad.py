"""Deterministic stdlib-only GGML Silero -> F32 GGUF sidecar conversion.

Tensor layout/metadata follows NVIDIA NeMo-Speech.cpp v0.2.0 conversion/vad.py
(Apache-2.0); source weights are the existing Silero MIT model. No downloads.
"""
from pathlib import Path
import hashlib
import math
import struct as s

def convert(source, output):
    tensors = {}
    with Path(source).open('rb') as f:
        def i(): return s.unpack('<i', f.read(4))[0]
        assert i() == 0x67676d6c
        model = f.read(i()).decode()
        version = '.'.join(str(i()) for _ in range(3))
        assert model == 'silero-16k' and version == '5.1.2', (model, version)
        i(); i()
        layers = i()
        assert layers == 4
        for _ in range(layers*3+4): i()
        while header := f.read(12):
            ndim, namesize, dtype = s.unpack('<iii', header)
            assert 0 <= ndim <= 4 and 0 < namesize < 1024 and dtype in (0, 1), (ndim, namesize, dtype)
            dims = s.unpack('<'+'i'*ndim, f.read(ndim*4))
            name = f.read(namesize).decode()
            count = math.prod(dims)
            data = f.read(count*(2 if dtype else 4))
            if dtype:
                data = b''.join(s.pack('<f', v[0]) for v in s.iter_unpack('<e', data))
            assert len(data) == count*4
            tensors[name] = (dims, data)
    def string(x):
        b=x.encode(); return s.pack('<Q',len(b))+b
    metadata = []
    def kv(k,v):
        if isinstance(v,str): val=s.pack('<I',8)+string(v)
        elif isinstance(v,list): val=s.pack('<IIQ',9,5,len(v))+s.pack('<'+'i'*len(v),*v)
        else: val=s.pack('<II',4,v)
        metadata.append(string(k)+val)
    kv('general.architecture','vad'); kv('general.name','silero-vad-v'+version); kv('general.file_type',0)
    for k,v in {'version':version,'sample_rate':16000,'window_size':512,'context_size':64,
        'n_encoder_layers':4,'encoder.in_channels':[129,128,64,64],
        'encoder.out_channels':[128,64,64,128],'encoder.strides':[1,2,2,1],
        'encoder.kernel_size':3,'lstm.input_size':128,'lstm.hidden_size':128,
        'final_conv.in_channels':128,'final_conv.out_channels':1,
        'stft.n_freqs':129,'stft.n_basis':258,'stft.filter_length':256}.items():kv('vad.'+k,v)
    mapping=[]
    for n in range(4):
        for part in ['weight','bias']:mapping.append((f'encoder.{n}.conv.{part}',f'_model.encoder.{n}.reparam_conv.{part}'))
    for side in ['ih','hh']:
        for part in ['weight','bias']:mapping.append((f'lstm.{side}.{part}',f'_model.decoder.rnn.{part}_{side}'))
    mapping += [('final_conv.weight','_model.decoder.decoder.2.weight'),('final_conv.bias','_model.decoder.decoder.2.bias'),('stft.basis','_model.stft.forward_basis_buffer')]
    infos=[];blocks=[];offset=0
    for name,old in mapping:
        dims,data=tensors[old]
        if name=='final_conv.weight':dims=(128,1)
        if name=='final_conv.bias':dims=(1,)
        infos.append(string(name)+s.pack('<I',len(dims))+s.pack('<'+'Q'*len(dims),*dims)+s.pack('<IQ',0,offset))
        padding=(-len(data))%32; blocks.append(data+b'\0'*padding); offset+=len(data)+padding
    header=b'GGUF'+s.pack('<IQQ',3,len(infos),len(metadata))+b''.join(metadata)+b''.join(infos)
    dest=Path(output);dest.parent.mkdir(parents=True,exist_ok=True)
    dest.write_bytes(header+b'\0'*((-len(header))%32)+b''.join(blocks))
    return hashlib.sha256(dest.read_bytes()).hexdigest()

if __name__=='__main__':
    root=Path(__file__).resolve().parents[1]
    print(convert(root/'src-tauri/resources/ggml-silero-v5.1.2.bin',root/'src-tauri/resources/nemo-runtime/silero-v5.1.2.gguf'))
