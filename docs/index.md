# xdi-platform

*Natural language data analysis, in early development. The goal: upload a CSV, Parquet or Excel file, ask a question in Vietnamese, and get a table, a chart, the SQL that produced it, and the rows behind each figure. Rust backend, DuckDB for analytics, Postgres for application data, React UI. Apache 2.0. Vietnamese documentation follows.*

Mục tiêu: nạp một file dữ liệu, hỏi bằng tiếng Việt, nhận bảng, biểu đồ, câu SQL đã chạy và các dòng dữ liệu đứng sau
mỗi con số; với câu hỏi "tại sao", hệ thống tính nhóm nào làm con số tăng hay giảm. Con số do engine phân tích tính trên
dữ liệu; mô hình ngôn ngữ chỉ chuyển câu hỏi thành SQL và diễn giải kết quả.

## Chạy

```bash
git clone https://github.com/pgiahuy/xdi-platform.git
cd xdi-platform
docker compose up
```

Mở http://localhost:8000; lần đầu, nhập mã thiết lập in trong log của backend để tạo tài khoản quản trị.
`curl localhost:8000/api/health` trả `{"status":"ok"}`. Dataset mẫu, câu hỏi mẫu và các trang nghiệp vụ đang được làm trong [milestone v0.1.0](https://github.com/pgiahuy/xdi-platform/milestone/1).

## Tài liệu

- [Kiến trúc](ARCHITECTURE.md): sơ đồ, luồng một câu hỏi, hai kho dữ liệu, hướng phát triển.
- [Cài đặt](INSTALL.md): Docker, build từ nguồn, đưa lên Internet với HTTPS, sao lưu, lỗi hay gặp.
- [Phát hành](RELEASING.md): quy trình ra phiên bản mới.
- [Giấy phép](LICENSES.md): giấy phép của các thành phần bên thứ ba.
- [Dữ liệu và quyền riêng tư](PRIVACY.md): dữ liệu nằm ở đâu, gửi gì cho mô hình ngôn ngữ, cách xóa.
- [API](API.md): đặc tả OpenAPI, đăng nhập bằng cookie, workspace và vai, mã lỗi.
- [Guard SQL](GUARD.md): câu SQL nào được chạy, câu nào bị từ chối, các ca đã có test.
- [Dữ liệu](DATA.md): định dạng file nhận, giới hạn, phiên bản, tên bảng và tên cột.

## Tham gia

- Lỗi: [Issues](https://github.com/pgiahuy/xdi-platform/issues).
- Câu hỏi và ý tưởng: [Discussions](https://github.com/pgiahuy/xdi-platform/discussions).
- Đóng góp mã: [CONTRIBUTING.md](https://github.com/pgiahuy/xdi-platform/blob/main/CONTRIBUTING.md).
