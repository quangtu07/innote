# Chú giải thư mục và phân biệt các bản nội dung — 08/11/2026

## Thay đổi

- Giữ tên kỹ thuật ổn định cho audio/agent/index, không tạo thêm bản sao audio hoặc nhiều alias dễ nhầm.
- Thư viện mới có `00 - HƯỚNG DẪN THƯ VIỆN INNOTE.txt`; mỗi meeting có `00 - ĐỌC TÔI TRƯỚC.txt`, UTF-8 đọc được bằng Notepad.
- Hướng dẫn meeting nêu title, trạng thái tiếng Việt, mục đích từng file, dữ liệu nội bộ, nguồn tóm tắt và ĐÃ CÓ/CHƯA CÓ dựa trên file thực tế sau khi publish.
- Markdown gốc/corrected có nhãn BẢN CHÉP GỐC/BẢN HIỆU CHỈNH đầu file. Summary/notes có tiêu đề Tóm tắt/Ghi chú. SRT giữ đúng định dạng, không chèn dòng chú giải làm hỏng playback.
- UI có chú giải mở rộng trong Settings và meeting đã lưu, không mở thêm card hoặc làm dài setup mặc định. Phân biệt `.innote/` ở root library với `.innote/work/` trong từng meeting.
- Cuộc họp legacy được giải thích riêng: nội dung vẫn trong DB, chỉ WAV/PCM tại folder cũ; hướng dẫn xuất từ app, không báo giả có Markdown. Không tự ghi thêm file vào dữ liệu legacy.
- Startup chỉ bổ sung guide còn thiếu cho meeting modern, không publish lại/sửa các file đã có chỉ để cài hướng dẫn. Publish tiếp theo cập nhật guide và checksum manifest.

Tài liệu sử dụng: [Đọc thư mục cuộc họp](../meeting-files-vi.md).

## Kiểm tra

- Full Rust: **140 passed, 0 failed, 0 ignored**.
- Nhóm UI `Meeting storage`: **8/8 PASS**, gồm chú giải root/meeting ở 900×640 và 1180×800, legacy, xác nhận/cancel/restart/lỗi đọc/kết nối lại.
- Layout meeting: **3/3 PASS**, nút export/nội dung vẫn dùng được ở 900×640 và 1180×800.
- Svelte check: **0 errors, 0 warnings**; frontend build PASS.
- NSIS test build và production native check được chạy bằng private/WebView fixture riêng, không thay bản app của người dùng.
- `scripts/meeting-file-guide-native-check.cjs`: **PASS** trên production origin `http://tauri.localhost`: guide tự có ở folder modern cũ sau startup, UTF-8 không lỗi ký tự, labels trong Markdown đúng, status khớp file thật, hướng dẫn library có thật, Session trước/sau giữ nguyên.
- Test không dùng API cloud hoặc thu thêm âm thanh cho thay đổi tài liệu này. Dùng các meeting synthetic đã có corrected/translation/summary từ kiểm thử trước.

Rust assertions bao gồm: guide/checksum tồn tại kể cả meeting chưa có lời nói; không tạo transcript/corrected/audio rỗng để đáp ứng hướng dẫn; xóa summary/corrected thì guide đổi CHƯA CÓ; guide không lộ profile/prompt sentinel; cài guide còn thiếu giữ nguyên Markdown đã được sửa bên ngoài.

Ảnh/bằng chứng: `test-results/meeting-file-guide-ui/`, `test-results/meeting-file-guide-layout/`, `test-results/meeting-storage-native/file-guide-results.json`. Ảnh element screenshot có thể bị clip bởi vùng scroll nội bộ; assertions kiểm tra cả nội dung DOM và không tràn ngang, cùng các test layout riêng.

Chưa push/release hoặc nâng version. Bộ cài test 0.8.8 cùng đường dẫn được build lại; đây không phải bản cập nhật khách hàng đã phát hành ngày trước.
