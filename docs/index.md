# xdi-platform

*Ask questions about your data in Vietnamese and get answers with evidence for every number. Upload a CSV, Parquet or
Excel file; the system writes SQL, checks it with a SQL guard, runs it on DuckDB and returns a table, a chart and the
SQL it ran. For "why" questions it computes which groups drove the change and shows the source rows behind the
numbers. Rust backend, DuckDB for analytics, Postgres for application data, React UI. Apache 2.0. Latest release:
v0.2.0. Vietnamese documentation follows.*

Nền tảng phân tích dữ liệu bằng ngôn ngữ tự nhiên: hỏi bằng tiếng Việt, nhận câu trả lời kèm lời giải thích và minh
chứng cho từng con số. Nạp file, gõ câu hỏi, nhận bảng, biểu đồ và câu SQL đã chạy. Câu "tại sao" được tách thành phần chênh lệch của từng nhóm; bấm vào một số để xem các dòng dữ liệu
đứng sau nó.

Con số do DuckDB tính trên máy của bạn. Mô hình ngôn ngữ viết mô tả cột lúc nạp, viết SQL, điền form câu "tại sao" và
viết lời; nó nhận tên cột và danh sách giá trị của cột phân loại, không nhận bảng dữ liệu. Riêng đoạn trả lời ngắn của
câu hỏi thường gửi tối đa 10 dòng đầu của kết quả (với câu không gộp, đó là dòng gốc), tắt bằng `ANSWER_TEXT=false`
([Dữ liệu và quyền riêng tư](PRIVACY.md)).

![Một câu hỏi đi qua xdi-platform: giao diện, API, guard SQL, DuckDB, mô hình ngôn ngữ, Postgres, theo thứ tự 12 bước](images/luong-cau-hoi.png)

## Chạy thử

```bash
git clone https://github.com/pgiahuy/xdi-platform.git
cd xdi-platform
cp .env.example .env
docker compose up -d --build
docker compose logs backend | grep setup_code
```

Mở http://localhost:8000, nhập mã thiết lập để tạo tài khoản quản trị. Workspace đầu tiên có sẵn dataset mẫu DataCo;
mô hình mặc định `mock` trả lời các câu mẫu trên dataset này. Hướng dẫn từng bước, kể cả cách bật Gemini hay Ollama:
[Cài đặt](INSTALL.md).

## Trạng thái

Đã có: tài khoản, workspace, vai `admin` và `member`; nạp CSV, Parquet, Excel thành phiên bản; hồ sơ cột và câu gợi ý,
gồm câu "vì sao"; hỏi theo cuộc trò chuyện, SQL qua guard, bảng, biểu đồ, thẻ "Đã hiểu là"; câu "tại sao" có phân rã
và lời giải thích bằng lời thường; minh chứng gồm SQL, phân rã, các bước và dòng nguồn của các ô số đo; mô hình `mock`,
Gemini, Ollama.

Chưa có: gán vai dữ liệu cho thành viên, chế độ demo, màn xem lịch sử câu hỏi, màn sửa hồ sơ cột và chỉ số. Chi tiết
ở bảng Trạng thái của [Kiến trúc](ARCHITECTURE.md#trạng-thái).

## Tài liệu

| Trang | Nội dung |
|---|---|
| [Cài đặt](INSTALL.md) | Docker, build từ nguồn, đưa lên Internet, sao lưu, lỗi hay gặp, dùng lần đầu |
| [Kiến trúc](ARCHITECTURE.md) | Sơ đồ, luồng một câu hỏi, hai kho dữ liệu, trạng thái từng phần |
| [API](API.md) | Đặc tả OpenAPI, đăng nhập bằng cookie, workspace và vai, mã lỗi |
| [Guard SQL](GUARD.md) | Câu SQL nào được chạy, câu nào bị từ chối |
| [Dữ liệu](DATA.md) | Định dạng file nhận, giới hạn, phiên bản, tên bảng và tên cột |
| [Dữ liệu và quyền riêng tư](PRIVACY.md) | Dữ liệu nằm ở đâu, gửi gì cho mô hình, cách xóa |
| [Gói khái niệm ngành](DOMAINS.md) | Khái niệm chung của một ngành, cách thêm một ngành |
| [Phát hành](RELEASING.md) | Quy trình ra phiên bản mới |
| [Giấy phép](LICENSES.md) | Giấy phép của các thành phần bên thứ ba |

## Tham gia

- Lỗi: [Issues](https://github.com/pgiahuy/xdi-platform/issues). Câu hỏi và ý tưởng:
  [Discussions](https://github.com/pgiahuy/xdi-platform/discussions).
- Đóng góp mã: [CONTRIBUTING.md](https://github.com/pgiahuy/xdi-platform/blob/main/CONTRIBUTING.md).
- Lỗ hổng bảo mật: báo riêng theo [SECURITY.md](https://github.com/pgiahuy/xdi-platform/blob/main/SECURITY.md), không
  mở issue công khai.
