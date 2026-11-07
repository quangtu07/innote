# Meeting storage — implementation checkpoint 1

Ngày: 07/11/2026. Nhánh: `codex/meeting-storage`. Đây là checkpoint nền tảng, chưa hoàn thành toàn bộ tính năng.

## Đã triển khai

- `storage_paths.rs`: schema v1 registry riêng tư, default library và các thư viện lịch sử; validate trước khi chọn/lưu.
- Ghi registry qua temp + flush + Windows MoveFileExW replace/write-through; lỗi thay thế giữ nguyên bản cũ, dọn temp.
- Kiểm tra canonical root, private/runtime overlap, ổ local/removable, ngân sách path và probe đọc/ghi/rename.
- Library marker ổn định, không tạo DB khi chỉ kiểm tra root; từ chối marker hỏng/DB không có marker.
- Từ chối reparse/junction trong đường dẫn quản lý. Writer lock dùng OS file handle độc quyền, được giải phóng khi đóng/crash.
- Chỉ khai báo module trong app; chưa gọi từ startup hoặc luồng ghi, chưa đổi đường dẫn dữ liệu thật.

## Kiểm tra đã chạy

PowerShell từ workspace:

```powershell
. ./scripts/environment.ps1
cargo test --manifest-path src-tauri/Cargo.toml
```

Kết quả: **129 passed; 0 failed; 0 ignored**, gồm 11 test storage mới. Writer lock được kiểm tra cả hai handle và hai tiến trình test riêng, trước/sau giải phóng lock. Junction thật được tạo trong fixture riêng để thử từ chối truy cập private root. Registry bị giữ file handle không cho replace vẫn giữ bytes cũ và retry sau khi thả handle thành công.

Chu kỳ red/green: các test registry/marker ban đầu fail do API chưa triển khai, sau implement pass. Lần gọi Cargo đầu thiếu LIBCLANG_PATH; dùng environment.ps1 có sẵn để nạp toolchain rồi chạy lại. Test mklink ban đầu truyền dấu `/` trong đường dẫn bị cmd hiểu thành switch; đã đổi sang dấu `\`, không bỏ qua test.

Chưa chạy UI/native app/ghi âm thật cho tính năng mới vì chưa nối chức năng. Không có tuyên bố rằng dữ liệu đã được chuyển hoặc đường dẫn ghi mới đã hoạt động. Các cảnh báo dead_code cho location types là do bước router chưa triển khai; cảnh báo soniox::transcribe có sẵn trước thay đổi.

## Còn lại theo plan

- Task 2: router legacy/thư viện, tên thư mục và journal phục hồi.
- Task 3: pin location trong recording/caption/correction/export/recovery, giới hạn asset scope.
- Task 4: bundle JSON/Markdown/SRT không có credentials, revision và retry.
- Task 5: Settings chọn thư mục, thông báo và mở đúng thư mục cuộc họp.
- Task 6: thử Windows thực tế, audio hệ thống/ổ khác, disk failures và tương thích dữ liệu cũ.

Chưa build installer, chưa release/push. Test dùng thư mục tạm riêng; không thao tác di chuyển dữ liệu khách.
