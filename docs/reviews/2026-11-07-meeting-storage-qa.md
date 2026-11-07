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

## Rà soát lại theo yêu cầu người dùng

Ngày 07/11/2026, sau checkpoint 1. **Kết luận: chưa đủ điều kiện nghiệm thu tính năng lưu trữ mới.**

| Yêu cầu | Bằng chứng hiện tại | Kết luận |
|---|---|---|
| Một thư mục chung đổi trong Cài đặt | App.svelte vẫn hiển thị data.data_dir và gọi open_folder; chưa có storage commands | Chưa có luồng UI |
| Cuộc họp mới thực sự thu tại nơi đã chọn | May.root, storage::save/load/session_dir vẫn dùng root cũ; storage_paths chỉ được khai báo module | Chưa tích hợp |
| Cuộc họp cũ nguyên vị trí, vẫn đọc được cùng thư viện mới | Adapter/router hợp nhất chưa có; tests cũ vẫn qua | Chưa chứng minh luồng A→B + legacy |
| Folder theo tên cuộc họp, đổi tên an toàn | storage::session_dir vẫn sessions/UUID | Chưa triển khai |
| Các bản gốc/dịch/hiệu chỉnh và JSON/MD cho agent | BundlePublisher chưa có | Chưa triển khai |
| API key tách khỏi cuộc họp mới | Lớp nền không copy private file (test sentinel qua); recording chưa đổi nơi lưu | Chỉ xác minh lớp nền, chưa đạt end-to-end |
| Media scope giới hạn ở audio hợp lệ | tauri.conf.json vẫn còn scope ** | Chưa harden; phải xử lý trước nghiệm thu |
| Mất ổ/đầy ổ/restart giữa phiên | Validation thiếu root đã test; luồng capture/router mới chưa có | Chưa test được toàn phiên |

Bổ sung ba test: lỗi đọc registry không thành default rỗng, root quá dài không tạo file probe/thư viện, đường dẫn Unicode không copy private file. Đổi kiểm tra tồn tại registry/marker/DB sang try_exists để truyền lỗi I/O thay vì coi mọi lỗi metadata là không tồn tại.

Kết quả lần rà soát:

- Full Rust: **132 passed; 0 failed; 0 ignored** (14 test storage).
- `npm run check`: 0 errors, 0 warnings.
- `npm run build`: PASS; đây là frontend build, không phải build/test NSIS.
- `node scripts/workflow-presets-check.mjs`: PASS; persistence/rollback/mẫu và cấu hình hiệu chỉnh.
- `node scripts/caption-presentation-check.mjs`: PASS; ghép song ngữ/trạng thái bản gốc-bản dịch.
- `git diff --check`: PASS.

Không gọi các kết quả trên là kiểm thử UI tương tác hay ghi âm thật cho storage mới. Chưa có màn hình/command storage để test theo hành vi người dùng; không chạy phiên API cloud hoặc ghi dữ liệu thật chỉ để minh họa cho chức năng chưa tích hợp. Cần hoàn thành Tasks 2–6 và thử trên Windows trước khi kết luận đúng toàn bộ yêu cầu.
