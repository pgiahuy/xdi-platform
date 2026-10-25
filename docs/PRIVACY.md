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
| File bạn nạp lên | Chuyển sang Parquet, lưu trong thư mục dữ liệu (`DATA_DIR`, volume `xdidata` khi chạy compose), mỗi workspace một thư mục, mỗi phiên bản một thư mục con. File gốc chỉ nằm trong thư mục tạm lúc nạp và bị xóa ngay sau đó; nếu backend dừng đột ngột giữa lúc nạp, lần khởi động sau xóa |
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
| `gemini` | Tới API của Google, theo [điều khoản Gemini API](https://ai.google.dev/gemini-api/terms). Với gói miễn phí (khóa thuộc dự án Google Cloud chưa gắn tài khoản thanh toán), điều khoản cho phép Google dùng nội dung gửi lên và câu trả lời để cải thiện sản phẩm, có thể có người đọc. Gói trả phí không dùng cho việc đó, nhưng Google vẫn lưu một thời gian để phát hiện vi phạm. Dữ liệu nhạy cảm thì dùng Ollama |

Nội dung gửi (thiết kế):

| Lúc | Gửi | Không gửi |
|---|---|---|
| Nạp dữ liệu, viết mô tả cột | Tên cột, kiểu, min, max, vài giá trị mẫu của mỗi cột, giá trị phân loại của cột ít giá trị | Các dòng dữ liệu |
| Hỏi | Câu hỏi; tên, kiểu, mô tả, bí danh của các cột (cả bảng khi bảng tới 60 cột); danh sách giá trị của mỗi cột phân loại (cột chiều) không đánh dấu nhạy cảm, kèm bí danh: đủ khi cột tới 50 giá trị, 20 giá trị nhiều dòng nhất khi nhiều hơn, tối đa 400 giá trị mỗi câu hỏi; vài câu hỏi đã trả lời đúng trước đó trong cùng workspace | Các dòng dữ liệu; giá trị của cột đánh dấu nhạy cảm; dữ liệu của workspace khác |
| SQL chạy lỗi hoặc dùng bảng, cột không có, sửa một lần | Prompt của lần hỏi, SQL vừa sinh và thông báo lỗi của DuckDB hay lý do guard từ chối. Thông báo của DuckDB có thể chép một giá trị của dữ liệu, ví dụ chuỗi ngày không đọc được | Đường dẫn file trên máy chủ |
| Diễn giải câu "tại sao" | Bảng phân rã đã tính: nhóm và phần đóng góp của từng nhóm | Các dòng dữ liệu gốc |

Lưu ý: giá trị mẫu và giá trị phân loại là dữ liệu thật của bạn, ví dụ tên khu vực hay tên sản phẩm. Cột chữ có ít
giá trị khác nhau được tính là cột phân loại, nên ở bảng nhỏ một cột như tên khách hàng hay địa chỉ cũng có thể được
gửi danh sách giá trị cho tới khi được đánh dấu nhạy cảm. Nếu dữ liệu nhạy cảm, dùng `mock` hoặc Ollama chạy nội bộ.

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
