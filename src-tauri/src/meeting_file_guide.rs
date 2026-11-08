//! Human-readable help kept beside the portable meeting files.
use crate::types::Session;
use anyhow::Result;
use std::path::Path;

pub const MEETING_GUIDE: &str = "00 - ĐỌC TÔI TRƯỚC.txt";
pub const LIBRARY_GUIDE: &str = "00 - HƯỚNG DẪN THƯ VIỆN INNOTE.txt";

pub fn ensure_library_guide(root: &Path) -> Result<()> {
    let file = root.join(LIBRARY_GUIDE);
    // This is a shared root: never overwrite an existing user document.
    if !file.try_exists()? {
        crate::storage_paths::atomic_write(&file, library_guide().as_bytes())?;
    }
    Ok(())
}

pub fn library_guide() -> &'static str {
    "THƯ VIỆN CUỘC HỌP INNOTE\n\
=========================\n\n\
Các thư mục có dạng ngày_giờ - tên cuộc họp: mỗi thư mục là một cuộc họp.\n\
Mở file 00 - ĐỌC TÔI TRƯỚC.txt bên trong để biết bản gốc, bản dịch, bản hiệu chỉnh và bản ghi âm ở đâu.\n\n\
.innote/ ở cấp thư viện là dữ liệu nội bộ quan trọng của Innote:\n\
  meetings.sqlite và các file -wal/-shm: cơ sở dữ liệu gốc của toàn thư viện, gồm nội dung và đường dẫn cuộc họp. Không sửa/xóa từng file.\n\
  library.json: nhận diện thư viện để app tìm đúng nơi lưu khi mở lại.\n\
  writer.lock: giúp tránh hai tiến trình cùng ghi vào thư viện. Không cần tự xóa.\n\
  operations/: thông tin phục hồi thao tác đổi tên khi có gián đoạn.\n\
  audio-tests/: các bản thu thử thiết bị, KHÔNG phải bản ghi cuộc họp.\n\n\
MUỐN ĐỌC/NGHE/CHIA SẺ: mở thư mục cuộc họp tương ứng. Không cần mở .innote/ để đọc bản chép.\n\
MUỐN DÙNG AI KHÁC: chỉ cấp quyền thư mục cuộc họp cần phân tích; AI có thể đọc meeting.json hoặc các file .md. Không cần cấp thư mục cấu hình/API key của Innote.\n\
MUỐN SAO LƯU: thoát Innote sạch sẽ rồi sao lưu cả thư viện, gồm .innote/ và các thư mục cuộc họp. Chép vài file .md/.wav chỉ là chia sẻ nội dung, không phải bản sao lưu đầy đủ của app.\n\n\
Đổi nơi lưu trong Cài đặt chỉ áp dụng cho cuộc họp mới. Cuộc họp cũ ở nguyên nơi cũ. Không tự di chuyển/xóa dữ liệu cũ.\n"
}

pub fn ensure_meeting_guide(folder: &Path, s: &Session) -> Result<()> {
    let file = folder.join(MEETING_GUIDE);
    if !file.try_exists()? {
        crate::storage_paths::atomic_write(&file, meeting_guide(s, folder).as_bytes())?;
    }
    Ok(())
}

pub fn meeting_guide(s: &Session, folder: &Path) -> String {
    let state = |available| if available { "ĐÃ CÓ" } else { "CHƯA CÓ" };
    let has = |name: &str| folder.join(name).is_file();
    let status = match s.status.as_str() {
        "recording" => "Đang ghi",
        "paused" => "Tạm dừng",
        "processing" => "Đang xử lý",
        "stopping" => "Đang kết thúc",
        "completed" => "Đã hoàn tất",
        "recorded" => "Đã thu âm (không chép lời)",
        "interrupted" => "Bị gián đoạn",
        other => other,
    };
    let source = match s.summary.as_ref().map(|x| x.source.as_str()) {
        Some("corrected") => "Bản hiệu chỉnh",
        Some("smoothed") => "Bản biên tập cũ",
        Some(_) => "Bản gốc",
        None => "Chưa tạo tóm tắt",
    };
    let languages = s
        .translations
        .iter()
        .map(|t| t.language.as_str())
        .collect::<std::collections::BTreeSet<_>>()
        .into_iter()
        .collect::<Vec<_>>()
        .join(", ");
    format!("CUỘC HỌP: {}\nNgày tạo: {}\nTrạng thái trong Innote: {}\n\n\
CÁCH DÙNG NHANH\n\
- Muốn đọc nội dung đang lưu: mở original.md.\n\
- Muốn nghe lại để đối chiếu: mở recording.wav, hoặc nghe theo từng đoạn trong Innote.\n\
- Đã chạy Hiệu chỉnh? Mở corrected.md để đọc bản nghe lại từ audio; original.md vẫn được giữ riêng.\n\
- Muốn đọc bản dịch: mở translations/ và chọn file theo ngôn ngữ.\n\n\
TỆP NÀO LÀ BẢN NÀO?\n\
[{}] recording.wav — BẢN GHI ÂM. Âm thanh cuộc họp đã hoàn tất, gồm những nguồn loa/micro bạn đã bật. Không phải file văn bản.\n\
[{}] original.md — BẢN CHÉP GỐC. Lời nhận dạng từ phiên ghi và những sửa tay bạn đã lưu trong Innote; không phải bản dịch hay bản hiệu chỉnh. Nhận dạng có thể sai, hãy đối chiếu audio.\n\
[{}] original.srt — PHỤ ĐỀ CỦA BẢN GỐC. Cùng nội dung gốc, có mốc thời gian để dùng với trình phát hỗ trợ SRT.\n\
[{}] corrected.md — BẢN HIỆU CHỈNH. Model nghe lại file ghi âm khi bạn bấm Hiệu chỉnh; là một bản riêng, vẫn có thể sai. Không thay bản chép gốc.\n\
[{}] corrected.srt — PHỤ ĐỀ CỦA BẢN HIỆU CHỈNH, có mốc thời gian của kết quả hiệu chỉnh.\n\
[{}] translations/ — CÁC BẢN DỊCH. Ví dụ vi.md = tiếng Việt, en.md = tiếng Anh, ko.md = tiếng Hàn, zh.md = tiếng Trung, ja.md = tiếng Nhật. Ngôn ngữ hiện có: {}. Không tạo thư mục này nếu chưa có bản dịch.\n\
[{}] summary.md — BẢN TÓM TẮT do AI tạo. Nguồn tóm tắt: {}. Không phải bản chép đầy đủ.\n\
[{}] notes.md — GHI CHÚ bạn nhập và lưu trong Innote, tách khỏi lời nhận dạng.\n\
meeting.json — DỮ LIỆU CÓ CẤU TRÚC cho AI/phần mềm: các bản nội dung riêng, ID, mốc thời gian, trạng thái, đường dẫn audio tương đối và manifest. Không chứa cấu hình/API key.\n\
exports/ — CÁC FILE BẠN CHỦ ĐỘNG XUẤT từ Innote, như TXT/Markdown/SRT/MP3. Chỉ xuất hiện khi có file được xuất vào đây; bạn cũng có thể đã chọn đích khác. MP3 không được tự tạo để tránh nhân đôi audio.\n\n\
CÁC MỤC NỘI BỘ — KHÔNG CẦN MỞ ĐỂ ĐỌC BẢN CHÉP\n\
.innote/work/ — PCM nguồn và file làm việc để chép lời/phục hồi. Đây không phải bản nghe lại đã hoàn tất. Không tự sửa/xóa, nhất là khi đang ghi hoặc hiệu chỉnh.\n\
.innote-identity.json — mã nhận diện để Innote tìm lại cuộc họp nếu thư mục được đổi tên. Không phải bản chép; không sửa nội dung.\n\
*.user-copy-*.md — bản giữ lại khi file do app tạo từng được sửa bên ngoài và đầu ra đó không còn được dùng. Không phải bản gốc đang lưu trong DB; hãy đối chiếu trước khi sử dụng.\n\n\
LƯU Ý\n\
- CHƯA CÓ nghĩa là dữ liệu chưa được tạo, không có file rỗng thay thế. Audio có thể chưa hoàn tất nếu phiên còn đang ghi hoặc bị lỗi.\n\
- Trong lúc đang ghi, các file là bản đang cập nhật; nên dùng trạng thái trong meeting.json/Innote để biết phiên đã kết thúc chưa.\n\
- Các file .md/.srt/.json và hướng dẫn này do app cập nhật từ dữ liệu đang lưu. Sửa file bên ngoài KHÔNG sửa bản chép trong Innote. Muốn biên tập riêng, hãy tạo bản sao hoặc dùng exports/.\n\
- Thư mục có thể chứa thông tin nhạy cảm của cuộc họp. Chỉ chia sẻ dữ liệu cần thiết.\n",
        s.title,s.created,status,state(has("recording.wav")),state(has("original.md")),state(has("original.srt")),state(has("corrected.md")),state(has("corrected.srt")),state(s.translations.iter().any(|t|has(&format!("translations/{}.md",t.language)))),if languages.is_empty(){"chưa có"}else{&languages},state(has("summary.md")),source,state(has("notes.md")))
}
