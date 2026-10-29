"""Developer-only paced streaming benchmark. No network or private audio.

Research runtimes/models stay in .tools; this does not change the desktop app.
wall time starts after model loading; decode time is not end-to-end word latency.
"""
import argparse
import ctypes as c
import json
import os
from pathlib import Path
import statistics
import sys
import time
import wave

sys.stdout.reconfigure(encoding="utf-8")
parser = argparse.ArgumentParser()
parser.add_argument("--engine", choices=["nemo", "sherpa", "moonshine"], required=True)
parser.add_argument("--model", required=True)
parser.add_argument("--runtime", required=True)
parser.add_argument("--audio", default="test-results/fixtures/vietnamese-pcm.wav")
parser.add_argument("--output", required=True)
parser.add_argument("--right-context", type=int, default=0)
parser.add_argument("--frame-ms", type=int, default=20)
parser.add_argument("--threads", type=int, default=2)
parser.add_argument("--limit-seconds", type=float, default=0)
parser.add_argument("--language", default="vi-VN")
parser.add_argument("--endpoint-ms", type=int, default=0)
parser.add_argument("--restart-on-endpoint", action="store_true")
parser.add_argument("--prefix-silence", type=float, default=0.)
parser.add_argument("--replay-ms", type=int, default=0)
parser.add_argument("--vad-model")
parser.add_argument("--vad-based-eou", action="store_true")
parser.add_argument("--unpaced", action="store_true")
parser.add_argument("--gpu", type=int, default=-1)
parser.add_argument("--warmup-seconds", type=float, default=1)
parser.add_argument("--blank-penalty", type=float, default=0)
parser.add_argument("--update-interval", type=float, default=.1)
parser.add_argument("--vad-window", type=float, default=.5)
parser.add_argument("--no-speculative", action="store_true")
parser.add_argument("--manual-update", action="store_true")
parser.add_argument("--adaptive-update", action="store_true")
args = parser.parse_args()
model = Path(args.model).resolve()
runtime = Path(args.runtime).resolve()
output = Path(args.output).resolve()
output.parent.mkdir(parents=True, exist_ok=True)

class Nemo:
    def __init__(self):
        self.directory = os.add_dll_directory(str(runtime))
        self.d = d = c.CDLL(str(runtime / "nemo_speech_asr_c.dll"))
        class Backend(c.Structure):
            _fields_ = [("size",c.c_size_t),("gpu",c.c_int32)]
        class Model(c.Structure):
            _fields_ = [("size",c.c_size_t),("path",c.c_char_p),("name",c.c_char_p)]
        class Streaming(c.Structure):
            _fields_ = [("size",c.c_size_t),("chunk",c.c_float),("left",c.c_float),("right",c.c_float),("rnnt_context",c.c_int32)]
        class Endpoint(c.Structure):
            _fields_=[("size",c.c_size_t),("enable",c.c_bool),("vad_based",c.c_bool),("silence_ms",c.c_int32)]
        class Vad(c.Structure):
            _fields_=[("size",c.c_size_t),("path",c.c_char_p),("mask",c.c_bool),("onset",c.c_float),("offset",c.c_float)]
        class Config(c.Structure):
            _fields_ = [("size",c.c_size_t)]+[(name,c.c_void_p) for name in ["backend","model","streaming","decoder","vad","endpointing","postproc","diar","batching"]]
        class Options(c.Structure):
            _fields_ = [("size",c.c_size_t),("request_id",c.c_char_p),("language",c.c_char_p),
                ("interim",c.c_bool),("word_times",c.c_bool),("punctuation",c.c_bool),("verbatim",c.c_bool),("profanity",c.c_bool),
                ("eou",c.c_int32),("speech_context",c.c_void_p),("context_count",c.c_size_t),("max_alternatives",c.c_int32),
                ("diarize",c.c_bool),("max_speakers",c.c_int32)]
        def bind(name, parameters, result):
            f=getattr(d,"nemo_speech_asr_"+name);f.argtypes=parameters;f.restype=result;return f
        self.error=bind("last_error",[],c.c_char_p)
        self.destroy=bind("destroy",[c.c_void_p],None)
        create=bind("create",[c.POINTER(Config),c.POINTER(c.c_void_p)],c.c_int)
        defaults=bind("recognition_options_default",[],Options)
        start=bind("streaming_recognize",[c.c_void_p,c.POINTER(Options),c.POINTER(c.c_void_p)],c.c_int)
        self.push=bind("stream_push_f32",[c.c_void_p,c.POINTER(c.c_float),c.c_size_t,c.c_int32],c.c_int)
        self.next=bind("stream_next",[c.c_void_p,c.POINTER(c.c_void_p)],c.c_int)
        self.finish_stream=bind("stream_finish",[c.c_void_p],c.c_int)
        self.close_stream=bind("stream_close",[c.c_void_p],None)
        self.text=bind("result_transcript",[c.c_void_p,c.c_size_t],c.c_char_p)
        self.is_final=bind("result_is_final",[c.c_void_p],c.c_bool)
        self.processed=bind("result_audio_processed",[c.c_void_p],c.c_float)
        self.free_result=bind("result_destroy",[c.c_void_p],None)
        self.language_count=bind("result_language_count",[c.c_void_p,c.c_size_t],c.c_size_t)
        self.language_code=bind("result_language_code",[c.c_void_p,c.c_size_t,c.c_size_t],c.c_char_p)
        self.word_count=bind("result_word_count",[c.c_void_p,c.c_size_t],c.c_size_t)
        self.word_text=bind("result_word_text",[c.c_void_p,c.c_size_t,c.c_size_t],c.c_char_p)
        self.word_start=bind("result_word_start_time",[c.c_void_p,c.c_size_t,c.c_size_t],c.c_int32)
        self.word_end=bind("result_word_end_time",[c.c_void_p,c.c_size_t,c.c_size_t],c.c_int32)
        backend=Backend(c.sizeof(Backend),args.gpu)
        cfg_model=Model(c.sizeof(Model),str(model).encode(),None)
        streaming=Streaming(c.sizeof(Streaming),.16,1.92,1.92,args.right_context)
        cfg=Config();cfg.size=c.sizeof(Config)
        cfg.backend=c.cast(c.pointer(backend),c.c_void_p)
        cfg.model=c.cast(c.pointer(cfg_model),c.c_void_p)
        cfg.streaming=c.cast(c.pointer(streaming),c.c_void_p)
        endpoint=Endpoint(c.sizeof(Endpoint),True,args.vad_based_eou,args.endpoint_ms)
        if args.endpoint_ms:cfg.endpointing=c.cast(c.pointer(endpoint),c.c_void_p)
        if args.vad_model:
            vad=Vad(c.sizeof(Vad),str(Path(args.vad_model).resolve()).encode(),False,.5,.35)
            cfg.vad=c.cast(c.pointer(vad),c.c_void_p)
        self.committed=[];self.tail=[]
        self.recognizer=c.c_void_p()
        self.check(create(c.byref(cfg),c.byref(self.recognizer)))
        opts=defaults()
        assert opts.size==c.sizeof(Options),(opts.size,c.sizeof(Options))
        opts.language=args.language.encode();opts.interim=True;opts.word_times=True
        self.stream=c.c_void_p()
        self.start_stream=start;self.opts=opts;self.stream_samples=0;self.base_samples=0;self.finishing=False
        self.check(start(self.recognizer,c.byref(opts),c.byref(self.stream)))
        if args.warmup_seconds:
            self.accept([0.] * int(args.warmup_seconds * 16000))
            self.finish()
            self.close_stream(self.stream)
            self.stream=c.c_void_p()
            self.check(start(self.recognizer,c.byref(opts),c.byref(self.stream)))
            self.stream_samples=0;self.base_samples=0;self.finishing=False;self.committed=[]
    def check(self,status):
        if status:raise RuntimeError((self.error() or b"unknown error").decode(errors="replace"))
    def accept(self, samples):
        if args.replay_ms:
            self.tail=(self.tail+samples)[-round(args.replay_ms*16):]
        self.stream_samples+=len(samples)
        frame=(c.c_float*len(samples))(*samples)
        self.check(self.push(self.stream,frame,len(samples),16000))
        return self.pull()
    def pull(self):
        rows=[];final_seen=False
        for _ in range(1000):
            result=c.c_void_p();self.check(self.next(self.stream,c.byref(result)))
            if not result:break
            try:
                rows.append({"text":(self.text(result,0) or b"").decode(),"final":bool(self.is_final(result)),"processed_s":self.processed(result),"languages":[(self.language_code(result,0,i) or b"one").decode() for i in range(self.language_count(result,0))],
                "words":[{"text":(self.word_text(result,0,i) or b'').decode(),"start_ms":self.word_start(result,0,i),"end_ms":self.word_end(result,0,i)} for i in range(self.word_count(result,0))]})
                if args.endpoint_ms:
                    if rows[-1]['final']:final_seen=True
                    raw_text=rows[-1]['text']
                    rows[-1]['text']=' '.join(self.committed+([raw_text] if raw_text else []))
                    if rows[-1]['final'] and raw_text:self.committed.append(raw_text)
            finally:self.free_result(result)
        else:raise RuntimeError("Result loop did not drain")
        if final_seen and args.restart_on_endpoint and not self.finishing:
            self.close_stream(self.stream)
            self.base_samples+=self.stream_samples;self.stream_samples=0
            self.stream=c.c_void_p()
            self.check(self.start_stream(self.recognizer,c.byref(self.opts),c.byref(self.stream)))
            if args.replay_ms and self.tail:
                tail=(c.c_float*len(self.tail))(*self.tail)
                self.check(self.push(self.stream,tail,len(self.tail),16000))
            if args.prefix_silence:
                size=round(args.prefix_silence*16000);zeros=(c.c_float*size)()
                self.check(self.push(self.stream,zeros,size,16000))
        return rows
    def finish(self):
        self.finishing=True
        self.check(self.finish_stream(self.stream));return self.pull()
    def close(self):
        self.close_stream(self.stream);self.destroy(self.recognizer)

class Sherpa:
    def __init__(self):
        sys.path.insert(0,str(runtime))
        import sherpa_onnx
        import sentencepiece as spm
        tokens=model/'tokens.txt'
        if not tokens.exists():
            sp=spm.SentencePieceProcessor(model_file=str(model/'bpe.model'))
            tokens.write_text(''.join(f'{sp.id_to_piece(i)} {i}\n' for i in range(sp.get_piece_size())),encoding='utf-8')
        self.recognizer=sherpa_onnx.OnlineRecognizer.from_transducer(tokens=str(tokens),encoder=str(model/'encoder.onnx'),decoder=str(model/'decoder.onnx'),joiner=str(model/'joiner.onnx'),num_threads=args.threads,sample_rate=16000,feature_dim=80,decoding_method='greedy_search',model_type='zipformer2',provider='cpu',enable_endpoint_detection=False,blank_penalty=args.blank_penalty)
        self.stream=self.recognizer.create_stream()
        if args.warmup_seconds:
            self.accept([0.] * int(args.warmup_seconds * 16000))
            self.finish()
            self.stream=self.recognizer.create_stream()
    def accept(self,samples):
        self.stream.accept_waveform(16000,samples)
        while self.recognizer.is_ready(self.stream):self.recognizer.decode_stream(self.stream)
        return [{"text":self.recognizer.get_result(self.stream),"final":False}]
    def finish(self):
        self.stream.accept_waveform(16000,[0.]*8000)
        self.stream.input_finished()
        while self.recognizer.is_ready(self.stream):self.recognizer.decode_stream(self.stream)
        return [{"text":self.recognizer.get_result(self.stream),"final":True}]
    def close(self):pass

class Moonshine:
    def __init__(self):
        sys.path.insert(0,str(runtime))
        from moonshine_voice import Transcriber,ModelArch
        self.transcriber=Transcriber(str(model),ModelArch.TINY_STREAMING,update_interval=args.update_interval,
            options={"vad_window_duration":str(args.vad_window),"transcription_interval":str(args.update_interval),"return_audio_data":"false","log_output_text":"false","use_speculative_decoding":"false" if args.no_speculative else "true"})
        self.lines={}
        self.samples_since_update=0
        self.next_update_samples=round(args.update_interval*16000)
        self.pending_cost=0.
        self.stream=self.transcriber.create_stream(update_interval=3600 if args.manual_update else args.update_interval)
        def on_event(event):
            if hasattr(event,'error'):raise event.error
            self.lines[event.line.line_id]=event.line
        self.stream.add_listener(on_event)
        self.stream.start()
    def accept(self,samples):
        tick=time.perf_counter()
        self.stream.add_audio(samples,16000)
        if args.manual_update:
            self.samples_since_update+=len(samples)
            if self.samples_since_update>=self.next_update_samples:
                self.stream.update_transcription(flags=1)
                self.samples_since_update=0
                if args.adaptive_update:
                    import math
                    target=max(1280,min(5120,math.ceil(((self.pending_cost+time.perf_counter()-tick)*16000*1.2+160)/320)*320))
                    self.next_update_samples=math.ceil(max(target,self.next_update_samples*.75)/320)*320
                self.pending_cost=0.
            else:self.pending_cost+=time.perf_counter()-tick
        return [{"text":" ".join(line.text for line in self.lines.values()).strip(),"final":False}]
    def finish(self):
        transcript=self.stream.stop()
        if transcript:
            for line in transcript.lines:self.lines[line.line_id]=line
        return [{"text":" ".join(line.text for line in self.lines.values()).strip(),"final":True}]
    def close(self):
        self.stream.close()
        self.transcriber.close()

with wave.open(args.audio) as audio:
    assert audio.getframerate()==16000 and audio.getnchannels()==1 and audio.getsampwidth()==2
    import array
    pcm=array.array('h',audio.readframes(audio.getnframes()))
if args.limit_seconds:pcm=pcm[:int(args.limit_seconds*16000)]
load=time.perf_counter()
engine={"nemo":Nemo,"sherpa":Sherpa,"moonshine":Moonshine}[args.engine]()
load_ms=(time.perf_counter()-load)*1000
print(json.dumps({'loaded_ms':round(load_ms,2),'engine':args.engine}),flush=True)
start=time.perf_counter(); rows=[]; decode=[]; previous=''; frame_size=16*args.frame_ms
try:
    for at in range(0,len(pcm),frame_size):
        frame=pcm[at:at+frame_size]
        audio_ms=(at+len(frame))/16
        if not args.unpaced:time.sleep(max(0,start+audio_ms/1000-time.perf_counter()))
        tick=time.perf_counter()
        updates=engine.accept([x/32768 for x in frame])
        cost=(time.perf_counter()-tick)*1000;decode.append(cost)
        for update in updates:
            if update['text']!=previous:
                previous=update['text']; row={"wall_ms":round((time.perf_counter()-start)*1000,2),"audio_ms":audio_ms,"decode_ms":round(cost,2),**update};rows.append(row);print(json.dumps(row,ensure_ascii=False),flush=True)
    tick=time.perf_counter()
    for update in engine.finish():
        rows.append({"wall_ms":round((time.perf_counter()-start)*1000,2),"audio_ms":len(pcm)/16,"decode_ms":round((time.perf_counter()-tick)*1000,2),**update})
finally:engine.close()
text_rows=[x for x in rows if x['text']]
percentile=lambda values,p: sorted(values)[min(len(values)-1,int((len(values)-1)*p))] if values else None
summary={'engine':args.engine,'vad_window':args.vad_window,'speculative':not args.no_speculative,'manual_update':args.manual_update,'adaptive_update':args.adaptive_update,'update_interval':args.update_interval,'model':str(model),'right_context':args.right_context,'language':args.language,'endpoint_ms':args.endpoint_ms,'restart_on_endpoint':args.restart_on_endpoint,'prefix_silence':args.prefix_silence,'frame_ms':args.frame_ms,'threads':args.threads,'blank_penalty':args.blank_penalty,'gpu':args.gpu,'warmup_seconds':args.warmup_seconds,'load_ms':load_ms,'audio_s':len(pcm)/16000,'wall_s':time.perf_counter()-start,'first_text_ms':text_rows[0]['wall_ms'] if text_rows else None,'decode_p50_ms':percentile(decode,.5),'decode_p95_ms':percentile(decode,.95),'decode_max_ms':max(decode,default=0),'max_backlog_ms':max((x['wall_ms']-x['audio_ms'] for x in rows),default=0),'transcript':rows[-1]['text'] if rows else '', 'paced':not args.unpaced}
output.write_text(json.dumps({'summary':summary,'rows':rows},ensure_ascii=False,indent=2),encoding='utf-8')
print(json.dumps(summary,ensure_ascii=False),flush=True)
