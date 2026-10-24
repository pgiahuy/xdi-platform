# Guard SQL

Mô hình ngôn ngữ viết câu SQL từ câu hỏi; guard kiểm câu đó trước khi DuckDB chạy. Guard chỉ nhận một câu `SELECT`
chỉ đọc, trên đúng các bảng, hàm và cột được phép, thêm giới hạn số dòng, rồi đổi tên bảng logic sang file Parquet của
dataset.

Code: `backend/src/engine/guard/mod.rs`, hàm `build_guarded_sql`. Test: `backend/src/engine/guard/tests.rs`.

Trạng thái: guard đã có và có test, nhưng chưa endpoint nào gọi nó. Endpoint hỏi dữ liệu
([#33](https://github.com/pgiahuy/xdi-platform/issues/33)) sẽ gọi guard; tiêm điều kiện phân quyền theo dòng
([#31](https://github.com/pgiahuy/xdi-platform/issues/31)) chưa làm. Tài liệu này mô tả đúng hàm hiện có.

## Đầu vào và đầu ra

| | Nội dung |
|---|---|
| Vào | Câu SQL; danh sách bảng được phép (`AllowedTable`: tên bảng logic, tên các cột, thư mục Parquet của phiên bản); `max_rows` |
| Ra khi nhận | `GuardedSql` gồm hai bản của cùng một câu: `shown` và `runnable` (mục Bước 4) |
| Ra khi từ chối | `GuardError` kèm lý do tiếng Việt chỉ đúng chỗ sai: `UnknownName` khi bảng, bí danh hay cột không có (ví dụ "Không có bảng orders. Các bảng dùng được: sales, customers"), `Rejected` cho mọi lý do khác; `Internal` khi máy chủ lỗi lúc dựng bản chạy |

Guard không đọc Postgres hay DuckDB: service đưa danh sách bảng vào, nên test chạy không cần database. Lý do từ chối
viết để mô hình đọc và sửa ở vòng sau, nên nói rõ tên bảng, hàm hay cột bị từ chối. Trong luồng hỏi
(`service/ask_run.rs`), chỉ `UnknownName` được gửi lại cho mô hình sửa một lần: mô hình hay viết sai tên, và sửa được
khi biết tên đúng. Câu bị từ chối vì lý do khác (câu ghi, hàm không cho dùng, nhiều câu) dừng ngay: sửa tên không làm
nó qua được.

Guard kiểm trên cây cú pháp, không dò chữ: dò chữ `DROP` thì lọt khi viết hoa thường lẫn lộn hay chen chú thích. Cây
do crate `sqlparser` (0.63, cách đọc của DuckDB) dựng. Cả `shown` và `runnable` đều in lại từ cây đã kiểm, không dùng
câu gốc, nên phần bị bỏ qua khi đọc (chú thích) không tới được DuckDB.

## Bước 1: một câu SELECT chỉ đọc

- Câu dài hơn 32 KB (32 768 byte) bị từ chối trước khi đọc. Chuỗi `UNION` rất dài đọc được thành cây nhưng duyệt cây
  thì tràn stack và hủy cả tiến trình backend; câu do mô hình viết thường chỉ vài KB.
- Câu không đọc được, câu rỗng (kể cả chỉ có chú thích), hay nhiều câu (`SELECT 1; DROP TABLE x`) bị từ chối. Dấu `;`
  ở cuối một câu thì được.
- Câu ngoài cùng phải là truy vấn. Mọi câu lệnh khác bị từ chối: ghi (`INSERT`, `UPDATE`, `DELETE`, `DROP`, `CREATE`),
  gắn và chọn database (`ATTACH`, `DETACH`, `USE`), `COPY`, `PRAGMA`, `SET`, `RESET`, `CALL`, `INSTALL`, `LOAD`,
  `DESCRIBE`, `SHOW`, `SUMMARIZE`, `EXPLAIN`, `CREATE SECRET`, `EXPORT DATABASE`, `CHECKPOINT`.
- Kiểm cả bên trong câu, không chỉ câu ngoài cùng: câu lệnh lồng (`WITH t AS (DELETE ... RETURNING *)`) và
  `SELECT ... INTO` bị từ chối. Mỗi truy vấn con phải là `SELECT`, truy vấn trong ngoặc, hoặc `UNION`, `INTERSECT`,
  `EXCEPT` của chúng; `TABLE x` (đọc cả bảng mà không qua `FROM`) và `VALUES` bị từ chối.

## Bước 2: bảng, hàm, cột theo danh sách được phép

Guard dùng danh sách được phép, không dùng danh sách cấm: DuckDB có nhiều cách đọc file (`read_csv`, `glob`,
`FROM 'x.csv'`), đọc cấu hình, gọi mạng, và có thêm hàm mới qua từng phiên bản, nên danh sách cấm sẽ luôn sót.

So tên không phân biệt hoa thường, kể cả tên trong nháy kép, như DuckDB. Ngoài hoa thường, tên phải khớp đúng từng ký
tự: tên `sales` viết bằng chữ Kirin `а` (U+0430) thay cho `a` Latin bị từ chối dù trông giống hệt.

Bảng kiểm trước cột, để `SELECT region FROM orders` báo sai bảng `orders` chứ không báo thiếu cột `region`.

**Bảng.** Chỉ nhận bảng logic trong danh sách và bảng tạm đặt bằng `WITH`. Truy vấn con, `JOIN`, mọi vế của `UNION`
đều được kiểm. Bị từ chối:

- Tên kèm schema: `main.sales`, `information_schema.tables`, `pg_catalog.pg_tables`.
- Mọi hàm đặt ở chỗ bảng, kể cả hàm vô hại: `read_csv(...)`, `read_parquet(...)`, `glob(...)`, `range(10)`,
  `duckdb_tables()`. Chỉ guard được đổi bảng logic sang file; mô hình tự viết đường dẫn thì đọc được file của dataset
  hay workspace khác.
- Tên file viết như tên bảng: `FROM 'secret.csv'`, `FROM "secret.csv"`.
- `UNNEST`, `PIVOT` và các dạng lấy dữ liệu khác ngoài bảng, truy vấn con và `JOIN`.
- `WITH RECURSIVE`: có thể chạy không dừng, câu hỏi phân tích không cần.
- Bảng tạm trùng tên bảng dữ liệu (`WITH sales AS (...)`): nhận thì bước 4 đổi mọi `FROM sales` sang file Parquet,
  câu ra kết quả sai mà không báo lỗi.

Bí danh trùng tên bảng khác chỉ đổi tên gọi: `FROM customers AS sales` vẫn đọc dữ liệu của `customers`.

**Hàm.** Chỉ nhận hàm có tên trong `ALLOWED_FUNCTIONS`: tổng hợp (`sum`, `count`, `median`, `quantile_cont`...), cửa
sổ (`row_number`, `rank`, `lag`...), số, null và điều kiện (`coalesce`, `nullif`, `if`...), chuỗi, ngày giờ
(`date_trunc`, `year`, `strftime`...). Hàm ngoài danh sách bị từ chối ở mọi chỗ có biểu thức: danh sách cột, `WHERE`,
`GROUP BY`, `HAVING`, `ORDER BY`, `CASE`, `FILTER`, `OVER`, điều kiện `JOIN`. Ví dụ bị từ chối: `getenv`,
`current_setting`, `read_text`, `COLUMNS(...)`, tên hàm kèm schema (`main.sum`). Chỉ thêm hàm vào danh sách sau khi chắc
nó không đọc file, cấu hình hay mạng.

**Cột.** Tên cột `cot` phải là cột trong danh sách của một bảng câu đang dùng, hoặc tên chính câu đặt ra (`AS tong`,
`WITH t(a, b)`, `(SELECT ...) AS x(a, b)`). Dạng `bang.cot` thì `bang` phải là bảng, bảng tạm hay bí danh có trong câu.
Tên ba phần trở lên (`main.sales.region`) bị từ chối.

## Bước 3: giới hạn số dòng

`max_rows` do nơi gọi guard đặt; trần cụ thể sẽ do #33 chọn. Giới hạn đặt trên truy vấn ngoài cùng, nên áp cho cả
chuỗi `UNION`.

| Câu vào | Guard làm |
|---|---|
| Không có `LIMIT` | Thêm `LIMIT max_rows` |
| `LIMIT n` với n không quá `max_rows`, có hay không `OFFSET m` | Giữ nguyên |
| `LIMIT n` với n lớn hơn `max_rows` | Hạ còn `max_rows` |
| `LIMIT` là biểu thức hay truy vấn con (`LIMIT (SELECT 5000)`) | Thay bằng `max_rows`, vì không biết trước số dòng |
| `FETCH FIRST n ROWS ONLY` | Từ chối, lý do nhắc dùng `LIMIT` |
| Dạng `LIMIT` khác, như `LIMIT n BY cot` | Từ chối |

## Bước 4: hai bản shown và runnable

| Bản | Dùng để | Nội dung |
|---|---|---|
| `shown` | Hiện cho người dùng, lưu vào lịch sử câu hỏi (`queries.sql_guarded`) khi #33 nối guard vào luồng hỏi | Câu đã kiểm, còn tên bảng logic, có `LIMIT`; không chứa đường dẫn file trên máy chủ |
| `runnable` | Chỉ đưa cho DuckDB | Bảng logic thành `read_parquet(...)` đọc thư mục Parquet của phiên bản; bảng tạm đổi tên |

Ví dụ với `max_rows = 100`, bảng `sales` ở thư mục `/data/sales` (dòng `runnable` lấy từ test):

```
Câu vào:   SELECT sales.region FROM sales
shown:     SELECT sales.region FROM sales LIMIT 100
runnable:  SELECT sales.region FROM read_parquet('/data/sales/*.parquet', union_by_name = true) AS sales LIMIT 100
```

- Tên bảng cũ thành bí danh khi câu chưa đặt bí danh, để `sales.region` vẫn trỏ đúng bảng; câu đã có bí danh thì giữ.
- Phần `read_parquet(...)` lấy từ `analytics::parquet_glob`, cùng cách đọc lúc nạp dữ liệu ([Dữ liệu](DATA.md)).
- Bảng tạm đổi thành `guard_cte_1`, `guard_cte_2`... Bước 2 nhận tên bảng tạm ở mọi chỗ trong câu, không theo phạm vi;
  nếu giữ tên, một bảng tạm tên `"secret.csv"` gọi ở ngoài chỗ định nghĩa sẽ thành đọc file `secret.csv`. Đổi tên rồi
  thì chỗ gọi đó trỏ tới một bảng không tồn tại và DuckDB chỉ báo lỗi.

## Các ca đã có test

Test so nguyên chuỗi `shown` hoặc `runnable` ở phần lớn ca sửa câu, và kiểm lý do từ chối có nêu đúng tên ở các ca cần.

| Nhóm | Ca |
|---|---|
| Nhận | `SELECT` có `GROUP BY`, `ORDER BY`; `WITH`, `UNION ALL`, truy vấn con trong `FROM` và `WHERE`; `JOIN` có bí danh; tên viết hoa thường lẫn lộn và cột có khoảng trắng trong nháy kép (`"Order Item Total"`); `date_trunc` và bí danh cột; `WITH t(r, total)`; dấu `;` cuối câu; chuỗi `'; DROP TABLE sales; --'` là một giá trị so sánh |
| Đọc câu | Câu sai cú pháp; câu rỗng, chỉ khoảng trắng, chỉ chú thích; hai câu; chú thích giấu câu thứ hai (`--`, `/* */`) bị bỏ khỏi `shown`; `;` trước chú thích rồi câu thứ hai bị từ chối; `SEL/**/ECT` |
| Câu lệnh | Ghi, `ATTACH`, `COPY`, `PRAGMA`, `INSTALL`, `LOAD`, `DESCRIBE`, `SHOW`, `SUMMARIZE`, `EXPLAIN`, `SET`, `RESET`, `CALL`, `USE`, `DETACH`, `CREATE SECRET`, `EXPORT DATABASE`, `CHECKPOINT`; `DELETE` trong `WITH`; `SELECT INTO`; `TABLE x` kể cả trong `UNION` |
| Bảng | `read_csv`, `glob`, `read_parquet`, `parquet_scan`, `read_json_auto`, `generate_series`, `range` ở chỗ bảng, kể cả trong `JOIN`, truy vấn con và `WITH`; bảng hệ thống (`duckdb_tables`, `duckdb_settings`, `pg_catalog.pg_tables`, `sqlite_master`, `information_schema`); vế `UNION` đọc nguồn ngoài danh sách; tên file trong nháy đơn và nháy kép; bảng lạ (lý do liệt kê bảng dùng được); tên kèm schema; chữ Unicode giống chữ Latin; bí danh trùng tên bảng khác; `WITH RECURSIVE`; bảng tạm trùng tên bảng dữ liệu |
| Độ dài | Chuỗi 20 000 vế `UNION ALL` bị từ chối trước khi đọc; câu sát 32 KB vẫn qua mà không tràn stack 2 MB của luồng test |
| Hàm | `current_setting`, `getenv` (cả trong nháy kép), `read_text`, `main.sum`, `COLUMNS`; hàm lạ trong `ORDER BY`, `HAVING`, `CASE`, `FILTER`, `OVER`, `JOIN ... ON` |
| Cột | Cột không có (cả dạng `s.profit`); chữ Unicode giống chữ Latin; bí danh bảng không có; tên ba phần |
| Số dòng | Thêm `LIMIT`; hạ `LIMIT 5000`; giữ `LIMIT 10 OFFSET 5`; thay `LIMIT (SELECT 5000)`; giới hạn cả `UNION`; từ chối `FETCH FIRST` |
| Bản chạy | Bảng logic thành `read_parquet` và giữ tên làm bí danh; giữ bí danh có sẵn; đổi tên bảng tạm; bảng tạm tên giống file không còn trong bản chạy; `shown` không chứa đường dẫn; một câu có `WITH` và `JOIN` chạy trên DuckDB thật với file Parquet và ra đúng kết quả |

## Hạn chế đã biết

- Cây do `sqlparser` dựng, không phải DuckDB, nên với cú pháp hiếm hai bên có thể hiểu khác nhau.
- Cột kiểm trên cả câu, không theo phạm vi từng truy vấn con: cột của truy vấn con này dùng ở truy vấn con khác vẫn qua
  guard và chỉ bị DuckDB báo lỗi lúc chạy. Kiểm cột để mô hình sửa tên cột không có, không phải để chặn dữ liệu; chặn
  dữ liệu là việc của danh sách bảng và hàm.
- Trường của cột kiểu struct (`cot.truong`) bị từ chối, vì guard hiểu `cot` là tên bảng.
- Guard chỉ giới hạn số dòng trả về. Thời gian chạy (`QUERY_TIMEOUT_SECS`) và bộ nhớ (1 GB mỗi câu) giới hạn lúc
  chạy, ở `engine/query.rs`.
- Chưa tiêm điều kiện phân quyền theo dòng ([#31](https://github.com/pgiahuy/xdi-platform/issues/31)). Chỗ tiêm đã
  định: sau bước 2 (mọi bảng đã được kiểm), trước bước 4 (còn tên bảng logic); `shown` sẽ chứa cả điều kiện đó.
- Chưa có nơi gọi ([#33](https://github.com/pgiahuy/xdi-platform/issues/33)): danh sách cột của `AllowedTable` sẽ lấy
  từ hồ sơ cột, phần này chưa làm (xem [Dữ liệu](DATA.md), mục Chưa có). Trong luồng hỏi, câu bị guard từ chối (sau
  vòng sửa, nếu lý do là sai tên) sẽ trả 200 với `status: rejected` ([Kiến trúc](ARCHITECTURE.md), mục 8), không trả
  lỗi 400.
