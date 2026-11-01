# Bộ đánh giá

Các bộ câu đo chất lượng câu trả lời, phân loại câu hỏi và mô hình chấm. Lệnh `make` chạy với `mock` nếu không đặt
`LLM_PROVIDER`; `mock` chỉ trả lời được các câu mẫu, nên số đo thật cần `LLM_PROVIDER=gemini` (hay `ollama`). Lệnh
đánh giá dùng `EVAL_GEMINI_API_KEY` nếu có, không thì `GEMINI_API_KEY` ([INSTALL.md](../docs/INSTALL.md)).

## File

| File | Số ca | Dùng cho |
|---|---|---|
| `dataco.json` | 14 | `make eval` trên `data/samples/dataco.csv`; CI chạy bộ này |
| `sales_joins.json` | 16 | `make eval` trên `sales_demo` khi database mẫu đang chạy, phần lớn câu nối bảng |
| `edu.json` | 13 | `make eval` trên `edu_demo` khi database mẫu đang chạy |
| `oulad_db.json` | 57 | Chạy tay trên `edu_demo`: 11 loại câu, gồm câu hỏi nguyên nhân, câu không dấu, câu phải từ chối, câu tiếp nối |
| `classify.json` | 50 | `make eval-classify`: 25 câu hỏi số liệu, 25 câu hỏi nguyên nhân |
| `judge.json` | 80 | `make eval-judge`: lời giải thích có nhãn, 40 lời đúng, 40 lời sai |
| `lessons.json` | 2 bài học | `make eval-lessons`: chạy `dataco.json` hai lượt, không và có bài học |
| `baseline.json` | | Tỷ lệ đúng tối thiểu của `make eval` (mock phải đúng 100%) |

Một ca của bộ câu hỏi có `id`, `question`, `expected_sql` (SQL đáp án viết tay) và `dataset` (bảng bắt đầu, bỏ trống
khi dữ liệu là một file). Trường tùy chọn: `kind` (loại câu, `why_*` phải đi đường câu hỏi nguyên nhân), `variants`
(cách hỏi khác), `required_columns`, `ordered`, `must_refuse`, `follow_up_of` (id câu gốc trong cùng cuộc trò chuyện).

## Chấm

Mỗi câu ra một trong ba kết quả (`backend/src/service/eval_grade.rs`, `eval.rs`):
- `pass`: bảng kết quả khớp bảng của SQL đáp án. Không xét tên và vị trí cột, cột thừa vẫn đạt; không xét thứ tự dòng
  trừ khi `ordered`; số không nguyên so sau khi làm tròn 4 chữ số có nghĩa. Câu `must_refuse` đạt khi bị từ chối.
- `fail`: lệch kết quả, sai đường trả lời, hay trả lời câu phải từ chối.
- `review`: chưa chấm được (mô hình hết hạn mức hay không trả lời, đáp án không chạy, hệ thống hỏi lại, câu gốc của câu
  tiếp nối chưa có lời). Không tính vào tỷ lệ đúng.

Tỷ lệ đúng là `pass / (pass + fail)`. Báo cáo in thêm bảng theo `kind` kèm khoảng tin cậy Wilson 95% và ghi JSON ra
`--out` (mặc định `eval/result.json`). `make eval` thoát mã 1 khi tỷ lệ đúng dưới `baseline.json`, mã 2 khi lỗi hạ tầng.

## Lệnh

```bash
make eval                               # mock; thêm sales_demo, edu_demo nếu container xdi-demo-db đang chạy
make eval-classify LLM_PROVIDER=gemini  # tỷ lệ đúng của luật từ khóa và của mô hình phân loại
make eval-judge LLM_PROVIDER=gemini     # mô hình chấm: đúng trên tổng, bỏ lọt, chặn nhầm; không cần database
make eval-lessons LLM_PROVIDER=gemini   # tỷ lệ đúng không và có bài học
```

Bộ `oulad_db.json` chưa có target `make`. Sau khi nạp database mẫu ([data/postgres-demo](../data/postgres-demo/README.md))
và chạy `make eval` một lần (tạo database `xdi_eval` trong Postgres của compose, cổng 5433), từ gốc repo:

```bash
make duckdb-extension
cd backend && cargo build -q && cd ..
DATABASE_URL=postgres://xdi:xdi@localhost:5433/xdi_eval DATA_DIR=backend/target/eval-data \
  SAMPLE_DATABASE_URL=postgres://analyst:analyst@127.0.0.1:5440/edu_demo \
  SOURCE_POSTGRES_EXTENSION=$PWD/duckdb-extensions/postgres_scanner.duckdb_extension \
  LLM_PROVIDER=gemini backend/target/debug/xdi-backend eval --sample-database --cases eval/oulad_db.json \
  --subset 30 --repeat 3 --pause-ms 4500
```

Cờ của `xdi-backend eval`:
- `--subset N`: chọn N câu xoay vòng theo loại; câu tiếp nối kéo theo câu gốc.
- `--variants`: mỗi cách hỏi khác thành một câu chấm riêng.
- `--repeat N`: chạy N lượt, chụp và lập hồ sơ một lần; in các câu đổi kết quả giữa các lượt.
- `--previous <json>`: so với báo cáo cũ, liệt kê câu đạt lần trước mà trượt lần này.
- `--pause-ms`, `--quota-wait-ms`: giãn các lần gọi mô hình, chờ khi hết hạn mức theo phút (khóa miễn phí cần
  `--pause-ms`).
- `--lessons <json>`: chạy hai lượt, không và có bài học; không dùng chung với `--repeat`.

Trong repo chưa có lượt chạy `oulad_db.json` nào với Gemini được ghi lại.
