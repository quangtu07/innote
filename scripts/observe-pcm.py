"""Read-only availability probe for one explicitly selected local QA archive."""
import argparse,json,time,os,ctypes
from ctypes import wintypes
class Attr(ctypes.Structure):
 _fields_=[("attributes",wintypes.DWORD),("creation",wintypes.FILETIME),("access",wintypes.FILETIME),("write",wintypes.FILETIME),("high",wintypes.DWORD),("low",wintypes.DWORD)]
getattr_win=ctypes.WinDLL("kernel32",use_last_error=True).GetFileAttributesExW
getattr_win.argtypes=[wintypes.LPCWSTR,ctypes.c_int,ctypes.c_void_p];getattr_win.restype=wintypes.BOOL
from pathlib import Path
p=argparse.ArgumentParser();p.add_argument('--session-file',required=True);p.add_argument('--output',required=True);p.add_argument('--seconds',type=float,default=25);a=p.parse_args()
session=Path(Path(a.session_file).read_text(encoding='utf-8-sig').strip());rows=[];start=time.perf_counter();handle=None
while time.perf_counter()-start<a.seconds:
 path=session/'system.pcm'
 try:audio_ms=path.stat().st_size/32
 except FileNotFoundError:audio_ms=0
 if handle is None and path.exists():handle=path.open('rb')
 attr=Attr();ok=getattr_win(str(path),0,ctypes.byref(attr))
 rows.append({'unix_ms':time.time()*1000,'available_ms':audio_ms,'handle_ms':os.fstat(handle.fileno()).st_size/32 if handle else 0,'attributes_ms':((attr.high<<32)+attr.low)/32 if ok else 0});time.sleep(.01)
Path(a.output).write_text(json.dumps(rows),encoding='utf-8')
