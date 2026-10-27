# Changelog

Theo [Keep a Changelog](https://keepachangelog.com/en/1.1.0/). Số phiên bản theo [Semantic Versioning](https://semver.org/lang/vi/).

## [Unreleased]

### Added

- Dataset mẫu (#25): thiết lập lần đầu nạp sẵn DataCo (1500 dòng) kèm chỉ số mẫu vào workspace đầu tiên, tắt bằng `SEED_SAMPLE=false`; `data/download.sh` tải bản đầy đủ DataCo và Online Retail II.
- Bộ đánh giá câu hỏi mẫu (#37): `make eval` chạy 14 câu mẫu trên dữ liệu DataCo thu nhỏ (`data/samples/dataco.csv`, CC BY 4.0), so kết quả với SQL đúng viết tay và báo accuracy; CI đỏ khi accuracy giảm dưới `eval/baseline.json`. Chạy được với mock (mặc định) và với mô hình thật qua `LLM_PROVIDER`.
- Xóa dataset (#160): admin bấm **Xóa** ở trang Dữ liệu để xóa hẳn một dataset cùng mọi phiên bản, hồ sơ, lịch sử câu hỏi và file.
- Câu "tại sao" nhận thêm cách hỏi ("sao lại", "vì đâu") và cảnh báo khi câu hỏi nói giảm mà số thật tăng hay ngược lại (#164).
- Câu hỏi cần thông tin dataset không có (mức hài lòng, dự báo) được báo rõ kèm các cột gần nhất có thể hỏi, thay vì chạy một câu SQL lạc đề (#164).

### Changed

- Màn đăng nhập và thiết lập lần đầu (#159): form nằm cạnh panel giới thiệu sản phẩm, phiên bản máy chủ ở dưới form.
- Khung ứng dụng (#161): thanh bên có logo và icon cho từng mục, tài khoản chuyển xuống chân thanh bên, nội dung căn giữa trên màn rộng.
- Ở trang quản trị hệ thống, thanh bên vẫn giữ các mục của workspace đang làm việc thay vì ẩn đi.
- Câu "tại sao" cho chỉ số tỷ lệ và trung bình (ví dụ tỷ lệ giao trễ): chênh lệch tách thành phần do cơ cấu và phần do hiệu suất của từng nhóm.
- Giao diện mới "Mực và hổ phách": thanh bên nền mực, màu nhấn hổ phách cho nút chính và số chênh lệch, font Be Vietnam Pro đóng gói sẵn, chạy không cần mạng.

## [0.1.0] - 2026-10-26

### Added

- Nền tảng: backend Rust (Axum, DuckDB nhúng, Postgres), giao diện React và Vite do backend phục vụ cùng cổng, một image Docker chứa cả hai; hướng dẫn cài đặt, đưa lên Internet, sao lưu và khôi phục ở `docs/INSTALL.md`.
- Tài khoản và workspace: mã thiết lập in ra log lần chạy đầu, vai admin và member trong từng workspace, trang quản trị cho quản trị hệ thống.
- Nạp dữ liệu: CSV (kể cả windows-1252), Parquet, Excel nhiều sheet; mỗi lần nạp một phiên bản, nạp trùng bỏ qua; hồ sơ cột tự động (kiểu, vai trò, đơn vị, cột nhạy cảm) và câu hỏi gợi ý do mô hình đề xuất.
- Hỏi dữ liệu bằng tiếng Việt: mô hình ngôn ngữ (Gemini, Ollama, hoặc mock chạy offline) sinh SQL, guard chỉ cho truy vấn và tiêm phân quyền, tự sửa một vòng khi SQL lỗi, hỏi lại khi câu mơ hồ.
- Giải thích từng con số: thẻ "Đã hiểu là" với giả định đổi được, pane bằng chứng (câu SQL trước và sau guard, các bước backend), câu "tại sao" phân rã theo từng chiều, bấm ô số hoặc dòng phân rã để xem dòng nguồn, cảnh báo khi kết quả đáng ngờ.
- Biểu đồ chọn theo chuẩn IBCS; số và lời giải thích định dạng kiểu Việt Nam.
- Đặc tả OpenAPI sinh từ code (`/api/openapi.json`, Swagger UI ở `/api/docs`); quy trình phát hành gồm binary Linux, tarball mã nguồn và image trên GitHub Container Registry.
