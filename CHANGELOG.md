# 0.6.10 — 30/10/2026 — báo đang hiệu chỉnh cạnh nút tải

- Hiện thông báo **“Đang hiệu chỉnh… Có thể tải bản chép khi xong.”** ngay cạnh tiêu đề bản chép khi `recording/paused/processing`.
- 3 nút `.md/.txt/.srt` vẫn khóa trong lúc chờ; `.mp3` vẫn tải được. Thông báo tự mất và nút tự bật khi hiệu chỉnh xong.
- Không đổi dữ liệu/engine/định dạng xuất.

# 0.6.9 — 30/10/2026 — ẩn nhãn hiệu chỉnh, chặn tải bản chép chưa xong

- Ẩn dòng **“Đang hiệu chỉnh…”** dưới đoạn live; text vẫn hiển thị bình thường.
- Khi Whisper còn hiệu chỉnh (`recording/paused/processing`), 3 nút `.md/.txt/.srt` bị khóa; tooltip báo chờ hiệu chỉnh xong. Nút `.mp3` vẫn tải được.
- Phiên `interrupted` không có worker hiệu chỉnh đang chạy nên vẫn cho tải bản chép đã lưu.
- UI 44/44 pass, thêm kiểm tra ẩn nhãn + khóa export khi live; Rust 47/47; Svelte 0 lỗi/0 cảnh báo.

# 0.6.8 — 30/10/2026 — làm rõ trạng thái hiệu chỉnh

- Giữ bước Whisper hiệu chỉnh và nhãn trạng thái dưới đoạn live; đổi nhãn từ “Chờ hiệu chỉnh” thành **“Đang hiệu chỉnh…”** cho dễ hiểu.
- Vẫn cho xuất bản chép trong lúc chờ, nhưng nhãn báo rõ đoạn đó có thể còn thay đổi.
- Không thay đổi dữ liệu, engine hay luồng kết thúc/lưu bản thu.

# 0.6.7 — 30/10/2026 — kết thúc là lưu, không hủy bản chép

- Bấm **Kết thúc** mở cảnh báo xác nhận; xác nhận là dừng ghi ngay, lưu bản thu và giữ bản chép lời hiện có.
- Bỏ nút **Dừng chép, giữ bản thu** sau khi kết thúc.
- Bỏ nút **Chép tiếp bằng local** và luồng interrupted thủ công.
- Phần live chưa hiệu chỉnh vẫn nằm trong cùng danh sách bản chép (đoạn “Chờ hiệu chỉnh”) khi đang xử lý hoặc nếu xử lý bị lỗi; không bị mất khỏi màn hình.
- 44 UI test pass; Svelte 0 lỗi/0 cảnh báo.
- Installer 78.904.275 byte SHA `e2ad5c...4d527`; exe 13.941.760 byte SHA `a2a8d5...99fca`; phiên interrupted hiển thị đủ 4 nút export và không còn nút dừng/chép tiếp.

# 0.6.6 — 30/10/2026 — một bản chép lời duy nhất

- Bỏ box **Chép trực tiếp** tách riêng khỏi bản chép lời.
- Khi đang ghi/đang tạm dừng, text live được nối vào cuối danh sách bản chép như một đoạn đang chờ hiệu chỉnh.
- Khi phiên đã dừng, bị ngắt hoặc hoàn tất, chỉ hiển thị bản chép theo đoạn đã chốt; không còn hai chế độ chép trùng nhau.
- Không thay đổi dữ liệu, engine hay định dạng xuất.
- Installer 78.895.614 byte SHA `714224...8ca8`; phiên interrupted chỉ còn 2 segment, không còn box live/“Chép trực tiếp”.

# 0.6.5 — 29/10/2026 — logo installer/uninstaller

- Gán `installerIcon` và `uninstallerIcon` của NSIS sang logo mới, để file Setup và bộ gỡ cài cũng hiện **trang ghi chú + waveform**, không còn icon NSIS mặc định.
- Không thay đổi app runtime, dữ liệu hay engine.
- Installer 78.900.849 byte SHA `6f14a9...5b687e`; exe 13.941.248 byte SHA `32b61d...ab066`; đã kiểm tra icon installer và uninstaller.

# 0.6.4 — 29/10/2026 — tải MP3, SRT và logo mới

- Thêm nút **Tải .mp3** cho bản ghi âm của phiên; chuyển WAV PCM 16 kHz sang MP3 bằng Windows Media Foundation, không cần FFmpeg/Python.
- Đưa nút **Tải .srt** về cùng kiểu với **Tải .md** và **Tải .txt**, có icon tải xuống.
- Bổ sung nhãn MP3/SRT cho cả 4 ngôn ngữ giao diện.
- Đổi logo mây cũ sang mark **trang ghi chú + waveform**; áp dụng cho sidebar, màn hình chọn ngôn ngữ, empty state, favicon, icon cửa sổ/taskbar và installer.
- 47 Rust test pass (thêm test encode WAV → MP3); Svelte 0 lỗi/0 cảnh báo.
- Installer 78.899.959 byte SHA `45862c...41305`; exe 13.941.248 byte SHA `1b2f0e...ecf14`; MP3 test 913.551 byte, đọc lại được 114,14 giây.

# 0.6.3 — 29/10/2026 — bản thử nghiệm luồng tự cập nhật

- Bản thử nghiệm có version mới để xác nhận end-to-end: app 0.6.2 phát hiện 0.6.3, hiện banner, tải và cài rồi khởi động lại thành 0.6.3.
- Không thay đổi engine ASR, dữ liệu hay định dạng phiên; chỉ bump version và tạo signed updater artifact.
- Release này được publish như bản Latest bình thường để endpoint `releases/latest/download/latest.json` trả về 0.6.3.
- **E2E đã pass**: banner `Đã có bản Innote 0.6.3`; tiến độ tải 13% → 27% → 53% → 82%; app tự cài và mở lại là 0.6.3; 5 phiên và 4 model được giữ nguyên.
- Installer 78.887.480 byte SHA `4db477...65f5b`; exe 13.880.832 byte SHA `47b29d...9a808`; Release https://github.com/quangtu07/innote/releases/tag/v0.6.3.

# 0.6.2 — 29/10/2026 — updater + chuẩn bị phát hành GitHub

- Thêm **Tauri Updater** (`tauri-plugin-updater`) và banner **Đã có bản Innote x.y.z** + nút **Cập nhật ngay** / **Để sau**; chặn cập nhật khi đang ghi/thu thử/sửa.
- Build có `createUpdaterArtifacts`, public key ký, endpoint GitHub Releases; thêm script `configure-github.py` và `make-latest-json.py`.
- Thêm **LICENSE PolyForm Noncommercial 1.0.0** (source-available, không phải OSI open source), README cài đặt/build/phát hành, `.gitignore` cho repo public.
- Repo public chỉ cần source + README + LICENSE + THIRD_PARTY_NOTICES + CHANGELOG; model/runtime lớn, release, test-results và tài liệu nội bộ bị loại khỏi Git.
- **46 Rust/43 UI/0 Svelte warning**; payload 673 tài nguyên. Installer 78.883.574 byte SHA `9dfbaf...`; exe 13.880.832 byte SHA `a3426e...`.
- Bản 0.6.1 chưa có updater; người dùng 0.6.1 cần cài 0.6.2 thủ công một lần.

# 0.6.1 — 29/10/2026 — đổi tên sản phẩm thành Innote

- Đổi tên hiển thị, tiêu đề cửa sổ, sidebar, bộ cài, file chạy và tài liệu từ **Thư ký Mây** sang **Innote**.
- Đổi thư mục dữ liệu mặc định sang `%LOCALAPPDATA%\Innote`; lần mở đầu tự **rename** thư mục cũ `%LOCALAPPDATA%\Thư ký Mây` (cùng ổ đĩa), giữ nguyên phiên/model. Nếu thư mục cũ đang bị khóa, app dùng lại thư mục cũ và thử lại lần sau.
- Giữ `identifier` `vn.thukymay.desktop` để không làm mất dữ liệu WebView/localStorage và thuận tiện cho updater sau này.
- Thư mục xuất mới là `Downloads/Innote`; file cũ trong `Downloads/Thư ký Mây` không bị di chuyển.
- Giữ màn hình chọn 4 ngôn ngữ giao diện của 0.6.0.
- **46 Rust/42 UI/0 Svelte warning**; payload 673 tài nguyên. Installer 78.651.882 byte SHA `b2b471...`; exe 12.845.568 byte SHA `cb7413...`.
- Đã xác nhận migration trên dữ liệu thật: 2 phiên được giữ, app chạy với tiêu đề **Innote**.

# 0.6.0 — 29/10/2026 — chọn ngôn ngữ giao diện 4 thứ tiếng

- Lần mở đầu hiện màn hình **chọn ngôn ngữ giao diện**: Tiếng Việt, English, 한국어, 中文（简体）.
- Lựa chọn lưu tại `localStorage["may.ui_locale"]`; mở lại không hỏi nữa. Có thể đổi trong **Cài đặt → Ngôn ngữ giao diện**.
- Dịch nhãn chính toàn giao diện: sidebar, thiết lập nguồn âm thanh, model, trạng thái, nút ghi, bản chép lời, ghi chú, xuất file, hộp thoại xóa và thông báo kiểm tra âm thanh.
- Ngày/giờ đổi theo locale (`vi-VN`, `en-US`, `ko-KR`, `zh-CN`).
- **44 Rust/42 UI/0 Svelte warning**; ảnh 900×640 cho cổng chọn và 3 ngôn ngữ; payload 673 tài nguyên. Installer 78.641.724 byte SHA `5aca6d...`; exe 12.844.544 byte SHA `d48b89...`.
- Giới hạn: lỗi/cảnh báo do backend Rust sinh ra vẫn là tiếng Việt; tên sản phẩm và tên tác giả giữ nguyên.

# 0.5.10 — 29/10/2026 — liên hệ tác giả cuối file .md/.txt

- File **Markdown** kết thúc bằng mục “Liên hệ tư vấn, hỗ trợ & lập trình”, Alex Linh, Zalo **0962 239 818** (kèm link `zalo.me`) và website **alexlinh.io.vn**.
- File **TXT** kết thúc bằng khối liên hệ tương ứng, số Zalo và URL đầy đủ; nằm sau ghi chú nếu có.
- **SRT không đổi**, không chèn thông tin liên hệ vào phụ đề.
- **44 Rust/40 UI/0 Svelte warning**; kiểm file thật qua `export_session`; payload 673 tài nguyên. Installer 78.632.154 byte SHA `961794...`; exe 12.832.256 byte SHA `6424d3...`.
- Không đổi ASR.

# 0.5.9 — 29/10/2026 — tinh chỉnh thẻ tác giả

- Thẻ tác giả có nền sáng, viền mềm, bo 12 px và đổ bóng nhẹ; tách khỏi đường kẻ Cài đặt.
- Nhãn **TÁC GIẢ** dạng pill + tên **Alex Linh** rõ cấp bậc; role 10,5 px dễ đọc.
- Icon social tăng 31→34 px, bo 9 px, khoảng cách 8 px, hover nổi nhẹ.
- **43 Rust/40 UI/0 Svelte warning**; 900×640 không tràn; payload 673 tài nguyên. Installer 78.641.983 byte SHA `477e0a...`; exe 12.831.744 byte SHA `53f737...`.
- Không đổi ASR hay hành vi.

# 0.5.8 — 29/10/2026 — đưa tác giả lên trên Cài đặt

- Chuyển khối tác giả lên **trên Cài đặt** ở đáy sidebar; đổi nhãn thành **“Tác giả: Alex Linh”**.
- Giữ nguyên role “Tư vấn, hỗ trợ & lập trình” và 3 icon Zalo/LinkedIn/website.
- **43 Rust/40 UI/0 Svelte warning**; payload 673 tài nguyên + exe. Installer 78.636.706 byte SHA `a1d2a3...`; exe 12.831.744 byte SHA `9332f7...`.
- Không đổi ASR; giữ Save As 0.5.5, fix cảnh báo 0.5.6.

# 0.5.7 — 29/10/2026 — khối tác giả Alex Linh ở sidebar

- Thêm khối **Alex Linh — Tư vấn, hỗ trợ & lập trình** ở góc dưới trái sidebar, luôn thấy ở 900×640.
- Ba icon social mở trình duyệt hệ thống qua command allowlist `open_author_link`:
  - Zalo `zalo.me/0962239818`
  - LinkedIn `linkedin.com/in/alex-linh-790794237`
  - Website `alexlinh.io.vn`
- Icon Zalo/LinkedIn vẽ SVG brand-style, website dùng Lucide Globe; có tooltip/aria-label.
- **43 Rust/40 UI/0 Svelte warning**; payload 673 tài nguyên + exe. Installer 78.630.443 byte SHA `a7a1c9...`; exe 12.831.744 byte SHA `9737d4...`.
- Giữ Save As 0.5.5 và fix cảnh báo 0.5.6; không đổi ASR.

# 0.5.6 — 29/10/2026 — bỏ cảnh báo gián đoạn giả

- Chỉ cảnh báo khi **dòng thời gian thực sự mất ≥100 ms**; flag `data_discontinuity` đơn lẻ lúc start/stop hoặc jitter không còn hiện. Không cảnh báo khi đang dừng ghi.
- Cảnh báo mới nói rõ nguồn mất một đoạn âm thanh, giữ dòng thời gian và khuyên kiểm tra thiết bị nếu lặp lại.
- Ẩn cảnh báo cũ dạng chung trên phiên **đã completed/processed=duration**; phiên chưa xong vẫn giữ để người dùng Chép tiếp/kiểm tra thiết bị.
- **43 Rust/39 UI/0 Svelte warning**; payload 673 tài nguyên + exe. Installer 78.629.417 byte SHA `f539e1...`; exe 12.829.184 byte SHA `4e16e3...`.
- Giữ hộp thoại Lưu như 0.5.5; không đổi ASR/accuracy; vẫn không hứa 100%.

# 0.5.5 — 29/10/2026 — hộp thoại chọn chỗ lưu khi xuất

- Bấm **Tải .md / Tải .txt / SRT** mở **hộp thoại Lưu như** của Windows; mặc định vào `Downloads/Thư ký Mây`, gợi ý tên file + đuôi đúng định dạng, có bộ lọc.
- Chọn được thư mục/tên file khác; bấm **Hủy** thì không ghi file. Đường dẫn đã chọn được ghi qua `export_session`; vẫn có nút mở thư mục xuất.
- Thêm dependency `rfd` 0.15 (native file dialog); command `choose_export_path` + `export_session(path)`; UI test mock command này.
- **42 Rust/39 UI/0 Svelte warning**; payload 673 tài nguyên + exe. Installer 78.630.975 byte SHA `626baf...`; exe 12.827.648 byte SHA `a2b2e0...`.
- Không đổi engine/ASR; vẫn không hứa 100%. Hộp thoại native đã kiểm thật (ảnh `save-dialog-screen.png`) và ghi thử đường dẫn chỉ định (`chosen-export.txt`).

# 0.5.4 — 28/10/2026 — đưa nút xuất lên đầu bản chép

- Chuyển **Tải .md / Tải .txt / SRT** từ footer lên tiêu đề “Bản chép lời”, cạnh ô tìm kiếm và nút ghi chú; luôn thấy ở 900×640, không phải cuộn xuống.
- Bỏ select “Định dạng xuất” + nút “Xuất” cũ; mỗi định dạng là một nút một click. File vẫn lưu vào `Downloads/Thư ký Mây`, có nút mở thư mục xuất.
- Không đổi engine/ASR. **42 Rust/39 UI/0 Svelte warning**; payload 673 tài nguyên + exe. Installer 78.623.668 byte SHA `cee7c2...`; exe 12.779.520 byte SHA `5c1c20...`.
- Vẫn không hứa ASR đúng 100%; độ chính xác/latency giữ nguyên số 0.5.2.

# 0.5.3 — 28/10/2026 — bỏ lựa chọn chất lượng, luôn dùng model chính xác cao

- Bỏ dropdown **Chất lượng** khỏi màn chuẩn bị ghi. Việt/Anh luôn gửi `large-v3-turbo-q5_0`; không còn đường chọn `base`/`small` trong UI.
- Bỏ select **Model** ở màn chuẩn bị; chế độ đa ngôn ngữ hiển thị nhãn cố định Nemotron. Settings chỉ liệt kê `large-v3-turbo-q5_0` và `nemotron-3.5-q8`, không còn danh sách 4 model.
- Preferences cũ có `model: base/small` bị bỏ qua; phiên Việt/Anh luôn dùng turbo. UI test cập nhật contract mới; **42 Rust/39 UI/0 Svelte warning**.
- Đây là thay đổi UI/contract, **không sửa nhận dạng**. Đo engine 0.5.2 vẫn p50 555–651 ms/p95 725–822 ms, FLEURS mới 23,67% CER; 100% vẫn không khả thi. Payload 673 tài nguyên + exe; installer 78.621.248 byte SHA `89de5a...`; exe 12.779.520 byte SHA `24aa29...`.

# 0.5.2 — 27/10/2026 — sửa resume CJK và đo word-end thật

- Sửa lặp chữ ở mép cửa sổ Chép tiếp: so khớp theo ký tự Hán/Kana/Hangul (Latin vẫn theo từ), có bằng chứng thời gian, không xoá toàn bộ lượt lặp, không dùng chữ đã sửa làm tham chiếu. Áp dụng cả segment native kề nhau trong cùng cửa sổ (gap ≤50 ms).
- Thêm `speech_end` native vào preview event/audit. Đo end-to-end thật qua WebView2 CDP: audio → native word-end → worker → IPC → DOM sau 2 rAF.
- Thêm `--resume-test` (QA, không mở GUI) chạy đúng vòng 20 s/lookback1,5 s/commit, xuất raw segment từng cửa sổ và giữ chữ sửa.
- Thêm fixture công khai `cjk-resume-public.wav` (50,38 s; Trung/Hàn bắc qua mốc 20 s/40 s) và regression 3 cửa sổ: trước 2 lặp, sau 0, edited preserved.
- Thêm tập FLEURS mới offset 3–4 (8 clip + composite) và script CER; kết quả 23,67% tổng, before/after giống nhau. Không dùng tập này để tune.
- Đo được: 4 ngôn ngữ word-end→render p50 554,7 ms / p95 724,5 ms / max 801,6 ms; CJK p50 651,2 ms / p95 822,4 ms / max 4.257,5 ms. **Chưa đạt mọi từ <500 ms.** IPC p95 ≤1,8 ms; DOM p95 ≤30 ms; backend là phần lớn.
- 42 Rust, 39 UI, Svelte 0/0; native 7 streaming VI/EN, 5 multi, 4 batch, 3 anomaly, 3 quality, capture replay, Unicode payload qua.
- Payload 673 tài nguyên + exe; installer 78.616.222 byte SHA `5c244e...`; portable exe 12.783.616 byte SHA `d9fedc...`; không kèm weights Nemotron. Soniox vẫn opt-in; không dùng API trong test.

# 0.5.1 — 27/10/2026 — sửa chuyển lượt và cải thiện chép local

- Giữ/replay320ms lookahead khi đổi lượt; Silero VAD chốt theo giọng nói sau400ms, thay quy tắc800ms không sinh token.
- Sửa có điều kiện lỗi chữ Hán xen ký tự Thái lặp: chép lại local bằng streamTrung, loại phần âm cuối của lượt trước qua khoảng im lặng số; giữ bản đầu nếu thử lại thất bại.
- Lấy mốc đoạn từ model; chépWAV theo frame, bộ nhớ audio bounded. Bỏ xử lý neural cho im lặng số đầu luồng nhưng giữ clock và mọi sample nonzero, kể cả giọng nhỏ.
- Khóa cấu hình khi đang chuẩn bị ghi; thêm regression cho race start, chuyển ngôn ngữ, khoảng nghỉ, anomaly và job tiếp theo.
- 39Rust/39UI qua. Panel3bài ghép829ký tự:102→65phép sửa, CER12,30%→7,84%; chốt10lượt tương ứng sớm hơn trung vị462ms. Chữ đầu1848→1861ms, chưa nhanh hơn rõ; không đạt mọi từ<500ms. Nhiễu nhỏ xả chậm hơn do chi phíVAD. Chi tiết/giới hạn trong QUALITY_LATENCY.md.
- Không đổi weightsNemotron742MB, không thêm API trả phí. SidecarVAD1,24MBMIT được tái tạo bằng scriptstdlib, pinchecksum và kèm bộ cài.

# 0.5.0 — 26/10/2026 — bản thử đa ngôn ngữ

- Thêm một chế độ tự nhận Việt/Anh/Hàn/Trung xen kẽ theo lượt trong cùng cuộc họp; giữ nguyên chữ, không dịch. Có Hàn/Quan thoại cố định.
- Native NeMo-Speech.cpp 0.2.0 + Nemotron 3.5 Q8 CPU; model tải riêng 741.548.352 byte, checksum/revision cố định, không API/Python. Giấy phép SDK Apache 2.0, weights OpenMDW 1.1.
- Chốt lượt vào bản chép sửa/xuất được; worker hỗ trợ chép sau/khôi phục và offset. Không chạy lại Whisper với một ngôn ngữ cố định để ghi đè chữ đa ngôn ngữ.
- Giữ đường Việt/Anh cũ và Soniox tùy chọn. Đa ngôn ngữ hiện local; kiểm tra mode/model/provider trước ghi.
- 35 Rust, 38 UI, 4 native streaming đa ngôn ngữ, 7 hồi quy Việt/Anh, 4 batch/fixed/recovery qua. Thu Windows có đủ 4 chữ, sửa/xuất Markdown qua; installer đối chiếu 672 tài nguyên + exe.
- Vẫn có lỗi nhận dạng, phần đầu lúc đổi ngôn ngữ có thể mất; chưa đạt mọi từ <500 ms, chưa tách người nói/nói chồng/họp dài. Xem MULTILINGUAL.md.

# Thay đổi

## 0.4.0 — 26/10/2026 — bản thử nội bộ

- Preview local chuyển Vosk → Moonshine Voice 0.1.5 Tiny Streaming Việt/Anh MIT; native DLL và models kèm bản cài, không cần Python/API.
- Feed 20 ms, cadence tối thiểu 80 ms bằng sample counter; giữ decoder cache, cấu hình Việt giảm lookahead 16 xuống 8.
- Tail PCM giữ file handle, mở nguồn xuất hiện muộn, dùng lại buffer; budget cộng cả xử lý audio ở các frame giữa để tránh dồn trễ.
- Giữ đường ghi PCM/Whisper/khôi phục/xuất độc lập; chữ tạm có thể thay đổi. Mốc từ preview chia gần đúng trong câu, Whisper hiệu chỉnh sau.
- Trace capture → DOM sau khung hình chỉ bật khi MAY_LATENCY_AUDIT=1 cho QA; bình thường không ghi trace chữ. Chưa nghiệm thu mọi từ dưới 500 ms.
- Tài nguyên build có checksum và giấy phép; không phân phối model Zipformer CC BY-NC-ND đã nghiên cứu.

## 0.2.1 — 25/10/2026 — bản thử nội bộ

- Nút Kiểm tra âm thanh và Kiểm tra micro riêng ngay cạnh từng nguồn; thêm kiểm tra nguồn đã chọn, thu 5 giây có đếm ngược và nghe lại.
- Kết quả tín hiệu lấy từ backend theo từng nguồn; giữ cảnh báo capture, không kết luận im lặng là thiết bị hỏng.
- Đổi nguồn xóa audio/kết quả thử cũ, dừng player trước bài thử mới; chặn thao tác chuyển màn/ghi khi đang kiểm tra.
- Bổ sung QA luồng người dùng: playback theo timestamp, sửa/tìm/xuất MD/TXT/SRT, chép tiếp, xác nhận/hủy xóa, tải model lỗi và các đường kiểm tra nguồn.

## 0.2.0 — 25/10/2026 — bản thử nội bộ

- Local miễn phí mặc định cho từng cuộc họp; Soniox chỉ dùng khi người dùng chọn và có key.
- Key tùy chọn giữ trong RAM, không lưu xuống dữ liệu phiên/preferences; nhập và xóa tại Cài đặt. Hiện version ứng dụng.
- Chuyển một lần sang local khi Soniox lỗi/quota/mạng/timeout; không tự retry cloud, giữ PCM/chữ đã sửa/watermark. Chép tiếp luôn dùng local.
- Thêm kiểm thử WebSocket giả lập, lỗi provider/cancel, bảo vệ secret và lựa chọn UI; chưa test API Soniox bằng key thật.
- Giữ sửa timeout turbo: nhiều vùng nói của một chunk ngắn dùng một inference window, không nối mất khoảng im lặng.
- Hướng dẫn chuẩn bị dùng chung cho ứng dụng họp/trình duyệt; chưa chứng nhận riêng Meet/Teams hoặc họp dài.

Chưa có GitHub Release công khai hoặc updater hoạt động. Xem SONIOX.md, ZOOM_FIX.md và AUDIT.md cho kiểm chứng/giới hạn; không dùng version này làm lời hứa chất lượng cho mọi cuộc họp.
# 0.3.0 — Chữ trực tiếp local (bản thử)

- Thêm Vosk Việt/Anh streaming trong child riêng, sự kiện chữ tạm trực tiếp lên UI, không đợi Whisper 20 giây.
- Lưu từ đã chốt và export phần chưa hiệu chỉnh; loại đúng phần nháp khi Whisper lưu watermark.
- Xử lý pad im lặng/pause, offset, stop/drain, safe DLL loading, đường dẫn cài đặt có dấu và giới hạn luồng IPC.
- Kèm model/runtime miễn phí; không cần key hoặc Python khi chạy. Whisper/Soniox hiện có tiếp tục hiệu chỉnh ở nền.
- Chưa đạt nghiệm thu latency 100–500 ms; chất lượng bản nháp Việt thấp hơn Whisper. Xem STREAMING.md cho số đo và giới hạn.

