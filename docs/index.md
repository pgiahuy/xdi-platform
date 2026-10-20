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

Hiện mới có khung backend: `curl localhost:8000/health` trả `{"status":"ok"}`. Giao diện, dataset mẫu và câu hỏi mẫu
đang được làm trong [milestone v0.1.0](https://github.com/pgiahuy/xdi-platform/milestone/1).

## Tài liệu

- [Kiến trúc](ARCHITECTURE.md): sơ đồ, luồng một câu hỏi, hai kho dữ liệu, hướng phát triển.
- [Phát hành](RELEASING.md): quy trình ra phiên bản mới.

Đang viết: cài đặt ([#54](https://github.com/pgiahuy/xdi-platform/issues/54)), API, guard SQL và dữ liệu
([#56](https://github.com/pgiahuy/xdi-platform/issues/56)), giấy phép thành phần và quyền riêng tư
([#57](https://github.com/pgiahuy/xdi-platform/issues/57)).

## Tham gia

- Lỗi: [Issues](https://github.com/pgiahuy/xdi-platform/issues).
- Câu hỏi và ý tưởng: [Discussions](https://github.com/pgiahuy/xdi-platform/discussions).
- Đóng góp mã: [CONTRIBUTING.md](https://github.com/pgiahuy/xdi-platform/blob/main/CONTRIBUTING.md).
