# Thư mục mặc định cho cuộc họp mới

Yêu cầu: tự chọn nơi lưu khi chưa có lựa chọn; người dùng đổi sau trong Cài đặt. Không di chuyển các cuộc họp cũ.

## Hành vi

- Dùng thư mục cá nhân do Tauri `home_dir()` cung cấp, tạo `Innote Meetings`; không ghi cứng tên user/ổ C.
- Không dùng Documents làm mặc định vì máy kiểm thử chuyển Documents vào OneDrive.
- Chỉ khởi tạo nếu chưa có thư viện mặc định. Giữ nguyên lựa chọn đã lưu ngay cả khi ổ offline; không thay registry hỏng bằng registry mới.
- Báo lỗi trong Cài đặt nếu không tạo được nơi lưu, không âm thầm ghi cuộc họp mới vào AppData.
- Không chuyển dữ liệu cũ hoặc thay cấu hình/API key.

## Kiểm tra ngày 08/11/2026

- `cargo test --locked --manifest-path src-tauri/Cargo.toml`: **144 passed**, gồm bốn kiểm tra mới: khởi tạo/lặp lại, giữ lựa chọn custom offline, registry hỏng, home không hợp lệ/file trùng tên.
- `npm.cmd run tauri -- build --no-bundle -- --locked`: build release executable hoàn tất. Không tạo bộ cài mới trong lượt này.
- Đóng bản cũ đang rảnh; chạy đúng `src-tauri/target/release/innote.exe` vừa build, không dùng MAY_DATA_DIR hoặc WebView test overrides.
- Trước đó máy chưa có thư viện mặc định. Sau khởi động, registry tự chọn `C:\Users\PC\Innote Meetings`. Trang cuộc họp và Cài đặt đều hiển thị đúng đường dẫn, nút Chọn thư mục/Mở thư mục khả dụng.
- Đối chiếu trước/sau bằng SHA-256: **69 cuộc họp cũ không đổi**, cả ba tệp cấu hình/key được đối chiếu giữ nguyên. Chỉ xuất hash/kết quả kiểm tra, không xuất nội dung key hoặc cuộc họp. Bằng chứng cục bộ: `test-results/default-meeting-folder/before.json`, `after.json`.
- Hai mục giải thích đã yêu cầu xóa vẫn không xuất hiện trong Cài đặt.
- `git diff --check`: đạt.

Chưa kiểm tra lỗi quyền ghi/ổ offline bằng việc thay đổi thư mục thật của người dùng; các trường hợp này kiểm thử trên fixture Rust. Không push hoặc phát hành GitHub.
