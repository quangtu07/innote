# Kiểm thử lưu trữ cuộc họp riêng — 07/11/2026

## Kết quả hiện tại — thay thế kết luận checkpoint bên dưới

Đã tích hợp backend, UI và kiểm thử Windows trên nhánh `codex/meeting-storage`. Bản test vẫn mang version 0.8.8, chưa phát hành/updater, chưa push. Các phần “chưa triển khai” ở lịch sử bên dưới là trạng thái **trước lần hoàn thiện**, không phải kết luận hiện tại.

### Luồng đã hoàn thành

- Cài đặt → Lưu trữ cuộc họp: chọn folder bằng hộp thoại Windows, xem trước, xác nhận **Dùng cho cuộc họp mới**, hoặc hủy. Có dung lượng trống, mở folder, thông tin cấu hình/dữ liệu cũ.
- Một default cho meeting mới; không lưu storage path vào preset hoặc chọn đích theo phiên. Đổi A → B không chuyển dữ liệu A/legacy. Các library trước vẫn được định tuyến bằng UUID.
- DB mới ở `<library>/.innote/meetings.sqlite`; folder có timestamp + title, PCM/temp ở `<meeting>/.innote/work`, WAV tại `<meeting>/recording.wav`.
- File có dữ liệu thật: `meeting.json`, `original.md/.srt`, `translations/<language>.md`, `corrected.md/.srt`, `summary.md`, `notes.md`. Giữ các bản riêng, không thay bản gốc.
- JSON allowlist có revision/checksum manifest, bỏ key/profile_id/Base URL/prompt riêng/lỗi kỹ thuật. Publisher gộp khoảng một giây và flush cuối thao tác/phiên; lỗi giữ DB/audio và snapshot JSON cũ, có Thử lại. Giữ file agent khác; không để output đã xóa dữ liệu giả làm bản hiện hành.
- Title đổi lúc ghi/job giữ audio: hoãn đổi folder vật lý. Journal phục hồi rename; quét library relink folder đổi bằng Explorer, không nhập ngược bản chép từ file agent. Dirty bundle được xếp hàng lại sau startup.
- Thu âm/local/Soniox/Groq/phụ đề chung/nghe lại/hiệu chỉnh/tóm tắt/export dùng location của UUID. Export mặc định vào `exports/` của meeting; đích tự chọn vẫn được tôn trọng.
- Bỏ asset scope `**` và toàn AppData; backend chỉ cấp từng WAV theo ID/kết quả thu thử. Cấu hình/API/model/bộ cài vẫn riêng; dữ liệu cũ trên C không tự giảm dung lượng.

### Kết quả kiểm tra

| Kiểm tra | Kết quả |
|---|---|
| Full Rust `cargo test --locked --manifest-path src-tauri/Cargo.toml` | **138 passed; 0 failed; 0 ignored** |
| `npm run check` | **0 errors, 0 warnings** |
| Full `scripts/ui-check.mjs` | **140/140 PASS**, `test-results/meeting-storage-ui-clean/results.json` |
| Nhóm storage UI sau chỉnh cuối | **5/5 PASS**, 900×640 và 1180×800, gồm lỗi đọc được xóa khi library kết nối lại |
| Build NSIS cục bộ | PASS, updater artifact tắt riêng cho build test; không ký/phát hành updater |
| Speaker capture, microphone null, Moonshine EN, pause/resume/đổi title khi ghi | PASS thật trên library A ở C; original 3 đoạn, WAV ngoài private root |
| Chuyển default sang D, thu riêng không chép | PASS; meeting A vẫn đọc được |
| Hiệu chỉnh từ WAV A | PASS Moonshine EN; corrected riêng, segments gốc giữ nguyên |
| Restart | PASS; A/B và profile nạp đúng trong fixture |
| Soniox meeting + dịch Việt | PASS API thật: **4 đoạn gốc, 4 đoạn dịch, một kết nối dùng chung** |
| Soniox phụ đề độc lập | PASS API thật; có chữ dịch, stop sạch, không tạo meeting |
| Groq Whisper large-v3-turbo chép/phụ đề chung | PASS API thật: **5 đoạn**, WAV/original.md ở D |
| Groq GPT-OSS-20B tóm tắt từ bản gốc | PASS API thật; summary riêng và summary.md |
| Mất default root qua injection trong fixture | PASS; start bị từ chối, không fallback C/tạo DB giả; library khác còn đọc |
| Explorer rename/rescan và xóa một meeting fixture mới | PASS |
| Hộp chọn thư mục Windows thật bằng Computer Use | PASS: select folder B → preview; hủy preview; mở lại/Escape không tạo lựa chọn mới |
| Production `http://tauri.localhost` | PASS restart, phát WAV khác ổ, export MD đúng folder meeting, MP3, ghi mới/đổi title khi ghi; private asset bị từ chối |

Rust tests thêm coverage cho legacy body/private sentinel giữ nguyên, library marker bị thay, collision/Unicode/path dài, writer lease hai tiến trình, junction thật, JSON bị khóa/retry, snapshot không lộ credential sentinel, journal/crash boundary, deferred rename không bị notes-save kích hoạt sớm và phục hồi WAV tại library mới.

Full UI từng có một timeout ở ca thu thử mock 8 giây khi chạy đồng thời với build nặng; chạy riêng pass, rồi full suite khi máy rảnh hơn đạt 140/140. Không bỏ test hoặc giảm assertion. Native media test ban đầu kiểm tra currentTime quá sớm; đã đổi sang chờ playback thật tiến lên, không chỉ chờ số mili giây cố định.

### Tái lập và bằng chứng

Các script `meeting-storage-{native,cloud,groq,offline,packaged}-check.cjs` trong `scripts/` kết nối CDP của **bản test với MAY_DATA_DIR/WebView profile riêng**, kiểm tra snapshot đúng fixture trước khi thao tác. `meeting-storage-play-fixture.ps1` phát WAV qua loa Windows; không dùng microphone/chèn transcript giả.

Bằng chứng tại `test-results/meeting-storage-native/{native,cloud,groq,offline,packaged}-results.json`; ảnh UI tại đó và `test-results/meeting-storage-ui-focused-final/`. Không stage audio/bằng chứng/API key để push.

Build test cuối: `src-tauri/target/release/bundle/nsis/Innote_0.8.8_x64-setup.exe`, **79,341,427 bytes**, SHA256 `75b608bf22d452f9f6bc4c316d793efe469491457995ebdfb94eb68ad922d9a9`. Production smoke cuối đã xác nhận thêm ghi mới và rename sau stop trên đúng executable/resources này. Probe asset cuối dùng `meeting-storage.json` **đang tồn tại** trong private fixture (không lấy 404 của key file đã dọn làm bằng chứng bảo vệ).

App người dùng đã cài giữ nguyên và không bị dừng/thay thế. Để thử API thật đã dùng một bản sao profile mã hóa nhẹ trong **private fixture**, không trong library; hash khớp source khi dọn, bản sao credential đã xóa. Source profile thực không được ghi bởi script. Không in API key ra output kiểm thử.

### Điều chỉnh thiết kế và giới hạn kiểm thử

- Tái dùng facade `storage::load/save/list/session_dir` để nối engine hiện tại. `May.root` vẫn là private root; location đã tạo theo UUID không phụ thuộc default mới. Không phải thay toàn bộ API worker để đảm bảo pin đường dẫn.
- `.innote-identity.json` trong folder meeting chỉ chứa meeting/library UUID, phục vụ relink trước lần publish đầu tiên.
- Không tạo SRT dịch giả định alignment riêng: bản dịch có Markdown/JSON và source IDs/ranges; SRT gốc/corrected giữ hành vi của app. Không tự dọn PCM hoặc làm cloud sync/MCP.
- V1 local/USB; UNC/network DB bị từ chối. Chưa rút USB vật lý/làm đầy ổ thật; đã thử missing-root injection, file-lock/atomic replace, storage-controller failure và journal boundaries. Không cam kết cứu bytes chưa flush khi ổ mất đột ngột.
- Không cài đè NSIS lên app người dùng. Production smoke dùng exe/resources cùng build, không dùng UI development/backend cũ. Không benchmark lại WER/độ trễ hoặc khẳng định chính xác tuyệt đối.

## Lịch sử checkpoint 1 (đã được hoàn thiện tiếp)

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
