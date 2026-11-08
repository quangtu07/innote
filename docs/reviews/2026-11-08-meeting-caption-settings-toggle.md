# Thu gọn cài đặt phụ đề khi đang ghi

Nguyên nhân: nút Cài đặt phụ đề chỉ gán `meetingStyleOpen=true`, nên bấm lại không đóng được.

Sửa trong phạm vi giao diện cuộc họp đang ghi:

- Nút mở chuyển thành Thu gọn cài đặt phụ đề khi khung mở; cùng nút bật/tắt khung.
- Thêm nút X + Thu gọn ở đầu khung, có tên truy cập Đóng cài đặt phụ đề.
- Đóng từ trong khung trả focus về nút mở; các trường tùy chỉnh vẫn mở khi hiển thị khung.
- Giữ nguyên kiểu chữ đã chỉnh và mẫu đã lưu, không gọi dừng ghi hoặc dừng phụ đề khi thu gọn.

Kiểm tra:

- `npm run check`: 0 errors, 0 warnings.
- `npm run build`: đạt.
- `npm run tauri -- build --no-bundle -- --locked`: đạt; build Windows release trong 2m33s (11 cảnh báo dead-code hiện có). Đã mở lại đúng executable vừa build, không dùng dữ liệu fixture hoặc backend cũ.
- `scripts/ui-check.mjs`, filter `Meeting`, URL Vite riêng 1421: **17/17 đạt**, dùng mock API Tauri, không ghi âm hay gửi API thật.
- Bổ sung vào bài kiểm tra đang có: mở/đóng lặp, Enter/Space, nút X, focus trả lại, cỡ chữ giữ sau mở lại, mẫu gốc giữ nguyên, không có lệnh stop/stop_caption.
- Xem ảnh ở 900×640: `test-results/meeting-caption-close/meeting-runtime-caption-settings-close.png`; tiêu đề và nút thu gọn không đè nhau. Bài kiểm tra cũng chạy mở/chỉnh khung ở 1180×800.

Kiểm tra ứng dụng thật trước khi thay bản: phiên vừa ghi đã hoàn tất, không còn session đang recording/paused/processing/stopping. Không tạo thêm cuộc họp hoặc sửa dữ liệu cuộc họp thật để thử nút.
