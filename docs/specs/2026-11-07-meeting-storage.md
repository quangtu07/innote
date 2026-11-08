# Đặc tả: nơi lưu cuộc họp riêng, thư mục có tên và dữ liệu dễ đọc cho AI

Ngày: 07/11/2026. Trạng thái: đã triển khai trên nhánh `codex/meeting-storage`, chưa phát hành. Không di chuyển dữ liệu thật. Kết quả và giới hạn trong [báo cáo kiểm thử](../reviews/2026-11-07-meeting-storage-qa.md).

## 1. Quyết định đã chốt với người dùng

- Một thư mục lưu chung cho các cuộc họp mới, chỉ đổi trong **Cài đặt**; không thêm ô chọn nơi lưu cho từng cuộc họp.
- Chỉ thay nơi lưu cuộc họp mới. Cuộc họp cũ giữ nguyên vị trí, nội dung và khả năng xem/nghe lại; không chuyển hoặc xóa hàng loạt khi nâng cấp.
- Tên thư mục phải có tên cuộc họp, không chỉ là UUID.
- Bản ghi âm, bản gốc, bản dịch, bản hiệu chỉnh, tóm tắt và ghi chú của cuộc họp mới ở nơi người dùng chọn. Không chỉ đổi chỗ xuất file trong khi bản thu vẫn âm thầm nằm ở ổ C.
- Cấu hình ứng dụng/API key nằm riêng, không được sao chép sang nơi chứa cuộc họp hoặc gói dữ liệu cho agent.
- Đây là nền tảng lưu trữ và chia sẻ file. Chưa làm MCP, đồng bộ cloud, agent phân tích hoặc phát hành phiên bản mới trong công việc này.

**Hệ quả cần nói rõ trong UI:** dữ liệu cũ vẫn chiếm dung lượng ở vị trí cũ theo lựa chọn trên. Đổi nơi lưu không tự giải phóng dung lượng cũ ở ổ C. Tác vụ người dùng chủ động thực hiện trên cuộc họp cũ vẫn lưu kết quả tại nơi cũ.

## 2. Hiện trạng repo

- `storage::root()` hiện trỏ về `%LOCALAPPDATA%/Innote` hoặc `MAY_DATA_DIR`.
- `meetings.sqlite` trong thư mục đó chứa toàn bộ Session dạng JSON: bản chép, bản dịch, hiệu chỉnh, ghi chú, tóm tắt và metadata.
- Âm thanh nằm ở `sessions/<uuid>/`: PCM nguồn, WAV đang hoàn tất và `recording.wav`. Đổi đường dẫn export hiện tại không đổi nơi thu thật.
- `May.root` đang dùng chung cho lưu cuộc họp, khôi phục, key, profile, log, model tải thêm và một số dữ liệu tạm. Phải tách trách nhiệm; không thay toàn bộ root sang ổ D vì sẽ mang theo cấu hình/key.
- Mẫu và nhiều tùy chọn UI hiện nằm trong WebView localStorage; không đồng nhất chúng với một thư mục cuộc họp.
- Bộ cài/tài nguyên runtime có thể cũng nằm dưới thư mục Innote ở LocalAppData. Đây không phải bản ghi cuộc họp và không chuyển theo cài đặt này.

## 3. Mô hình lưu trữ được đề xuất

### 3.1. Khu vực riêng tư của ứng dụng

Giữ vị trí/key-format tương thích hiện có trong lần thay đổi này. Thêm `meeting-storage.json` tại private root, chỉ lưu phiên bản schema, thư mục mặc định và danh sách các thư viện đã đăng ký. Không chứa bản thu hay nội dung bản chép.

API profiles, khóa cũ, xác nhận đã nhập key, model/config, tùy chọn UI và log kỹ thuật được quản lý riêng như hiện tại. Không di chuyển profile/key hoặc đổi cơ chế mã hóa trong dự án này. Bộ cài, WebView cache và model local hiện có không được tuyên bố là đã chuyển khỏi ổ C.

`private_root` không còn là đầu vào mặc định cho API lưu cuộc họp mới. Root cũ chỉ còn là nguồn cho **LegacyRepository** khi truy cập những cuộc họp đã tồn tại.

### 3.2. Thư viện cuộc họp mới

Ví dụ người dùng chọn `D:/Innote Meetings`:

    D:/Innote Meetings/
      00 - HƯỚNG DẪN THƯ VIỆN INNOTE.txt
      .innote/
        library.json                 # library_id + schema_version
        meetings.sqlite              # nguồn dữ liệu chuẩn của thư viện này
        operations/                  # journal cho tạo/đổi tên, không có API key
        audio-tests/                 # kiểm tra âm thanh do app tạo, không ghi ở C
      2026-11-07_09-30-00 - Họp kế hoạch/
        00 - ĐỌC TÔI TRƯỚC.txt        # hướng dẫn tiếng Việt + trạng thái file thực tế
        meeting.json                 # snapshot UTF-8 cho agent, có revision
        original.md
        original.srt
        translations/
          vi.md
        corrected.md
        corrected.srt
        summary.md
        notes.md
        recording.wav
        .innote/work/                # PCM/phần thu phục hồi/tệp làm việc

Các tệp bản dịch, hiệu chỉnh, tóm tắt và WAV chỉ xuất hiện khi thực sự có dữ liệu tương ứng. Không tạo tệp rỗng làm agent hiểu nhầm đã có kết quả. MP3 chỉ tạo khi người dùng yêu cầu xuất, không tự nhân đôi WAV thành MP3.

Triển khai giữ quy tắc hiện có của app: không tạo SRT bản dịch khi chưa có alignment dịch riêng. Bản dịch có Markdown và JSON với source IDs/time ranges; SRT bản gốc/hiệu chỉnh vẫn được tạo. Thêm `.innote-identity.json` chỉ có meeting/library UUID để quét lại folder đã đổi tên, kể cả trước lần công bố `meeting.json` đầu tiên.

Xuất thủ công mặc định vào `exports/` của đúng cuộc họp; người dùng vẫn có thể chọn đích khác. Không tiếp tục mặc định ghi bản xuất mới vào Downloads trên ổ C. Thư mục nội bộ `.innote/` thuộc ứng dụng; để agent đọc, ưu tiên cấp quyền vào thư mục cuộc họp cụ thể và các tệp công bố, không yêu cầu đọc DB nội bộ.

SQLite trong thư viện là nguồn dữ liệu chuẩn để giữ giao dịch/khôi phục hiện tại. `meeting.json` và Markdown/SRT là bản công bố từ dữ liệu chuẩn, không phải nguồn được app tự nhập ngược. Như vậy không cần viết lại toàn bộ engine lưu trữ chỉ để agent đọc được file.

Mỗi lần đổi thư mục mặc định, thư viện trước vẫn được đăng ký để liệt kê các cuộc họp đã tạo ở đó. Không copy dữ liệu sang thư viện mới. Danh sách cuộc họp hợp nhất dữ liệu cũ và các thư viện đã chọn trước đây.

### 3.3. Định danh và định tuyến

- UUID cuộc họp vẫn là định danh bất biến trong app/JSON. Tên thư mục chỉ để con người nhận biết.
- Thư viện mới thêm bảng `session_paths(session_id, relative_folder, revision, bundle_revision, bundle_state, pending_folder_name)`; Session JSON hiện tại tiếp tục giữ nguyên cấu trúc dữ liệu nghiệp vụ.
- `MeetingLocation` xác định library, layout cũ/mới, thư mục cuộc họp, thư mục capture và WAV cuối. Không tính lại đường dẫn từ tiêu đề hiện tại mỗi lần đọc.
- Phiên đang chạy giữ một location cố định từ lúc bắt đầu. Chép lời local/cloud, phụ đề dùng chung, finalization, nghe lại, hiệu chỉnh và export phải dùng cùng location đó.
- Root cũ vẫn dùng `sessions/<uuid>` và DB cũ. Không tạo lại tên thư mục hoặc tự sinh bundle cho cuộc họp cũ khi chỉ mở app.
- Hai thư viện có cùng library_id hoặc cùng session_id không được âm thầm gộp/chọn ngẫu nhiên. Liên kết lại một thư viện bị mất phải xác nhận marker/ID; bản copy trùng khi nguồn vẫn còn được báo là trùng.

## 4. Tên thư mục và đổi tên

Mẫu: `YYYY-MM-DD_HH-mm-ss - <Tên cuộc họp>`. Thời gian lấy từ thời điểm tạo đã lưu, không lấy lúc export hoặc lúc khởi động lại.

- Giữ tiếng Việt và Unicode hợp lệ; loại ký tự điều khiển, thay ký tự cấm của Windows, bỏ dấu chấm/khoảng trắng cuối tên. Tiêu đề trong UI vẫn giữ bản người dùng nhập.
- Xử lý tên rỗng, CON/PRN/AUX/NUL/COM/LPT và tên tương đương có phần mở rộng; không cho tiêu đề trở thành đường dẫn.
- Giới hạn phần tên theo ngân sách đường dẫn; mục tiêu tương thích là đường dẫn được app tạo không vượt 240 UTF-16 code units. Nếu root quá dài để đặt các tệp chuẩn, báo chọn root ngắn hơn trước khi thu. Không đổi thiết lập long-path của Windows.
- Trùng tên, kể cả chỉ khác hoa/thường hoặc cùng giây: tạo hậu tố ` (2)`, ` (3)` bằng thao tác create-new; không ghi đè thư mục đã có.
- Cuộc họp đã kết thúc, không có job/đọc audio đang giữ file: đổi tiêu đề sẽ đổi thư mục tương ứng và cập nhật ánh xạ theo ID.
- Đang thu/processing/hiệu chỉnh/export: cho đổi tiêu đề nhưng hoãn đổi tên vật lý đến khi các handle được giải phóng; hiện “Tên thư mục sẽ cập nhật khi xử lý xong”. Không di chuyển file đang ghi.
- Đổi tên thư mục và DB dùng journal có thể khôi phục sau crash. Nếu rename thất bại vì tệp đang mở, giữ đường dẫn hoạt động cũ và báo rõ; không để UI khẳng định thư mục đã đổi.
- Người dùng tự đổi tên thư mục trong Explorer: “Quét lại thư viện” chỉ tìm trong root đã đăng ký và ghép lại bằng ID trong `meeting.json`. Nhiều thư mục trùng ID phải yêu cầu chọn lại, không tự ghi đè.

Quy tắc tên tham chiếu [Microsoft: Naming Files, Paths, and Namespaces](https://learn.microsoft.com/en-us/windows/win32/fileio/naming-a-file).

## 5. File cho agent và tính nhất quán

`meeting.json` schema v1 là snapshot tự đủ để đọc bản chép mà không mở SQLite: id, title, created/updated, revision, status, ngôn ngữ gốc, duration, audio availability/relative path, original segments, translations theo ngôn ngữ, correction, summary và notes. Giữ ID/mốc thời gian segment để phân tích có thể trỏ lại nguồn.

- Bộ chuyển DTO phải allowlist trường; không serialize nguyên Session/config ra gói chia sẻ. Không có API key, key_tail, profile_id, Base URL, đường dẫn config tuyệt đối, request header, log kỹ thuật hoặc prompt cấu hình riêng.
- Provider/model và nguồn tóm tắt được phép ghi để giải thích xuất xứ. Không tự thay bản gốc bằng bản hiệu chỉnh hoặc bản dịch.
- Nội dung cuộc họp tự nó có thể nhạy cảm; tách thư mục không phải cơ chế ngăn một agent đã có quyền đọc toàn bộ ổ đĩa. Chỉ cấp agent quyền vào thư mục cuộc họp cần dùng; không cấp private root.
- App không tự upload hoặc mở server mạng. Tệp cho agent nằm trên máy/ổ được chọn; muốn cloud đọc khi laptop tắt là một tính năng đồng bộ riêng.
- Đang ghi: chỉ công bố phần đã chốt, status thể hiện chưa kết thúc; không biến chữ tạm thành kết quả cuối. Agent có thể lọc chỉ đọc cuộc họp đã hoàn tất.
- Publisher chạy nền ngoài callback audio, gộp các thay đổi và cập nhật tối đa một lần/giây; sau kết thúc/sửa/hiệu chỉnh/tóm tắt phải flush bản mới trước khi báo “Tệp cuộc họp đã cập nhật”.
- Ghi tệp tạm trong chính thư mục đích, flush và thay thế nguyên tử. JSON là snapshot nhất quán tự đủ, có revision và manifest checksum của MD/SRT, được công bố sau cùng; MD có thể thêm revision nhưng SRT vẫn giữ đúng định dạng phát phụ đề. Không hứa giao dịch nguyên tử cho cả nhóm file.
- Lỗi công bố file không được làm mất bản thu/DB. UI báo “Đã lưu trong Innote, chưa cập nhật xong tệp cuộc họp”, cho Thử lại/Mở thư mục; `bundle_revision` phải nhỏ hơn revision chuẩn khi còn bẩn.
- File do agent tự tạo không được app dùng làm lệnh, tự nhập ngược hoặc ghi đè. Mọi thao tác xóa/đổi tên chỉ làm trong thư mục đã nhận diện bằng library/session ID, không theo đường dẫn tùy ý từ nội dung file.

## 6. UX

Giữ thiết kế Operate hiện tại; không thêm nhiều card/cột hoặc một wizard dài.

### Cài đặt → Lưu trữ

Một vùng gồm đường dẫn cuộc họp mới, dung lượng trống và nút **Chọn thư mục**, **Mở thư mục**. Bên dưới: “Chỉ áp dụng cho cuộc họp tạo từ bây giờ. Các cuộc họp đã có không bị chuyển.”

Sau khi chọn bằng native folder picker, hiển thị đường dẫn thực, trạng thái kiểm tra ghi/đọc và ví dụ tên thư mục. Bấm **Dùng cho cuộc họp mới** mới lưu thiết lập; Hủy giữ nguyên. Thành công có thông báo và khởi động lại vẫn giữ lựa chọn.

Phần thông tin thu gọn cho biết: cấu hình/API nằm riêng; dữ liệu cũ vẫn ở vị trí cũ; các thư viện trước vẫn được đọc. Không biến các thư viện cũ thành nhiều lựa chọn cạnh tranh trên màn hình tạo cuộc họp.

Đổi nơi lưu bị khóa trong lúc ghi/tạm dừng/processing/hiệu chỉnh/xuất hoặc đang có ghi chú chưa lưu. Nói rõ thao tác đang giữ tệp; chờ lưu xong rồi đổi, không tự dừng tác vụ của người dùng.

### Cuộc họp mới và cuộc họp đã lưu

- Thiết lập cuộc họp mới chỉ thêm một dòng gọn “Lưu tại: …” và liên kết tới Cài đặt. Không có dropdown chọn nơi lưu theo phiên.
- Sau khi kết thúc có **Mở thư mục cuộc họp**; đường dẫn đầy đủ nằm trong phần thông tin, không làm dài trang.
- Danh sách hợp nhất, có nhãn “Nơi lưu cũ” khi cần phân biệt, không tạo bản sao cuộc họp.
- Thiếu ổ đĩa: vẫn vào được app/Cài đặt và đọc các thư viện còn hoạt động; không giả vờ danh sách trống hoặc tạo thư viện mới thay cho thư viện mất kết nối.
- Lần đầu dùng bản có tính năng này: yêu cầu xác nhận nơi lưu trước khi tạo cuộc họp mới; gợi ý thư mục Documents/Innote Meetings theo Windows Known Folder nhưng cho chọn ổ khác. Không mặc định ghi tiếp bản mới vào legacy AppData.

## 7. Hỏng ổ, đầy ổ và phạm vi hỗ trợ

- V1 hỗ trợ thư mục trên ổ local/ổ rời; không hỗ trợ DB đang ghi trên UNC/network share hoặc nhiều máy ghi chung một thư viện. SQLite WAL không phù hợp network filesystem: [SQLite WAL](https://sqlite.org/wal.html).
- Một tiến trình ghi cho mỗi thư viện; giữ khóa single-instance ở private root và khóa writer cho thư viện trước recovery.
- Chọn nơi lưu và bắt đầu phiên phải đồng bộ qua cùng bộ điều phối tác vụ; kiểm tra đang rảnh ngay trước commit thay đổi, không chỉ khóa nút UI. Phiên đã bắt đầu không được đổi đường dẫn theo một bản cấu hình mới hơn.
- Kiểm tra tồn tại, marker, quyền tạo/đọc/rename và dung lượng trước start. Không kiểm tra bằng cách mở DB thiếu rồi tạo DB rỗng.
- Không nhận root trùng/chứa/bị chứa bởi private config hoặc thư mục cài/runtime; không theo junction/symlink/reparse point cho thao tác quản lý. Nếu chọn cả ổ D, gợi ý tạo một thư mục Innote Meetings chuyên dụng và cho người dùng xác nhận đường dẫn thực.
- Mất ổ/đầy ổ giữa phiên: dừng thu theo lỗi storage, giữ phần đã ghi được và báo rõ trạng thái có thể phục hồi. Không âm thầm ghi tạm âm thanh/bản chép mới vào C hoặc chuyển cloud. Không cam kết khôi phục phần chưa kịp flush khi ổ bị rút.
- Thư mục cloud-sync cục bộ không được quảng cáo là đồng bộ trực tiếp DB hoặc ghi từ nhiều máy. Đồng bộ bản hoàn tất/remote agent là dự án riêng.
- Giữ nguyên bản ghi và PCM hiện có; không tự dọn audio cũ hoặc xóa nguồn để tiết kiệm dung lượng trong dự án này.
- Asset protocol chỉ cho đọc audio tại các root đã xác thực, `sessions/` cũ và đường dẫn kiểm tra âm thanh cần thiết. Thay scope `**` hiện có bằng scope động có giới hạn; không cấp quyền vào toàn bộ private root, key/config hoặc đường dẫn do frontend truyền tùy ý.

## 8. Tương thích và kiểm thử chấp nhận

1. Nâng từ 0.8.8 với các phiên đã kết thúc: tất cả body/hash của DB cũ và audio cũ giữ nguyên khi chỉ mở app, chọn nơi mới hoặc khởi động lại; profiles/key-file bytes không đổi. Không sao chép chúng sang thư viện mới. Phiên cũ bị gián đoạn vẫn được phép khôi phục theo luồng hiện tại, phải kiểm thử riêng.
2. Cuộc họp mới ghi trực tiếp PCM/WAV/DB/bundle tại root đã chọn; private root không xuất hiện audio/transcript mới. Bao gồm nguồn kiểm tra âm thanh và temp hiệu chỉnh do app quản lý.
3. Đổi mặc định A → B: các cuộc họp A vẫn truy cập được, phiên tiếp theo ở B; không có copy ngầm. Sửa/hiệu chỉnh cuộc họp A vẫn dùng A.
4. Tên Unicode, trùng, tên cấm, đường dẫn dài, title đổi khi đang thu và title đổi sau kết thúc đều giữ cùng UUID, nghe lại và các liên kết segment.
5. Agent đọc được original/translation/corrected/summary/notes từ JSON/MD; không có placeholder giả và không có credential sentinel trong cả nội dung lẫn manifest/đường dẫn chia sẻ.
6. Fault injection tại các điểm rename/commit/publish: không mất DB/âm thanh, không file JSON bị ghi dở, không báo tệp đã cập nhật khi revision chưa đồng bộ; restart sửa journal có thể lặp an toàn.
7. Mất ổ, tái gán ký tự ổ, hết dung lượng, khóa file, quyền bị từ chối, thư viện trùng ID: báo đúng và không quay lại C.
8. Ghi loa, loa+micro, pause/resume/stop, Soniox dùng chung dịch, Groq dùng chung chép, local, correction và summary tiếp tục hoạt động từ đường dẫn mới. Test audio hệ thống, không chỉ mock microphone.
9. Bản development và bản NSIS thật đọc WAV ở ổ khác qua asset scope đã giới hạn; không đọc được file config/key qua asset protocol.
10. Không tự chuyển dữ liệu cũ, nâng version, build release hoặc push trong giai đoạn viết spec/plan này.

Nếu sau này làm tính năng chuyển dữ liệu cũ, phải có spec riêng với copy → verify → chuyển tham chiếu → dọn nguồn có xác nhận, dùng SQLite Backup API thay vì copy mù file DB/WAL đang mở: [SQLite Backup API](https://sqlite.org/backup.html). Không đưa migration đó vào lần triển khai hiện tại.
