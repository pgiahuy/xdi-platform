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
| Ra khi từ chối | `GuardError` kèm lý do tiếng Việt chỉ đúng chỗ sai: `UnknownName` khi bảng, bí danh hay cột không có (ví dụ "Không có bảng orders. Các bảng dùng được: sales, customers"), `Oversized` khi câu có thể dựng giá trị quá lớn (luật kích thước, số tầng, mảnh ghép, độ rộng ở bước 2), `Rejected` cho mọi lý do khác; `Internal` khi máy chủ lỗi lúc dựng bản chạy |

Guard không đọc Postgres hay DuckDB: service đưa danh sách bảng vào, nên test chạy không cần database. Lý do từ chối
viết để mô hình đọc và sửa ở vòng sau, nên nói rõ tên bảng, hàm hay cột bị từ chối. Trong luồng hỏi
(`service/ask_run.rs`), `UnknownName` và `Oversized` được gửi lại cho mô hình sửa một lần: mô hình hay viết sai tên và
sửa được khi biết tên đúng, còn câu quá lớn thì viết lại gọn hơn được cho cùng câu hỏi. Câu bị từ chối vì lý do khác
(câu ghi, hàm không cho dùng, nhiều câu) dừng ngay: viết lại không làm nó qua được.

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
nó không đọc file, cấu hình hay mạng, và không dựng được giá trị khổng lồ từ tham số.

Vài hàm trong danh sách có luật riêng vì tham số quyết định kích thước kết quả (`engine/guard/sizes.rs`). DuckDB tính
biểu thức hằng ngay lúc chuẩn bị câu, khi giới hạn thời gian chưa có tác dụng và ngoài trần bộ nhớ; một lời gọi hàm
dựng chuỗi rất dài lúc chạy cũng không dừng được giữa chừng. Đo bằng `run_query` trên máy phát triển:
`SELECT length(lpad('x', 300000000, 'y'))` chạy 32 giây với giới hạn 1 giây; độ dài lấy từ cột chạy 87 giây và dùng
1,2 GB RAM với trần 1 GB. Độ dài so theo byte: thay `'a'` bằng `'😀'` giữ số ký tự nhưng gấp 4 số byte.

- `lpad`, `rpad`: độ dài phải là số viết thẳng, tối đa 250. Hai hàm đếm ký tự, mà một ký tự UTF-8 dài tới 4 byte
  (`lpad('😀', 1000, '😀')` ra 4000 byte), nên 250 ký tự giữ kết quả trong 1000 byte như chuỗi hằng.
- Gọi hàm dạng phương thức (`x.lpad(3, '0')`, `(a).concat(b)`, `main.sum(a)`) bị từ chối: biểu thức trước dấu chấm
  là tham số đầu mà các luật ở đây không thấy (`(a).concat(a)` nhân đôi chuỗi mà chỉ đếm một mảnh). Lý do gợi ý viết
  dạng hàm thường.
- `strftime`: dạng `strftime(ngày giờ, định dạng)`, định dạng viết thẳng, tối đa 100 byte, vì mỗi mã hai byte in ra
  nhiều byte (`%c` thành cả ngày giờ).
- `replace`: chuỗi tìm và chuỗi thay viết thẳng, chuỗi thay không dài hơn chuỗi tìm (bỏ ký tự, đổi dấu phân cách).
- `regexp_replace`: mẫu và chuỗi thay viết thẳng; chuỗi thay rỗng (xóa), hoặc một ký tự ASCII khi mẫu không khớp được
  chuỗi rỗng (`regexp_replace(region, '\s+', ' ', 'g')` gộp khoảng trắng). Mẫu khớp rỗng (`''`, `x*`) khớp ở mọi vị
  trí giữa hai ký tự, thay một ký tự cũng gần gấp đôi chuỗi mỗi lần lồng. Guard xét theo chữ: mẫu không có `*`, `?`,
  `{`, `|`, `\Q` và có ít nhất một phần chiếm ký tự.

**Số tầng, mảnh ghép, độ rộng.** Ngoài từng hàm, câu còn dựng được giá trị lớn bằng cách ghép giá trị của tầng dưới
nhiều lần qua các tầng câu con, hay chép một giá trị lớn ra nhiều cột, nhiều dòng. Các luật trên cả câu:

- Tối đa 4 tầng câu (`engine/guard/depth.rs`): câu con ở mọi chỗ (`FROM`, `WHERE`, danh sách cột, `ORDER BY`,
  `JOIN ... ON`) và bảng tạm `WITH` đều tính; bảng tạm đọc từ bảng tạm khác tính như câu con lồng trong nó. Bảng tạm
  cùng tên trong một `WITH` lồng chỉ che bảng tạm ngoài bên trong câu đó, không làm số tầng ít đi.
- Tối đa 20 mảnh ghép trong cả câu (`engine/guard/pieces.rs`): toán hạng của `||` (`a || b || c` là 3), tham số của
  `concat`, `concat_ws` (dấu ngăn là biểu thức thì tính cả các lần nó được chèn vào), phần tử của danh sách, struct,
  map viết thẳng. Dùng lại bí danh đặt trước đó trong cùng danh sách cột (`a0 || a0 AS a1, a1 || a1 AS a2`) cộng số
  mảnh của bí danh vào chỗ dùng. Tuple và mảng ở vế phải của `IN`, `= ANY(...)`, `= ALL(...)` chỉ để so sánh, không
  tính.
- Chuỗi hằng tối đa 1000 byte, ở mọi chỗ trong câu (`engine/guard/width.rs`).
- Mỗi danh sách cột của SELECT tối đa 200 mục (`engine/guard/width.rs`). `*` và `t.*` là một mục khi `FROM` có một
  nguồn. Khi có nhiều nguồn, `*` tính theo số cột thật trong hồ sơ của bảng dữ liệu, và bị từ chối nếu có bảng tạm hay
  câu con (không biết số cột): `SELECT * FROM w a1, w a2, ...` nhân số cột theo số lần JOIN.
- Mỗi `FROM` tối đa 4 nguồn, tính cả `JOIN`, bảng tạm, câu con (`engine/guard/width.rs`).
- Tối đa 10 nhánh `UNION`, `INTERSECT`, `EXCEPT` trong cả câu, cộng qua mọi tầng (`engine/guard/width.rs`): mỗi nhánh
  `UNION ALL` cho một dòng chép lại cùng giá trị, nên nhân như chép cột.

Trường hợp xấu nhất về độ dài một giá trị: 20 mảnh chia đều qua 4 tầng (ghép 5 chuỗi hằng ở tầng trong cùng, rồi 5 lần
mỗi tầng), tức 625 lần chuỗi hằng 1000 byte. Khi tầng ngoài dùng để chép ra 200 cột thì còn 3 tầng ghép (7, 7, 6), tức
294 lần mỗi cột. Trần 4 tầng giữ vì 6 tầng đã đo vượt mức khoảng 300 MB: chép 200 cột 486 MB, `UNION ALL` 74 nhánh
2,4 GB. Đo qua guard rồi chạy bằng `run_query` trên máy phát triển, giới hạn 1 giây, mỗi ca một tiến
trình có `ulimit -v` 4 GB, đọc VmHWM (RSS đỉnh):

| Ca (đều qua guard, 4 tầng) | Câu | RSS đỉnh | Kết quả |
|---|---|---|---|
| ghép 5 chuỗi hằng 1000 byte, rồi 5, 5, 5 lần, một cột | 5,2 KB | 67 MB | chuỗi 625 KB |
| 20 mảnh chia 7/7/6, một cột, chuỗi 1000 byte | 1,2 KB | 66 MB | chuỗi 294 KB |
| ghép 7 chuỗi hằng, rồi 7, 6 lần, chép ra 200 cột | 9,2 KB | 268 MB | 200 chuỗi 294 KB |
| 20 mảnh chia 10/10, chép ra 200 cột | 3,2 KB | 136 MB | 200 chuỗi 100 KB |
| như trên, 200 cột `lower(v)` | 4,6 KB | 134 MB | xong |
| 0 mảnh, chép 200 cột | 3,1 KB | 69 MB | xong |
| nguồn là `lpad` 250 ký tự 4 byte, 10/10 mảnh, 200 cột | 2,3 KB | 134 MB | xong |
| nguồn là `strftime` định dạng 100 byte, 10/10 mảnh, 200 cột | 2,4 KB | 134 MB | xong |
| `*` qua 4 nguồn là bảng dữ liệu, 20 dòng mỗi bảng | 48 byte | 73 MB | 100 000 dòng |
| `UNION ALL` 74 nhánh, mỗi nhánh 200 cột của giá trị 10/10 mảnh | 32,5 KB | | guard từ chối (74 nhánh) |
| `UNION ALL` 10 nhánh × 200 cột, giá trị 10/10 mảnh | 5,4 KB | 534 MB | 10 dòng |
| `UNION ALL` 10 nhánh × 200 cột, ghép 7 chuỗi hằng rồi ×7×6 | 11,4 KB | 1,37 GB | 10 dòng |
| chuỗi 1000 byte trên 50 000 dòng của bảng | 1 KB | 127 MB | xong |
| `string_agg` chuỗi 1000 byte trên 50 000 dòng | 1,1 KB | 227 MB | chuỗi 50 MB |
| chuỗi 1000 byte, 50 000 dòng, 200 cột | 1,4 KB | 1,7 GB | dừng vì quá giờ |
| 500 dòng, 100 cột, giá trị 2/2 mảnh | 1,3 KB | 1,03 GB | xong |
| 5000 dòng, 200 cột, giá trị 10/10 mảnh | 1,6 KB | 1,9 GB | OutOfMemory |

Các ca cho một dòng kết quả giờ dưới 270 MB. Trước các luật độ rộng, chép giá trị 16 KB ra 8000 cột, không ghép gì, đã
lên 690 MB; trước luật dạng phương thức, câu nhân đôi chuỗi bằng `(a).concat(a)` mỗi bước vẫn qua guard.
Các ca nhân theo số dòng vẫn vượt mức khoảng 300 MB. Không đọc bảng nào: `UNION ALL` 10 nhánh, mức luật cho phép, tới
1,37 GB (trước luật số nhánh, 74 nhánh đo được 1,94 GB). Đọc bảng: tới khoảng 1,9 GB. Mỗi dòng chép lại giá trị, phần
đổi kết quả sang Arrow và sang giá trị của backend nằm ngoài trần bộ nhớ của DuckDB, và đọc kết quả đã tính xong không
bị giới hạn thời gian. Phần này để tiến trình con, xem mục Hạn chế đã biết.

Thời điểm DuckDB kiểm cờ dừng thay đổi giữa các lần chạy, nên số của các ca bị dừng vì quá giờ dao động: ca
`UNION ALL` 74 nhánh từng đo ra 124 MB (dừng sớm) rồi 1,94 GB (chạy xong sau 1,76 giây).

Từ chối theo các luật kích thước, số tầng, mảnh ghép, độ rộng (`GuardError::Oversized`) được gửi lại cho mô hình viết
lại gọn hơn, như sai tên bảng, cột.

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
| Hàm | `current_setting`, `getenv` (cả trong nháy kép), `read_text`, `main.sum`, `COLUMNS`; hàm lạ trong `ORDER BY`, `HAVING`, `CASE`, `FILTER`, `OVER`, `JOIN ... ON`; `lpad`, `rpad` với độ dài hằng lớn, biểu thức, tên cột, câu con; `replace` làm chuỗi dài ra, tính theo byte (cả lồng nhau); `regexp_replace` với mẫu khớp được chuỗi rỗng hay chuỗi thay dài hơn một byte; `strftime` với định dạng từ cột, đảo thứ tự tham số hay dài hơn 100 byte; các cách dùng để làm sạch chuỗi (cả gộp khoảng trắng) vẫn qua |
| Số tầng, mảnh, độ rộng | 4 tầng nhận, 5 tầng từ chối ở mọi chỗ (`FROM`, `WHERE`, cột, `WITH`, bảng tạm đọc bảng tạm); bảng tạm cùng tên trong `WITH` lồng không che số tầng; bảng tạm kèm câu con kiểu mô hình hay viết vẫn qua; 20 mảnh nhận, 21 từ chối (`||`, `concat`, `concat_ws`, cộng qua các tầng, danh sách và struct viết thẳng, bí danh dùng lại); tuple, mảng trong `IN`, `ANY`, `ALL` không tính; gọi hàm dạng phương thức bị từ chối; chuỗi hằng 1000 byte nhận, 1001 từ chối (tính theo byte, ở `IN`, đối số hàm); 200 cột nhận, 201 từ chối; `*` trên nhiều nguồn tính theo số cột thật, từ chối khi có bảng tạm; 4 nguồn trong `FROM` nhận, 5 từ chối; 10 nhánh `UNION`, `INTERSECT`, `EXCEPT` nhận, 11 từ chối, cộng qua các tầng; câu mẫu của mock và của `ask_examples.rs` vẫn qua |
| Câu mẫu (`sample_tests.rs`) | 31 câu phân tích dạng mô hình hay viết, 25 qua; 6 bị từ chối có chủ ý: 3 câu 5 tầng (vào vòng sửa), `strftime` đảo thứ tự tham số, hàm `list_contains` ngoài danh sách, nhãn 1200 byte. 17 câu tìm cách lách: các câu nhân giá trị bị chặn, các câu vô hại qua |
| Cột | Cột không có (cả dạng `s.profit`); chữ Unicode giống chữ Latin; bí danh bảng không có; tên ba phần |
| Số dòng | Thêm `LIMIT` (cả khi chỉ có `OFFSET`); hạ `LIMIT 5000`; giữ `LIMIT 10 OFFSET 5`; thay `LIMIT (SELECT 5000)`; giới hạn cả `UNION`; từ chối `FETCH FIRST`; ghi chú `limit_note` ("Thêm LIMIT 100", "Hạ LIMIT xuống 100") |
| Bản chạy | Bảng logic thành `read_parquet` và giữ tên làm bí danh; giữ bí danh có sẵn; đổi tên bảng tạm; bảng tạm tên giống file không còn trong bản chạy; `shown` không chứa đường dẫn; một câu có `WITH` và `JOIN` chạy trên DuckDB thật với file Parquet và ra đúng kết quả |

## Hạn chế đã biết

- Cây do `sqlparser` dựng, không phải DuckDB, nên với cú pháp hiếm hai bên có thể hiểu khác nhau.
- Cột kiểm trên cả câu, không theo phạm vi từng truy vấn con: cột của truy vấn con này dùng ở truy vấn con khác vẫn qua
  guard và chỉ bị DuckDB báo lỗi lúc chạy. Kiểm cột để mô hình sửa tên cột không có, không phải để chặn dữ liệu; chặn
  dữ liệu là việc của danh sách bảng và hàm.
- Trường của cột kiểu struct (`cot.truong`) bị từ chối, vì guard hiểu `cot` là tên bảng.
- Guard chỉ giới hạn số dòng trả về. Thời gian chạy (`QUERY_TIMEOUT_SECS`) và bộ nhớ (1 GB mỗi câu) giới hạn lúc
  chạy, ở `engine/query.rs`: câu dừng ở lần DuckDB kiểm cờ dừng, chuỗi tạo lúc chạy nằm trong trần (vượt thì báo
  OutOfMemory). Phần không bị giới hạn là biểu thức hằng DuckDB tính lúc chuẩn bị câu, và một lời gọi hàm dựng chuỗi
  rất dài thì không dừng được giữa chừng. Bước 2 thu hẹp phần này bằng luật kích thước hàm, số tầng, mảnh ghép, độ
  rộng: các ca cho một dòng kết quả đã đo qua guard đều dưới 270 MB (bảng ở bước 2). Còn lại: câu nhân theo số dòng
  (dòng của bảng, hay `UNION ALL` tới 10 nhánh của cùng một giá trị) lên tới khoảng 1,4 đến 1,9 GB, vì phần đổi kết quả
  sang Arrow và sang giá trị của backend nằm ngoài trần của DuckDB, và đọc kết quả đã tính xong không bị giới hạn thời
  gian. Đó là số đo trên các dạng câu đã thử, không phải bảo đảm;
  bảo đảm thật cần chạy câu trong tiến trình con có trần RAM và thời gian của hệ điều hành, như bộ đọc Excel
  (`engine/excel_worker.rs`). Refs [#119](https://github.com/pgiahuy/xdi-platform/issues/119).
- Chưa tiêm điều kiện phân quyền theo dòng ([#31](https://github.com/pgiahuy/xdi-platform/issues/31)). Chỗ tiêm đã
  định: sau bước 2 (mọi bảng đã được kiểm), trước bước 4 (còn tên bảng logic); `shown` sẽ chứa cả điều kiện đó.
- Chưa có nơi gọi ([#33](https://github.com/pgiahuy/xdi-platform/issues/33)): danh sách cột của `AllowedTable` sẽ lấy
  từ hồ sơ cột, phần này chưa làm (xem [Dữ liệu](DATA.md), mục Chưa có). Trong luồng hỏi, câu bị guard từ chối (sau
  vòng sửa, nếu lý do là sai tên) sẽ trả 200 với `status: rejected` ([Kiến trúc](ARCHITECTURE.md), mục 8), không trả
  lỗi 400.
