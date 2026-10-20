# xdi-platform

*Natural language data analysis with evidence for every number. Upload a CSV, Parquet or Excel file, ask a question in Vietnamese, get a table, a chart, the SQL that produced it, and the rows behind each figure. Rust backend, DuckDB for analytics, Postgres for application data, Next.js UI. Apache 2.0. Vietnamese documentation follows.*

Nạp một file dữ liệu, hỏi bằng tiếng Việt, nhận bảng, biểu đồ và bằng chứng: câu SQL đã chạy, dòng dữ liệu đứng sau
mỗi con số, và với câu hỏi "tại sao", phần tính toán cho thấy nhóm nào kéo con số đó lên hay xuống.

Số không do mô hình ngôn ngữ bịa. Mô hình chỉ dịch câu hỏi thành SQL và diễn giải kết quả. Số do engine tính.

## Chạy thử trong ba lệnh

```bash
git clone https://github.com/pgiahuy/xdi-platform.git
cd xdi-platform
docker compose up
```

Mở http://localhost:3000, bấm một nút đăng nhập demo, chọn dataset mẫu, bấm một câu hỏi gợi ý. Không cần khóa API:
chế độ mặc định trả lời được các câu hỏi mẫu và báo rõ khi cần cấu hình mô hình ngôn ngữ cho câu hỏi tự do.

## Tài liệu

- [Cài đặt](INSTALL.md): Docker hoặc build từ nguồn, biến môi trường, lỗi hay gặp.
- [Kiến trúc](ARCHITECTURE.md): sơ đồ, luồng một câu hỏi, hai kho dữ liệu, hướng phát triển.
- [API](API.md): khái niệm, mã lỗi, Swagger UI.
- [Guard SQL](GUARD.md): hệ thống cho phép gì, cấm gì, tiêm phân quyền ở đâu.
- [Dữ liệu](DATA.md): định dạng nhận, cách suy vai trò cột, cách sửa bí danh.
- [Giấy phép thành phần](LICENSES.md).
- [Phát hành](RELEASING.md): quy trình ra phiên bản mới.

## Tham gia

- Lỗi: [Issues](https://github.com/pgiahuy/xdi-platform/issues).
- Câu hỏi và ý tưởng: [Discussions](https://github.com/pgiahuy/xdi-platform/discussions).
- Đóng góp mã: [CONTRIBUTING.md](https://github.com/pgiahuy/xdi-platform/blob/main/CONTRIBUTING.md).
