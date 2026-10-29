"""Copy only the configured bundle resources alongside the built exe.
Fails if output exists, so an in-use release cannot be silently overwritten.
"""
import argparse, json, shutil
from pathlib import Path
p=argparse.ArgumentParser();p.add_argument('--output',required=True);a=p.parse_args()
root=Path(__file__).resolve().parents[1];base=root/'src-tauri'
config=json.loads((base/'tauri.conf.json').read_text(encoding='utf-8'))
out=Path(a.output).resolve();out.mkdir(parents=True,exist_ok=False)
shutil.copy2(base/'target/release/innote.exe',out/'innote.exe')
for source,target in config['bundle']['resources'].items():
    source=(base/source).resolve();target=out/target
    if source.is_dir():shutil.copytree(source,target,dirs_exist_ok=True)
    else:target.parent.mkdir(parents=True,exist_ok=True);shutil.copy2(source,target)
(out/'HUONG-DAN.txt').write_text('Innote '+config['version']+'\nGiữ toàn bộ thư mục, mở innote.exe. Không kéo riêng exe khỏi DLL/models/licenses. Windows x64, WebView2, CPU AVX2. Preview Việt/Anh miễn phí đã kèm theo; tải model Whisper cho bước hiệu chỉnh. Chọn Đa ngôn ngữ tự động và tải model khoảng742MB một lần để nói xen kẽ Việt/Anh/Hàn/Trung theo lượt. Chế độ đa ngôn ngữ chạy local, chưa tách người nói; đổi giữa câu hoặc nói chồng có thể sai. Soniox tùy chọn cho Việt/Anh. Bản thử chưa cam kết mọi từ dưới 500 ms.\n',encoding='utf-8-sig')
print(str(out))
