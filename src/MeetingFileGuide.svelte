<script lang="ts">
  export let library=false;
  export let legacy=false;
  const files=[
    ['00 - ĐỌC TÔI TRƯỚC.txt','Hướng dẫn cuộc họp','Mở bằng Notepad để biết từng file dùng làm gì và bản nào đã được tạo.'],
    ['recording.wav','Bản ghi âm','Nghe lại âm thanh đã thu từ những nguồn loa/micro bạn bật.'],
    ['original.md','Bản chép gốc','Lời nhận dạng trong phiên ghi và sửa tay đã lưu. Đối chiếu với audio khi có nghi ngờ.'],
    ['original.srt','Phụ đề bản gốc','Nội dung gốc kèm mốc thời gian, dùng với trình phát hỗ trợ SRT.'],
    ['corrected.md / corrected.srt','Bản hiệu chỉnh','Kết quả model nghe lại file ghi âm khi bấm Hiệu chỉnh; không thay bản gốc.'],
    ['translations/vi.md, en.md…','Bản dịch','Mỗi file là một ngôn ngữ đích: vi = Việt, en = Anh, ko = Hàn, zh = Trung, ja = Nhật.'],
    ['summary.md','Bản tóm tắt','Nội dung rút gọn từ bản gốc hoặc bản hiệu chỉnh bạn chọn.'],
    ['notes.md','Ghi chú','Nội dung bạn tự nhập trong Innote, tách khỏi lời nhận dạng.'],
    ['meeting.json','Dữ liệu cho AI/phần mềm','Các bản nội dung riêng, ID, thời gian và đường dẫn audio. Không chứa cấu hình/API key.'],
    ['exports/','File xuất thủ công','Các file TXT, Markdown, SRT, MP3 bạn chủ động xuất vào đây. Đích khác vẫn được tôn trọng.'],
    ['.innote/work/','Dữ liệu làm việc của cuộc họp','PCM nguồn và file phục hồi/xử lý; không cần mở để đọc bản chép. Không tự sửa/xóa.'],
    ['.innote-identity.json','Nhận diện cuộc họp','Giúp app tìm lại đúng cuộc họp khi folder đổi tên. Không sửa nội dung.'],
  ];
</script>

<details class="file-guide">
  <summary>Các tệp và thư mục dùng để làm gì?</summary>
  {#if legacy}
    <p><strong>Cuộc họp ở nơi lưu cũ:</strong> bản chép gốc, bản dịch, hiệu chỉnh, tóm tắt và ghi chú nằm trong cơ sở dữ liệu của Innote, chưa phải các file Markdown riêng trong folder này.</p>
    <p><code>recording.wav</code> là bản ghi âm đã hoàn tất. Các file <code>system.pcm</code>, <code>microphone.pcm</code> và WAV tạm là dữ liệu xử lý/phục hồi; không tự sửa/xóa.</p>
    <p>Muốn chia sẻ bản chép cho AI, chọn đúng phiên bản trong Innote rồi xuất Markdown/TXT/SRT vào một thư mục riêng. Không cấp quyền toàn bộ nơi lưu cũ vì nơi đó có thể còn chứa cấu hình/API key.</p>
  {:else}
  {#if library}
    <p>Mỗi thư mục có ngày, giờ và tên là một cuộc họp. File <strong>00 - HƯỚNG DẪN THƯ VIỆN INNOTE.txt</strong> giải thích cách dùng thư viện ngay trong Explorer.</p>
    <p><strong>.innote/ ở cấp thư viện</strong> chứa DB gốc <code>meetings.sqlite</code>, nhận diện <code>library.json</code>, khóa ghi, thông tin phục hồi và <code>audio-tests/</code> (bản thu thử thiết bị). Đây là dữ liệu quan trọng; không tự sửa/xóa. Muốn sao lưu đầy đủ, thoát app rồi sao lưu cả thư viện.</p>
  {/if}
  <p>Trong <strong>từng thư mục cuộc họp</strong>:</p>
  <table><thead><tr><th scope="col">Tên file / thư mục</th><th scope="col">Mục đích</th></tr></thead><tbody>
    {#each files as [name,title,description]}<tr><td><code>{name}</code></td><td><strong>{title}</strong><span>{description}</span></td></tr>{/each}
  </tbody></table>
  <p>Chỉ tạo file khi có dữ liệu tương ứng. Chưa bấm Hiệu chỉnh, chưa dịch hoặc chưa tóm tắt thì chưa có những bản đó. Các file đang ghi có thể còn cập nhật.</p>
  <p>Để sửa nội dung trong app, hãy sửa ở Innote. Sửa file bên ngoài không cập nhật ngược vào Innote; hãy tạo bản sao nếu muốn biên tập riêng. Chỉ chia sẻ thư mục cuộc họp cần dùng, không cần cấp quyền thư mục cấu hình/API key cho AI.</p>
  {/if}
</details>

<style>
  .file-guide{margin:16px 0;font-size:.9rem;line-height:1.6}.file-guide summary{cursor:pointer;color:var(--accent,#146c55);font-weight:600;padding:8px 0}.file-guide p{max-width:76ch;margin:12px 0}table{width:100%;border-collapse:collapse;table-layout:fixed}th,td{text-align:left;vertical-align:top;padding:12px 8px;border-bottom:1px solid var(--border,#dbe5df);overflow-wrap:anywhere}th:first-child,td:first-child{width:38%;padding-left:0}th{font-weight:600}td strong,td span{display:block}td span{margin-top:3px;color:var(--muted,#536b61)}code{font-size:.9em;white-space:normal;overflow-wrap:anywhere}summary:focus-visible{outline:2px solid var(--accent,#146c55);outline-offset:3px}
</style>
