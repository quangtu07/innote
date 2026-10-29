# Innote

Innote là ứng dụng desktop Windows ghi âm cuộc họp và micro, chép lời trên máy, nghe lại, sửa và xuất Markdown/TXT/SRT. Không cần bot tham gia cuộc họp, không cần API key để dùng local, không gửi audio lên cloud mặc định.

> **Giấy phép:** PolyForm Noncommercial 1.0.0 — xem [LICENSE](LICENSE). Đây là **source-available**, không phải "open source" theo định nghĩa OSI, vì mục đích thương mại bị hạn chế. Muốn dùng thương mại, liên hệ Alex Linh để có giấy phép riêng.

## Tính năng

- Ghi đồng thời **âm thanh cuộc họp** (WASAPI loopback) và **micro**.
- Chép lời local bằng Whisper large-v3-turbo cho Việt/Anh; Nemotron cho Việt/Anh/Hàn/Trung xen kẽ.
- Chọn ngôn ngữ giao diện: Tiếng Việt, English, 한국어, 中文（简体）.
- Nghe lại, sửa từng đoạn, ghi chú, tìm kiếm và xuất `.md`, `.txt`, `.srt`.
- Tự thông báo và cập nhật khi có bản mới qua GitHub Releases.
- Dữ liệu lưu tại `%LOCALAPPDATA%\Innote`, không tự gửi lên cloud.
- Soniox là lựa chọn tắt mặc định; nếu bật, audio phiên đó được gửi tới Soniox và có thể tính phí.

## Tải và cài đặt

1. Mở trang **Releases** của repository.
2. Tải `Innote-Setup-<version>.exe` (không tải riêng file `.exe` trần hoặc thư mục portable nếu muốn tự cập nhật).
3. Chạy bộ cài, chọn thư mục cài và hoàn tất.
4. Mở **Innote**, chọn ngôn ngữ giao diện ở lần chạy đầu.
5. Chọn nguồn âm thanh, tải model một lần nếu cần, rồi bấm **Bắt đầu ghi**.

Bản cài hiện chưa ký Authenticode, nên Windows có thể hiện SmartScreen. Chỉ tải file từ trang Releases chính thức; không tắt bảo vệ hệ thống để chạy file không rõ nguồn.

## Dùng nhanh

1. Chọn **Âm thanh cuộc họp** và/hoặc **Micro của bạn**. Nên dùng tai nghe để giảm vọng.
2. Chọn ngôn ngữ nhận dạng. Với cuộc họp xen kẽ, chọn **Đa ngôn ngữ** và tải model Nemotron khoảng 742 MB một lần.
3. Bấm **Bắt đầu ghi**. Chữ trực tiếp hiện khi đang nói; bản đã chốt được lưu trên máy.
4. Kết thúc, nghe lại, sửa nếu cần, ghi chú và xuất file.
5. File xuất mới nằm trong `Tải xuống\Innote`. File `.md`/`.txt` có phần liên hệ tác giả ở cuối.

## Yêu cầu

- Windows 10/11 x64, WebView2 và CPU hỗ trợ AVX2.
- Khoảng trống cho model: Whisper khoảng 574 MB hoặc Nemotron khoảng 742 MB.
- Không cần Python/Node để chạy bản đã cài. Python/Node/Rust chỉ cần khi build từ source.

## Dữ liệu và quyền riêng tư

- Phiên ghi, PCM/WAV, SQLite, model nằm trong `%LOCALAPPDATA%\Innote`.
- Không có telemetry, không tự upload audio.
- Xóa app không tự xóa dữ liệu người dùng; muốn xóa hoàn toàn phải xóa thư mục dữ liệu.
- Soniox chỉ dùng khi người dùng chủ động chọn và nhập key; key không lưu xuống đĩa.

## Build từ source

Cần Windows x64, Node.js 20+, Rust 1.90+, Visual Studio 2022 Build Tools (C++), CMake, Python 3 và `libclang.dll` (đặt `LIBCLANG_PATH`).

```powershell
git clone https://github.com/OWNER/innote.git
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

### 1. Trỏ updater về repository

Sau khi tạo repository GitHub, chạy một lần:

```powershell
python scripts/configure-github.py --repo OWNER/innote
```

Lệnh này đổi endpoint trong `src-tauri/tauri.conf.json`; chạy **trước khi build** vì endpoint được nhúng vào app.

```text
https://github.com/OWNER/innote/releases/latest/download/latest.json
```

Repo và file release phải **public** để app có thể tải cập nhật mà không cần token.

### 2. Tạo khóa ký và giữ bí mật

```powershell
npm run tauri signer generate -- -w $env:USERPROFILE\.tauri\innote.key
```

Public key đã được đưa vào `src-tauri/tauri.conf.json`. Private key và mật khẩu phải được sao lưu ngoài repo. **Mất private key hoặc mật khẩu thì người dùng đã cài sẽ không nhận được bản cập nhật tiếp theo.**

Khi build bản phát hành:

```powershell
$env:TAURI_SIGNING_PRIVATE_KEY = (Get-Content "$env:USERPROFILE\.tauri\innote.key" -Raw)
$env:TAURI_SIGNING_PRIVATE_KEY_PASSWORD = "mật-khẩu-của-bạn"
npm run package -- --bundles nsis
```

Kết quả có file setup và signature trong thư mục bundle; đổi tên/copy sang tên phát hành nếu cần:

- `Innote-Setup-<version>.exe`
- `Innote-Setup-<version>.exe.sig`

### 3. Tạo `latest.json`

```powershell
python scripts/make-latest-json.py `
  --repo OWNER/innote `
  --version 0.6.2 `
  --installer "src-tauri\target\release\bundle\nsis\Innote_0.6.2_x64-setup.exe" `
  --signature "src-tauri\target\release\bundle\nsis\Innote_0.6.2_x64-setup.exe.sig" `
  --notes "Mô tả thay đổi" `
  --output "release\latest.json"
```

### 4. Tạo GitHub Release

Tạo tag `v0.6.2`, rồi upload đúng **3 file**:

1. `Innote-Setup-0.6.2.exe`
2. `Innote-Setup-0.6.2.exe.sig`
3. `latest.json`

Không upload source/build folder vào release. Endpoint `releases/latest/download/latest.json` sẽ được app đọc ở lần kiểm tra sau.

Trong app, sau khoảng 5 giây sẽ kiểm tra nền. Khi có bản mới, banner hiện **Đã có bản Innote x.y.z** với **Cập nhật ngay** / **Để sau**. Nút cập nhật bị chặn khi đang ghi âm, đang thu thử hoặc đang sửa bản chép.

> **Quan trọng:** bản 0.6.1 chưa có updater. Người dùng đang cài 0.6.1 phải tải và cài 0.6.2 thủ công một lần; từ 0.6.2 trở đi mới nhận thông báo cập nhật.

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