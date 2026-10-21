# Dữ liệu và quyền riêng tư

Trang này nói dữ liệu của bạn nằm ở đâu, phần nào được gửi cho mô hình ngôn ngữ, và cách xóa. Phần mềm đang phát
triển: các mục ghi "thiết kế" là cách hệ thống sẽ làm, chưa có trong bản hiện tại. Phần nào đã có, xem mục Trạng thái
trong [ARCHITECTURE.md](ARCHITECTURE.md).

## Nguyên tắc

- Phần mềm chạy trên máy hoặc máy chủ của bạn. Dữ liệu nằm trong Postgres và thư mục dữ liệu của bản cài đó.
- Không có telemetry: phần mềm không gửi gì về cho nhóm phát triển.
- Mô hình ngôn ngữ chỉ chuyển câu hỏi thành SQL và diễn giải kết quả đã tính. Con số do engine phân tích tính trên máy
  của bạn, nên không cần gửi cả bảng dữ liệu cho mô hình.

## Dữ liệu nằm ở đâu

| Dữ liệu | Nơi lưu |
|---|---|
| File bạn nạp lên (thiết kế) | Chuyển sang Parquet, lưu trong thư mục dữ liệu (`DATA_DIR`, volume `xdidata` khi chạy compose), mỗi workspace một thư mục |
| Hồ sơ dữ liệu (thiết kế): tên cột, kiểu, min, max, số ô trống, vài giá trị mẫu, giá trị phân loại, mô tả, bí danh | Postgres |
| Lịch sử câu hỏi (thiết kế): câu hỏi, SQL đã chạy, trạng thái | Postgres |
| Tài khoản: email, tên, mật khẩu đã băm (Argon2id) | Postgres; không lưu mật khẩu gốc |
| Phiên đăng nhập: mã băm SHA-256, người dùng, hạn | Postgres; mã gốc chỉ nằm trong cookie của trình duyệt, xóa khi đăng xuất |

## Gửi gì cho mô hình ngôn ngữ

Tùy biến `LLM_PROVIDER`:

| Giá trị | Dữ liệu đi đâu |
|---|---|
| `mock` (mặc định) | Không gửi đi đâu |
| `ollama` | Tới máy chạy Ollama ở `OLLAMA_URL`. Ollama chạy trong mạng của bạn thì dữ liệu không ra ngoài |
| `gemini` | Tới API của Google, theo điều khoản của Google |

Nội dung gửi (thiết kế):

| Lúc | Gửi | Không gửi |
|---|---|---|
| Nạp dữ liệu, viết mô tả cột | Tên cột, kiểu, min, max, vài giá trị mẫu của mỗi cột, giá trị phân loại của cột ít giá trị | Các dòng dữ liệu |
| Hỏi | Câu hỏi; các cột liên quan kèm mô tả, bí danh; giá trị phân loại khớp với câu hỏi; vài câu hỏi đã trả lời đúng trước đó trong cùng workspace | Dữ liệu của workspace khác |
| SQL chạy lỗi, sửa một lần | SQL vừa sinh và thông báo lỗi | |
| Diễn giải câu "tại sao" | Bảng phân rã đã tính: nhóm và phần đóng góp của từng nhóm | Các dòng dữ liệu gốc |

Lưu ý: giá trị mẫu và giá trị phân loại là dữ liệu thật của bạn, ví dụ tên khu vực hay tên sản phẩm. Nếu dữ liệu nhạy
cảm, dùng `mock` hoặc Ollama chạy nội bộ.

Không bao giờ gửi: mật khẩu, khóa API của bạn cho nhà cung cấp khác, token đăng nhập.

## Xóa dữ liệu

- Xóa toàn bộ một bản cài bằng compose: `docker compose down -v`. Lệnh này xóa cả hai volume (`pgdata` của Postgres
  và `xdidata` chứa file dữ liệu). `docker compose down` không có `-v` thì giữ dữ liệu.
- Xóa một workspace trong Postgres xóa theo mọi dataset, hồ sơ, chỉ số, lịch sử câu hỏi và thành viên của nó (khóa
  ngoại `ON DELETE CASCADE`). File Parquet của workspace đó nằm ở thư mục riêng trong thư mục dữ liệu.
- Xóa từng dataset từ giao diện: thiết kế, chưa có.

## Bản demo công khai (thiết kế)

Khi chạy với chế độ demo:

- Mỗi phiên trình duyệt có một workspace tạm, tự xóa sau 24 giờ. Hai phiên không thấy dữ liệu của nhau.
- Workspace mẫu chỉ đọc.
- Đừng nạp dữ liệu nhạy cảm lên bản demo của người khác. Bản demo dùng nhà cung cấp mô hình nào thì nội dung ở mục trên
  được gửi tới đó.
