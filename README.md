# Innote

Innote là ứng dụng desktop Windows ghi âm cuộc họp và micro, chép lời trên máy, nghe lại, sửa và xuất Markdown/TXT/SRT. Không cần bot tham gia cuộc họp, không cần API key để dùng local, không gửi audio lên cloud mặc định.

> **Giấy phép:** PolyForm Noncommercial 1.0.0 — xem [LICENSE](LICENSE). Đây là **source-available**, không phải "open source" theo định nghĩa OSI, vì mục đích thương mại bị hạn chế. Muốn dùng thương mại, liên hệ Alex Linh để có giấy phép riêng.

## Tính năng

- Ghi đồng thời **âm thanh cuộc họp** (WASAPI loopback) và **micro**.
- Chép lời local bằng Whisper large-v3-turbo cho Việt/Anh; Nemotron cho Việt/Anh/Hàn/Trung xen kẽ.
- Chọn ngôn ngữ giao diện: Tiếng Việt, English, 한국어, 中文（简体）.
- Nghe lại, sửa từng đoạn, ghi chú, tìm kiếm và xuất `.md`, `.txt`, `.srt`; tải thêm bản ghi âm `.mp3`.
- Tự thông báo và cập nhật khi có bản mới qua GitHub Releases.
- Dữ liệu lưu tại `%LOCALAPPDATA%\Innote`, không tự gửi lên cloud.
- Soniox là lựa chọn tắt mặc định; nếu bật, audio phiên đó được gửi tới Soniox và có thể tính phí.

## Tải và cài đặt

### 1. Tải đúng file

1. Mở trang **Releases**: [github.com/quangtu07/innote/releases](https://github.com/quangtu07/innote/releases).
2. Tải file có dạng:

   ```text
   Innote-Setup-<version>.exe
   ```

3. Chỉ tải từ trang Releases chính thức ở trên. Không dùng file từ mirror, nhóm chat hoặc nguồn không rõ.
4. Không cần tải riêng các file sau để cài:

   ```text
   Innote-Setup-<version>.exe.sig
   latest.json
   ```

   Hai file đó chỉ dùng cho cơ chế tự cập nhật của app, không phải file cài và không cần mở.

Nếu muốn app tự thông báo và cập nhật bản mới, hãy dùng bộ cài `.exe`; bản portable/thư mục giải nén không phù hợp để thử luồng tự cập nhật.

### 2. Chạy bộ cài

1. Nhấp đúp file `Innote-Setup-<version>.exe`.
2. Nếu Windows hiện cảnh báo SmartScreen, xem mục **Windows SmartScreen** ngay bên dưới.
3. Làm theo hướng dẫn của bộ cài và hoàn tất.
4. Mở **Innote**, chọn ngôn ngữ giao diện ở lần chạy đầu.

Bộ cài hiện cài cho người dùng Windows hiện tại, thường không cần quyền quản trị. Nếu Windows hoặc WebView2 yêu cầu quyền nâng cao, chỉ chấp nhận khi bạn chắc chắn file được tải từ Releases chính thức.

### 3. Lần đầu sử dụng

1. Chọn **Âm thanh cuộc họp** và/hoặc **Micro của bạn**. Nên dùng tai nghe để giảm vọng.
2. Chọn ngôn ngữ nhận dạng.
3. Tải model khi app hỏi. Model **không nằm trong bộ cài**; app tải một lần qua Internet rồi lưu trên máy.
4. Sau khi model đã có, bấm **Bắt đầu ghi**.

## Windows SmartScreen

Bản cài hiện **chưa ký Authenticode**. Đây là chữ ký số của Windows dùng để xác nhận nhà phát hành với hệ điều hành. Vì chưa có chứng chỉ này, Windows có thể hiện:

```text
Windows protected your PC
Microsoft Defender SmartScreen prevented an unrecognized app from starting.
```

hoặc bản tiếng Việt:

```text
Windows đã bảo vệ PC của bạn
```

Kèm theo:

```text
Unknown publisher
```

### Cảnh báo này có phải file có virus không?

Không. SmartScreen đang nói rằng file **chưa được ký số** và chưa có đủ uy tín trong hệ thống của Microsoft. Nó không kết luận file sạch hay nhiễm virus.

Tuy vậy, bạn vẫn nên thận trọng. Chỉ tiếp tục nếu:

1. File được tải từ đúng trang:
   [github.com/quangtu07/innote/releases](https://github.com/quangtu07/innote/releases)
2. Bạn biết đây là ứng dụng **Innote**.
3. Nếu release có công bố SHA-256, bạn đã đối chiếu và khớp.

### Cách chạy nếu bạn tin file

Trên hộp thoại SmartScreen:

1. Bấm **More info** — **Thông tin thêm**.
2. Kiểm tra tên file là `Innote-Setup-...exe`.
3. Bấm **Run anyway** — **Vẫn chạy**.

Không bấm chạy nếu tên file, nguồn tải hoặc nhà phát hành hiển thị không giống mô tả.

### Điều không nên làm

- Không tắt toàn bộ **Microsoft Defender** hoặc **SmartScreen** chỉ để cài một file.
- Không bấm **Run anyway** với file nhận từ người lạ, nhóm chat hoặc link rút gọn.
- Không bỏ qua cảnh báo của trình duyệt nếu file không đến từ GitHub Releases chính thức.

Nếu bạn không chắc, hãy xóa file và tải lại từ Releases. Không có cách nào để người dùng tự kiểm tra đầy đủ file unsigned chỉ bằng cách nhìn tên file.

### Kiểm tra SHA-256 nếu cần

Nếu release có công bố mã SHA-256, mở PowerShell tại thư mục chứa file:

```powershell
Get-ChildItem .\Innote-Setup-*.exe | Get-FileHash -Algorithm SHA256
```

So sánh kết quả với mã SHA-256 trên trang release. Nếu khác, **không chạy file** và tải lại.

### File `.sig` có làm hết cảnh báo không?

Không. File `.sig` trong release là chữ ký của **Tauri updater**, dùng để app kiểm tra gói cập nhật có đúng từ khóa của nhà phát hành hay không. Nó không phải chữ ký Authenticode của Windows, nên không làm SmartScreen biến mất.

Hiện tại cảnh báo SmartScreen là hành vi bình thường của bản cài chưa ký. Khi Innote có chứng chỉ Authenticode, cảnh báo có thể giảm, nhưng Microsoft vẫn có thể cần thời gian để tích lũy uy tín — không có bảo đảm hết cảnh báo ngay.

## Dùng nhanh

1. Chọn **Âm thanh cuộc họp** và/hoặc **Micro của bạn**. Nên dùng tai nghe để giảm vọng.
2. Chọn ngôn ngữ nhận dạng. Với cuộc họp xen kẽ, chọn **Đa ngôn ngữ** và tải model Nemotron khoảng 742 MB một lần.
3. Bấm **Bắt đầu ghi**. Chữ trực tiếp hiện khi đang nói; bản đã chốt được lưu trên máy.
4. Kết thúc, nghe lại, sửa nếu cần, ghi chú và xuất file. Có thể tải bản chép `.md`/`.txt`/`.srt` và bản ghi âm `.mp3`.
5. File xuất mới nằm trong `Tải xuống\Innote`. File `.md`/`.txt` có phần liên hệ tác giả ở cuối.

## Yêu cầu và dung lượng

### Hệ điều hành và phần cứng

- **Windows 10 hoặc Windows 11, bản x64**. Bộ cài hiện không hỗ trợ Windows 32-bit.
- **CPU hỗ trợ AVX2** để chép lời local. Nếu CPU không có AVX2, app vẫn có thể ghi âm nhưng không chép lời local được.
- **WebView2 Runtime**. Windows 11 thường có sẵn; Windows 10 cũng thường có theo Microsoft Edge. Nếu máy chưa có, bộ cài sẽ thử tải và cài WebView2 tự động, cần Internet.

Nếu bộ cài không cài được WebView2 do mạng hoặc chính sách công ty, có thể cài WebView2 Runtime từ trang chính thức của Microsoft:

[https://developer.microsoft.com/microsoft-edge/webview2/](https://developer.microsoft.com/microsoft-edge/webview2/)

### Dung lượng cần chuẩn bị

Bộ cài và app chiếm khoảng **140 MB** sau khi cài. Model được tải riêng:

| Model | Dung lượng |
|---|---:|
| Whisper large-v3-turbo | khoảng **574 MB** |
| Nemotron đa ngôn ngữ | khoảng **742 MB** |
| Cả hai model | khoảng **1,3 GB** |

Nên để trống:

- Khoảng **1 GB** nếu chỉ dùng Whisper.
- Khoảng **1,2 GB** nếu chỉ dùng Nemotron.
- Khoảng **2 GB** nếu muốn dùng cả hai, cộng thêm dung lượng cho audio các phiên họp.

Model lưu tại:

```text
%LOCALAPPDATA%\Innote\models
```

### Internet

- Cần Internet khi cài nếu máy chưa có WebView2.
- Cần Internet khi tải model lần đầu.
- Sau khi WebView2 và model đã có, app có thể ghi âm/chép lời local mà không cần Internet.
- Kiểm tra cập nhật và Soniox opt-in cần Internet.

### Không cần cài thêm

Người dùng bản đã cài **không cần**:

```text
Python
Node.js
Rust
Visual Studio Build Tools
```

Các runtime C++ cần thiết đã được đóng gói cùng app. Python/Node/Rust chỉ cần khi build từ source.

## Câu hỏi thường gặp khi cài

### “Unknown publisher” là gì?

Windows chưa nhận diện được nhà phát hành vì bản cài chưa ký Authenticode. Đây không phải thông báo app chắc chắn có virus. Chỉ tiếp tục nếu file được tải từ Releases chính thức.

### Tôi có nên tắt SmartScreen/Defender để cài không?

Không. Chỉ bấm **More info → Run anyway** cho file bạn chắc chắn đã tải từ trang chính thức. Không tắt bảo vệ toàn hệ thống.

### Tôi có cần cài WebView2 thủ công không?

Thường không. Bộ cài sẽ tự xử lý nếu máy thiếu WebView2 và có Internet. Nếu mạng/chính sách công ty chặn, cài WebView2 Runtime từ trang Microsoft ở trên.

### Model có nằm trong bộ cài không?

Không. App tải model khi bạn chọn model lần đầu. Model không tự tải nếu bạn chưa xác nhận trong app.

### Có dùng offline được không?

Được, sau khi WebView2 và model đã có trên máy. App không tự gửi audio lên cloud ở chế độ local.

### Cài app có cần quyền admin không?

Bộ cài hiện cài cho người dùng hiện tại, thường không cần admin. Nếu Windows/WebView2 yêu cầu nâng quyền, chỉ chấp nhận khi file đến từ Releases chính thức.

### Sau khi cập nhật, shortcut vẫn hiện logo cũ?

Windows có thể giữ icon cache của shortcut cũ. Hãy đóng Innote, unpin shortcut cũ, mở Innote từ Start menu rồi pin lại; hoặc restart Explorer. Việc này không ảnh hưởng dữ liệu hay phiên họp.

### Tôi tải nhầm file `.sig` hoặc `latest.json` thì sao?

Hai file đó không phải bộ cài. `.sig` dùng cho updater, `latest.json` là manifest cập nhật. Hãy quay lại Releases và tải file `Innote-Setup-<version>.exe`.

## Dữ liệu và quyền riêng tư

- Phiên ghi, PCM/WAV, SQLite, model nằm trong `%LOCALAPPDATA%\Innote`.
- Không có telemetry, không tự upload audio.
- Xóa app không tự xóa dữ liệu người dùng; muốn xóa hoàn toàn phải xóa thư mục dữ liệu.
- Soniox chỉ dùng khi người dùng chủ động chọn và nhập key; key không lưu xuống đĩa.

## Build từ source

Cần Windows x64, Node.js 20+, Rust 1.90+, Visual Studio 2022 Build Tools (C++), CMake, Python 3 và `libclang.dll` (đặt `LIBCLANG_PATH`).

```powershell
git clone https://github.com/quangtu07/innote.git
cd innote
npm ci
python scripts/prepare-moonshine.py
python scripts/prepare-streaming.py
python scripts/prepare-nemo.py
```

Các script trên tải runtime/model công khai theo checksum. Repo không chứa model lớn, runtime native, bản release hay dữ liệu test. Giữ `src-tauri/resources/licenses/`, `runtime/` và `ggml-silero-v5.1.2.bin` trong repo; các thư mục model/runtime lớn được tạo bởi các script chuẩn bị.

Build thử:

```powershell
npm run check
cargo test --manifest-path src-tauri/Cargo.toml --locked
npm run dev
npm run package -- --bundles nsis
```

Bộ cài NSIS nằm trong `src-tauri\target\release\bundle\nsis\`.

## Phát hành và tự cập nhật

### 1. Tạo khóa ký (chỉ làm một lần)

Nếu chưa có khóa:

```powershell
npm run tauri signer generate -- -w $env:USERPROFILE\.tauri\innote.key
```

Public key đã có trong `src-tauri/tauri.conf.json`. Sao lưu private key + mật khẩu ngoài repo. Mất khóa/mật khẩu thì người đã cài sẽ không nhận được cập nhật.

### 2. Build và tạo 3 file phát hành bằng một lệnh

```powershell
.\scripts\release.ps1 `
  -Repo USERNAME/innote `
  -Version 0.6.3 `
  -Notes "Innote 0.6.3: mô tả ngắn thay đổi."
```

Script tự:

- trỏ updater về `https://github.com/USERNAME/innote/releases/latest/download/latest.json`
- build binary + NSIS
- ký bằng `%USERPROFILE%\.tauri\innote.key`
- tạo `release\Innote-Setup-0.6.3.exe`
- tạo `release\Innote-Setup-0.6.3.exe.sig`
- tạo `release\latest.json`

Sau đó commit cấu hình endpoint:

```powershell
git add src-tauri\tauri.conf.json
git commit -m "Configure GitHub updater endpoint"
```

### 3. Tạo GitHub Release

Tạo tag `v0.6.3`, rồi upload đúng **3 file**:

1. `release\Innote-Setup-0.6.3.exe`
2. `release\Innote-Setup-0.6.3.exe.sig`
3. `release\latest.json`

Không upload source/build folder vào release. Endpoint `releases/latest/download/latest.json` sẽ được app đọc ở lần kiểm tra sau.

Trong app, sau khoảng 5 giây sẽ kiểm tra nền. Khi có bản mới, banner hiện **Đã có bản Innote x.y.z** với **Cập nhật ngay** / **Để sau**. Nút cập nhật bị chặn khi đang ghi âm, đang thu thử hoặc đang sửa bản chép.

> **Quan trọng:** bản 0.6.1 chưa có updater. Người dùng đang cài 0.6.1 phải tải và cài thủ công một lần (0.6.2 hoặc mới hơn); từ 0.6.2 trở đi mới nhận thông báo cập nhật.

Người dùng cuối chỉ cần tải file `.exe`; không cần Python, Node, Git hay PowerShell.

## Giấy phép và bên thứ ba

- Mã nguồn Innote: **PolyForm Noncommercial 1.0.0**, cho phép dùng/sửa/chia sẻ phi thương mại. Không phải giấy phép OSI open source.
- Dùng thương mại: liên hệ **Alex Linh** qua website/Zalo trong phần liên hệ.
- Model, runtime và thư viện bên thứ ba giữ giấy phép riêng; xem [THIRD_PARTY_NOTICES.md](THIRD_PARTY_NOTICES.md) và `src-tauri/resources/licenses/`.
- Không được dùng tên/model của bên thứ ba để ngụ ý họ bảo trợ Innote.

## Đóng góp

Trước khi gửi thay đổi, chạy:

```powershell
npm run check
cargo test --manifest-path src-tauri/Cargo.toml --locked
```

Không commit audio cuộc họp, database, `test-results/`, `release/`, `.tools/`, model weights, khóa ký hoặc thông tin cá nhân. Các file đó đã có trong `.gitignore`.

## Liên hệ

**Alex Linh** — Tư vấn, hỗ trợ & lập trình  
Website: https://alexlinh.io.vn/  
Zalo: 0962 239 818