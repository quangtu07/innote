# Đọc thư mục cuộc họp Innote

Nếu chưa chọn nơi lưu, Innote tự tạo và chọn `%USERPROFILE%\Innote Meetings` (ví dụ `C:\Users\PC\Innote Meetings`). Đổi trong **Cài đặt → Lưu trữ cuộc họp → Chọn thư mục**; chỉ các cuộc họp mới dùng nơi mới. Lựa chọn được giữ khi khởi động lại, kể cả lúc ổ đĩa đó tạm mất kết nối. Các cuộc họp cũ không tự chuyển.

Trong **thư viện mới**, mỗi thư mục có ngày, giờ và tên là một cuộc họp, ví dụ `2026-11-08_09-30-00 - Họp kế hoạch`. Mở **00 - ĐỌC TÔI TRƯỚC.txt** trước: hướng dẫn bằng tiếng Việt, đọc được bằng Notepad, có trạng thái **ĐÃ CÓ/CHƯA CÓ** theo file thực tế.

| Tên trong thư mục cuộc họp | Đây là gì? | Dùng khi nào? |
|---|---|---|
| `recording.wav` | Bản ghi âm đã hoàn tất của các nguồn loa/micro bạn bật | Nghe lại và đối chiếu những chỗ nhận dạng sai |
| `original.md` | Bản chép gốc của phiên ghi, gồm sửa tay bạn đã lưu trong app | Đọc lời gốc, chưa phải bản dịch hay bản hiệu chỉnh từ audio |
| `original.srt` | Phụ đề của bản gốc, kèm mốc thời gian | Dùng với trình phát hỗ trợ SRT |
| `corrected.md` | Bản hiệu chỉnh: model nghe lại file ghi âm | Chỉ có sau khi chạy Hiệu chỉnh; bản gốc vẫn được giữ riêng |
| `corrected.srt` | Phụ đề của bản hiệu chỉnh | Dùng nội dung/mốc thời gian sau hiệu chỉnh |
| `translations/vi.md`, `en.md`… | Bản dịch theo ngôn ngữ đích | `vi`: Việt; `en`: Anh; `ko`: Hàn; `zh`: Trung; `ja`: Nhật |
| `summary.md` | Bản tóm tắt do AI tạo | Đọc ý chính; hướng dẫn ghi rõ nguồn là bản gốc hay bản hiệu chỉnh |
| `notes.md` | Ghi chú bạn tự nhập trong Innote | Đọc ghi chú riêng, tách khỏi lời nhận dạng |
| `meeting.json` | Dữ liệu có cấu trúc cho AI/phần mềm | Đọc các bản riêng, ID, mốc thời gian, trạng thái, đường dẫn audio tương đối và checksum; không có cấu hình/API key |
| `exports/` | Các file bạn chủ động xuất vào thư mục này | Markdown/TXT/SRT/MP3 phục vụ chia sẻ; bạn có thể đã chọn đích xuất khác |
| `.innote/work/` | PCM nguồn và file xử lý/phục hồi của cuộc họp | App sử dụng khi chép hoặc phục hồi; không tự sửa/xóa |
| `.innote-identity.json` | Mã nhận diện cuộc họp và thư viện | App dùng để tìm lại folder khi đổi tên; không phải bản chép |
| `*.user-copy-*.md` | Bản giữ lại khi đầu ra do app tạo đã được sửa ngoài app và không còn dùng | Đối chiếu trước khi sử dụng; không tự coi đây là bản hiện hành của Innote |

Chưa dịch, chưa hiệu chỉnh, chưa tóm tắt hoặc chưa ghi chú thì chưa có những file tương ứng. Trong lúc ghi, audio có thể chưa hoàn tất và nội dung vẫn đang cập nhật. SRT dịch không được tự tạo khi chưa có alignment dịch riêng.

**Cách mở:** `.txt`, `.md` và `.srt` đều đọc được bằng Notepad; `.md` là văn bản Markdown. Mở `.wav` bằng trình phát âm thanh hoặc nghe theo đoạn trong Innote. `.json` dành cho AI/phần mềm; người dùng không cần tự chỉnh file này.

Các file Markdown có nhãn nội dung ngay đầu: **BẢN CHÉP GỐC**, **BẢN HIỆU CHỈNH**, **Tóm tắt**, **Ghi chú**; bản dịch có ngôn ngữ trong tiêu đề. Bản hiệu chỉnh vẫn có thể sai: dùng `recording.wav` để đối chiếu khi cần.

## Phân biệt thư viện với thư mục cuộc họp

Ở **cấp thư viện**, mở **00 - HƯỚNG DẪN THƯ VIỆN INNOTE.txt** để xem cấu trúc chung. Thư mục `.innote/` tại cấp này khác `.innote/work/` nằm trong từng cuộc họp:

| Bên trong `.innote/` của thư viện | Mục đích |
|---|---|
| `meetings.sqlite`, `-wal`, `-shm` | Cơ sở dữ liệu gốc của thư viện; không sửa/xóa từng file |
| `library.json` | Nhận diện thư viện |
| `writer.lock` | Tránh hai tiến trình cùng ghi; không cần tự xóa |
| `operations/` | Phục hồi thao tác đổi tên bị gián đoạn |
| `audio-tests/` | Bản thu thử thiết bị, không phải bản ghi cuộc họp |

Muốn sao lưu đầy đủ để tiếp tục dùng trong Innote, thoát app sạch sẽ rồi sao lưu **cả thư viện**, gồm dữ liệu nội bộ và các folder cuộc họp. Chép vài file `.md`/`.wav` chỉ là chia sẻ nội dung.

## Đọc và biên tập an toàn

- Đọc/chia sẻ: chọn đúng file hoặc chỉ cấp AI quyền vào thư mục cuộc họp cần phân tích; không cần cấp thư mục cấu hình/API key.
- Sửa trong app: chỉnh tại Innote. Sửa file bên ngoài không cập nhật ngược vào cơ sở dữ liệu; app có thể công bố lại file. Muốn biên tập độc lập, hãy tạo bản sao hoặc xuất file riêng.
- Cuộc họp ở **nơi lưu cũ**: bản chép/dịch/hiệu chỉnh/notes vẫn ở DB của Innote, chưa phải các file Markdown trong folder UUID cũ. Chọn phiên bản rồi xuất từ app. Không cấp toàn bộ nơi lưu cũ cho agent vì nơi đó có thể còn chứa cấu hình/API key.
- Cấu hình/API key, bộ cài và model local vẫn quản lý riêng; đổi thư viện không tự chuyển dữ liệu cũ hoặc giải phóng dung lượng cũ ở C.
