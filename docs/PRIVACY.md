# Dữ liệu và quyền riêng tư

Trang này nói dữ liệu của bạn nằm ở đâu, phần nào được gửi cho mô hình ngôn ngữ, và cách xóa. Phần mềm đang phát
triển: các mục ghi "thiết kế" là cách hệ thống sẽ làm, chưa có trong bản hiện tại. Phần nào đã có, xem mục Trạng thái
trong [ARCHITECTURE.md](ARCHITECTURE.md).

## Nguyên tắc

- Phần mềm chạy trên máy hoặc máy chủ của bạn. Dữ liệu nằm trong Postgres và thư mục dữ liệu của bản cài đó.
- Không có telemetry: phần mềm không gửi gì về cho nhóm phát triển.
- Mô hình ngôn ngữ chỉ viết mô tả cột lúc nạp, chuyển câu hỏi thành SQL (hay form của câu "tại sao") và viết lời cho
  kết quả đã tính. Con số do engine phân tích tính trên máy của bạn, nên không cần gửi cả bảng dữ liệu cho mô hình.

## Dữ liệu nằm ở đâu

| Dữ liệu | Nơi lưu |
|---|---|
| File bạn nạp lên | Chuyển sang Parquet, lưu trong thư mục dữ liệu (`DATA_DIR`, volume `xdidata` khi chạy compose), mỗi workspace một thư mục, trong đó mỗi dataset một thư mục, mỗi phiên bản một thư mục con. File gốc chỉ nằm trong thư mục tạm lúc nạp và bị xóa ngay sau đó; nếu backend dừng đột ngột giữa lúc nạp, lần khởi động sau xóa |
| Hồ sơ dữ liệu: tên cột, kiểu, vai trò, min, max, số ô trống, giá trị và số dòng của cột tối đa 200 giá trị khác nhau, đơn vị, cờ nhạy cảm, mô tả, bí danh, câu hỏi gợi ý | Postgres |
| Lịch sử câu hỏi: người hỏi, câu hỏi, SQL mô hình viết và SQL sau guard, trạng thái, lỗi (có thể chép một giá trị của dữ liệu), thời gian các bước, số dòng kết quả, cuộc trò chuyện chứa câu hỏi và phần "Đã hiểu là" dạng chữ ngắn; với câu "tại sao", form đã điền và bảng phân rã (giá trị nhóm và con số đã tính). Bảng kết quả không lưu | Postgres |
| Tài khoản: email, tên, mật khẩu đã băm (Argon2id) | Postgres; không lưu mật khẩu gốc |
| Phiên đăng nhập: mã băm SHA-256, người dùng, hạn | Postgres; mã gốc chỉ nằm trong cookie của trình duyệt, xóa khi đăng xuất |

## Gửi gì cho mô hình ngôn ngữ

Tùy biến `LLM_PROVIDER`:

| Giá trị | Dữ liệu đi đâu |
|---|---|
| `mock` (mặc định) | Không gửi đi đâu |
| `ollama` | Tới máy chạy Ollama ở `OLLAMA_URL`. Ollama chạy trong mạng của bạn thì dữ liệu không ra ngoài |
| `gemini` | Tới API của Google, theo [điều khoản Gemini API](https://ai.google.dev/gemini-api/terms). Với gói miễn phí (khóa thuộc dự án Google Cloud chưa gắn tài khoản thanh toán), điều khoản cho phép Google dùng nội dung gửi lên và câu trả lời để cải thiện sản phẩm, có thể có người đọc. Gói trả phí không dùng cho việc đó, nhưng Google vẫn lưu một thời gian để phát hiện vi phạm. Dữ liệu nhạy cảm thì dùng Ollama |

Nội dung gửi:

| Lúc | Gửi | Không gửi |
|---|---|---|
| Nạp dữ liệu, lập hồ sơ ngữ nghĩa (mô tả, bí danh cột và giá trị, câu hỏi gợi ý) | Tên dataset, tên bảng logic, số dòng; mỗi cột: tên, kiểu, vai trò, tỷ lệ ô trống, số giá trị khác nhau; min, max của cột thời gian và cột độ đo số thực không nhạy cảm; giá trị kèm số dòng của cột phân loại ít giá trị (tối đa 50 giá trị một cột, 400 giá trị một lần, bỏ giá trị dài quá 200 byte) | Các dòng dữ liệu; giá trị của cột chữ tự do, cột khóa và cột nhạy cảm (backend tự đánh dấu theo tên cột như email, mật khẩu, số điện thoại, địa chỉ, họ tên, và theo giá trị có dạng email hay số điện thoại) |
| Hỏi | Câu hỏi; tên, kiểu, mô tả, bí danh của các cột: cả bảng khi bảng tới 60 cột, bảng nhiều cột hơn thì chỉ các cột khớp câu hỏi và cột thời gian (không cột nào khớp thì cả bảng); danh sách giá trị của mỗi cột phân loại (cột chiều) không đánh dấu nhạy cảm, kèm bí danh: đủ khi cột tới 50 giá trị, 20 giá trị nhiều dòng nhất khi nhiều hơn, tối đa 400 giá trị mỗi câu hỏi; định nghĩa chỉ số của dataset (tên, công thức, điều kiện, mô tả; prompt form câu "tại sao" chỉ có tên và mô tả); khái niệm của gói ngành gắn với dataset (chưa có cách gắn gói, nên hiện không gửi); lựa chọn người hỏi gửi kèm; tối đa 3 câu hỏi đã được đánh dấu trả lời đúng của cùng dataset, có chung từ với câu hỏi, kèm SQL của chúng (chỉ prompt câu thường; chưa có cách đánh dấu, nên hiện không gửi) | Các dòng dữ liệu; giá trị của cột đánh dấu nhạy cảm; dữ liệu của workspace khác; đường dẫn file |
| Hỏi tiếp trong một cuộc trò chuyện | Thêm vào prompt câu thường và prompt form câu "tại sao": tối đa 4 lượt gần nhất của cùng cuộc, mỗi lượt là câu hỏi, trạng thái (đã trả lời, bị từ chối, lỗi), câu SQL mô hình viết (hoặc SQL sau guard khi không có) và phần "Đã hiểu là" dạng chữ ngắn (độ đo, bộ lọc, nhóm, kỳ; không gồm điều kiện phân quyền); cả khối tối đa 4 000 ký tự, dài hơn thì bỏ lượt cũ trước. Câu SQL và bộ lọc có thể chứa giá trị của cột phân loại, như phần danh sách giá trị ở dòng trên | Các dòng dữ liệu; giá trị trong bảng kết quả; lời báo lỗi của các lượt; lượt của cuộc khác hay của người khác |
| SQL chạy lỗi hoặc bị guard từ chối, sửa một lần | Prompt của lần hỏi, SQL vừa sinh và thông báo lỗi của DuckDB hay lý do guard từ chối. Thông báo của DuckDB có thể chép một giá trị của dữ liệu, ví dụ chuỗi ngày không đọc được | Đường dẫn file trên máy chủ |
| Form câu "tại sao" sai, sửa một lần | Prompt của lần hỏi, form vừa điền và lỗi của từng trường. Lỗi có thể kèm tối đa 20 giá trị của một cột phân loại (đã có trong prompt) và ngày đầu, ngày cuối của cột thời gian | Các dòng dữ liệu; đường dẫn file |
| Diễn giải câu "tại sao" so hai kỳ | Câu hỏi và bảng phân rã đã tính: độ đo, nhãn và giá trị hai lát cắt, chênh lệch, nhóm chính (`main_group`: nhóm đóng góp lớn nhất cùng chiều với chênh lệch ở chiều đầu), hai chiều đầu, mỗi chiều ba nhóm đầu kèm giá trị nhóm, giá trị hai lát cắt và phần đóng góp (độ đo tỷ lệ thêm phần cơ cấu và phần hiệu suất), tách lượng và giá nếu có. Câu so hai nhóm, hay chênh lệch bằng 0, thì không gửi gì: lời giải thích là câu khuôn do backend viết | Các dòng dữ liệu gốc |
| Trả lời câu thường | Câu hỏi, phần "Đã hiểu là" (nhãn độ đo, nhóm, bộ lọc), tên cột kết quả, tối đa 10 dòng đầu của kết quả và tổng số dòng. Đây là dữ liệu kết quả thật: giá trị nhóm và con số đã tính, có thể chứa giá trị của mọi cột câu SQL chọn. Tắt bằng `ANSWER_TEXT=false`: không gửi dòng kết quả nào, đoạn trả lời là câu khuôn do backend viết | Các dòng dữ liệu gốc; dòng thứ 11 trở đi của kết quả; câu SQL |

Khung "Tổng quan dữ liệu" (`GET .../datasets/{id}/overview`) không gửi gì cho mô hình: hồ sơ cột đọc từ Postgres, biểu
đồ do backend tính trên DuckDB. Kết quả các câu của nó giữ trong bộ nhớ của backend theo phiên bản, mất khi khởi động
lại.

Nguyên văn phần chữ cố định của từng prompt (luật, khuôn câu, chỗ trống nơi dữ liệu ở bảng trên được điền vào) nằm
trong [`backend/prompts/`](https://github.com/pgiahuy/xdi-platform/tree/main/backend/prompts), trừ prompt diễn giải
câu "tại sao", còn trong `backend/src/service/explain/mod.rs`.

Lưu ý: giá trị mẫu và giá trị phân loại là dữ liệu thật của bạn, ví dụ tên khu vực hay tên sản phẩm. Cột chữ có ít
giá trị khác nhau được tính là cột phân loại; backend đánh dấu cột nhạy cảm theo tên cột và dạng giá trị ngay khi nạp,
nhưng một cột không khớp luật (ví dụ tên khách hàng đặt tên khác thường) vẫn có thể được gửi cho tới khi mô hình
đánh dấu thêm lúc lập hồ sơ ngữ nghĩa; chưa có màn để admin đánh dấu tay. Nếu dữ liệu nhạy cảm, dùng `mock` hoặc
Ollama chạy nội bộ.

Không bao giờ gửi: mật khẩu, khóa API của bạn cho nhà cung cấp khác, token đăng nhập.

## Xóa dữ liệu

- Xóa toàn bộ một bản cài bằng compose: `docker compose down -v`. Lệnh này xóa cả hai volume (`pgdata` của Postgres
  và `xdidata` chứa file dữ liệu). `docker compose down` không có `-v` thì giữ dữ liệu.
- Admin xóa workspace ở trang Thành viên (hoặc `DELETE /api/workspaces/{ws}`): xóa mọi dataset, hồ sơ, chỉ số, lịch
  sử câu hỏi và thành viên của nó trong Postgres (khóa ngoại `ON DELETE CASCADE`) và thư mục Parquet của workspace.
- Admin xóa từng dataset ở trang Dữ liệu (hoặc `DELETE /api/workspaces/{ws}/datasets/{id}`): xóa mọi phiên bản, hồ
  sơ cột, chỉ số và cuộc trò chuyện của dataset trong Postgres và file Parquet của từng phiên bản trên đĩa. Lịch sử câu
  hỏi của dataset (câu hỏi, SQL, lỗi, bảng phân rã của câu "tại sao") vẫn còn trong Postgres, chỉ bỏ liên kết tới
  dataset; nó mất khi xóa cả workspace.

## Bản demo công khai (thiết kế)

Khi chạy với chế độ demo:

- Mỗi phiên trình duyệt có một workspace tạm, tự xóa sau 24 giờ. Hai phiên không thấy dữ liệu của nhau.
- Workspace mẫu chỉ đọc.
- Đừng nạp dữ liệu nhạy cảm lên bản demo của người khác. Bản demo dùng nhà cung cấp mô hình nào thì nội dung ở mục trên
  được gửi tới đó.
