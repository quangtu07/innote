# Tắt micro của bạn khi đang họp

Ngày 11/11/2026. Phạm vi: chỉ tắt nguồn micro của người dùng; âm thanh cuộc họp (loa/tai nghe) vẫn được ghi, bản chép và phụ đề vẫn chạy từ nguồn loa.

## Vấn đề

Trước thay đổi, muốn không ghi giọng mình phải chọn **Không thu** trong danh sách micro trước khi bắt đầu; khi phiên đã chạy không có nút tắt micro. Tắt cả phiên bằng Tạm dừng cũng làm ngừng ghi âm cuộc họp.

## Cách làm

- **Trong lúc họp:** nút **Tắt mic** cạnh Phụ đề/Tạm dừng/Kết thúc, chỉ hiện khi phiên có thu micro. Bấm lại thành **Bật mic**. Trạng thái hiển thị **Micro đang tắt** cạnh đồng hồ; không dừng ghi, không dừng phụ đề, không đổi trạng thái phiên.
- **Trước khi bắt đầu:** nút **Tắt mic/Bật mic** ngay cạnh tên micro trong mục Nguồn âm thanh, ghi nhớ thiết bị vừa chọn để bật lại. Cách chọn **Không thu** trong danh sách vẫn giữ nguyên.
- **Backend:** thêm cờ `mic_muted` trên phiên đang chạy và command `set_mic_muted`. Luồng thu mic khi tắt vẫn ghi **mẫu im lặng cùng độ dài** vào `microphone.pcm`, nên timeline của loa, mic, bản chép trực tiếp và file mix cuối không bị lệch; mic không ghi nội dung thật trong khoảng đã tắt.
- **Snapshot:** trả `mic_muted` và `has_microphone` để UI hiển thị đúng sau mỗi lần refresh/khởi động lại giao diện.
- Chuỗi giao diện có đủ vi/en/ko/zh. Ở màn hẹp ≤1000 px, cụm điều khiển đang ghi xuống hàng riêng để tiêu đề cuộc họp không bị cắt.

## Kiểm tra

| Kiểm tra | Kết quả |
|---|---|
| `npm run check` | 0 lỗi, 0 cảnh báo |
| `npm run build` | đạt |
| `cargo test --locked --manifest-path src-tauri/Cargo.toml` | **146 passed; 0 failed** (thêm 2 test: mã hoá PCM khi tắt mic và mix cuối không nhận mẫu mic đã tắt) |
| Full `scripts/ui-check.mjs` | **147/147 PASS**, `test-results/mic-mute-full/results.json` |
| Hai bài micro | **PASS**, `test-results/mic-mute/results.json` |

Bài kiểm tra UI mới:

1. **Microphone can be muted during a meeting without stopping capture** — bấm Tắt mic, kiểm tra `set_mic_muted {muted:true}`, nhãn **Micro đang tắt**, không có lệnh `stop`/`pause`; bấm Bật mic trả lại `muted:false`.
2. **Microphone can be turned off before a meeting starts** — nút cạnh tên micro đổi lựa chọn thiết bị, bật lại đúng thiết bị cũ; bắt đầu phiên gửi `microphone:null`, vẫn giữ nguồn loa.
3. **Recording header keeps the mic mute control visible at 900x640** — Tắt mic/Tạm dừng/Kết thúc nằm trong khung 900×640, không tràn ngang.

Ảnh: `test-results/mic-mute/meeting-mic-muted.png` (1180×800, đang tắt mic) và `meeting-mic-mute-900x640.png` (900×640).

## Giới hạn

- Chưa chạy phiên thu thật bằng thiết bị âm thanh trong lượt này; phần mã hoá PCM được kiểm tra bằng unit test, luồng UI bằng mock Tauri. Khi QA trên máy thật cần xác nhận `recording.wav` chỉ có tiếng loa trong khoảng đã tắt mic và không lệch tiếng sau khi bật lại.
- Trạng thái tắt mic không lưu vào cuộc họp/không khôi phục sau khi tắt app giữa phiên; mặc định phiên mới là bật mic nếu người dùng chọn micro.
- Không thay đổi cách chọn “Không thu”, mẫu cấu hình hay dữ liệu cuộc họp cũ.