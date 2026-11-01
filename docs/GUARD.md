# Guard SQL

Mô hình ngôn ngữ viết câu SQL từ câu hỏi; guard kiểm câu đó trước khi DuckDB chạy. Guard chỉ nhận một câu `SELECT`
chỉ đọc, trên đúng các bảng, hàm và cột được phép, nối bảng đúng khóa ngoại, lọc dòng theo vai dữ liệu của người hỏi,
thêm giới hạn số dòng, rồi đổi tên bảng logic sang file Parquet của phiên bản đang dùng.

Mọi câu SQL chạy trên dữ liệu đều qua guard: câu hỏi số liệu (`service/ask/run.rs`), các câu của phân rã
(`service/explain/breakdown.rs`), truy dòng nguồn (`service/drill/`), tổng quan dataset (`service/overview/`), chạy thử
chỉ số (`service/metrics/check.rs`), báo cáo (`service/report/`) và bộ đánh giá (`service/eval.rs`). Nơi gọi truyền các
bảng được đọc, quan hệ khóa ngoại giữa chúng (rỗng với dataset file) và điều kiện lọc dòng theo vai dữ liệu của người
hỏi, người bấm hay người xem (`load_row_filters`, `service/ask/row_filters.rs`; admin không bị lọc).

Code: `backend/src/engine/guard/mod.rs`, hàm `build_guarded_sql`. Test nằm cạnh code, mỗi file ghi danh mục ca ở khối
chú thích đầu file: `tests.rs` (các bước, có hai câu chạy trên DuckDB thật), `sample_tests.rs` (32 câu dạng mô hình hay
viết, 18 câu tìm cách lách luật kích thước), `row_filter_tests.rs` và `parent_filter_tests.rs` (bước 3, so kết quả trên
DuckDB thật với bảng chỉ còn dòng được phép), `join_tests.rs` (nối bảng).

## Đầu vào và đầu ra

| | Nội dung |
|---|---|
| Vào | Câu SQL; bảng được phép (`AllowedTable`: tên bảng logic, các cột, thư mục Parquet của phiên bản, `row_filters`); quan hệ khóa ngoại (`JoinLink`, từ `dataset_relations`); `max_rows` |
| Ra khi nhận | `GuardedSql`: hai bản `shown` và `runnable` (bước 5), ghi chú `limit_note`, các điều kiện lọc dòng đã thêm (`row_filters`), các bảng câu đã đọc |
| Ra khi từ chối | `GuardError`: `UnknownName` (bảng, bí danh, cột không có), `Oversized` (luật kích thước ở bước 2), `Rejected` (lý do khác), `Internal` (lỗi máy chủ, chính sách lọc dòng sai) |

Guard không đọc Postgres hay DuckDB, nên test chạy không cần database. Lý do từ chối viết bằng tiếng Việt, nêu đúng tên
bảng, hàm hay cột bị từ chối, để mô hình đọc và sửa. Trong luồng hỏi, mọi lý do trừ `Internal` được gửi lại cho mô hình
sửa một lần; câu sửa qua guard lại từ đầu, nên gửi lý do không mở thêm đường nào tới dữ liệu. Câu sửa vẫn bị từ chối thì
câu hỏi trả 200 với `status: rejected` kèm lời của hệ thống; lý do của guard nằm ở bước `guard` trong khung Minh chứng.

## Luật viết vào prompt

Prompt SQL có một mục luật sinh từ chính guard (`engine/guard/prompt_rules.rs`): danh sách hàm `ALLOWED_FUNCTIONS` và
các trần của bước 1, 2 lấy từ đúng hằng số guard dùng để kiểm, nên đổi luật thì prompt đổi theo. Prompt chỉ hướng mô
hình; guard vẫn kiểm lại mọi câu.

| Nhóm | Gồm | Có trong prompt |
|---|---|---|
| Luật mô hình tự đáp ứng được | Một câu `SELECT` tối đa 32 KB; chỉ bảng trong danh sách, không đọc file; chỉ cột có trong hồ sơ; danh sách hàm và luật riêng của vài hàm; các trần số tầng, mảnh ghép, chuỗi hằng, cột, nguồn, nhánh; chỉ viết `LIMIT` khi câu hỏi nêu số dòng | Có |
| Phần guard tự thêm | Điều kiện lọc dòng (bước 3), trần số dòng (bước 4), đường dẫn Parquet và tên bảng tạm của bản chạy (bước 5) | Không. Lọc dòng lộ phạm vi dữ liệu của người hỏi; đường dẫn lộ cách lưu trên máy chủ |

Guard kiểm trên cây cú pháp do crate `sqlparser` (0.63, cách đọc của DuckDB) dựng, không dò chữ. Cả `shown` và
`runnable` in lại từ cây đã kiểm, nên phần bị bỏ qua khi đọc (chú thích) không tới được DuckDB.

## Bước 1: một câu SELECT chỉ đọc

- Câu dài hơn 32 KB (32 768 byte) bị từ chối trước khi đọc: chuỗi `UNION` rất dài đọc được thành cây nhưng duyệt cây
  thì tràn stack và hủy cả tiến trình backend.
- Câu không đọc được, câu rỗng (kể cả chỉ có chú thích), hay nhiều câu (`SELECT 1; DROP TABLE x`) bị từ chối. Dấu `;`
  ở cuối một câu thì được.
- Câu ngoài cùng phải là truy vấn. Bị từ chối: ghi (`INSERT`, `UPDATE`, `DELETE`, `DROP`, `CREATE`), `ATTACH`,
  `DETACH`, `USE`, `COPY`, `PRAGMA`, `SET`, `RESET`, `CALL`, `INSTALL`, `LOAD`, `DESCRIBE`, `SHOW`, `SUMMARIZE`,
  `EXPLAIN`, `CREATE SECRET`, `EXPORT DATABASE`, `CHECKPOINT`.
- Kiểm cả bên trong: câu lệnh lồng (`WITH t AS (DELETE ... RETURNING *)`), `SELECT ... INTO`, `TABLE x`, `VALUES`,
  `FOR UPDATE`, `FOR SHARE` bị từ chối ở mọi câu, kể cả câu con. Mỗi truy vấn con phải là `SELECT`, truy vấn trong
  ngoặc, hay `UNION`, `INTERSECT`, `EXCEPT` của chúng.

## Bước 2: bảng, hàm, cột theo danh sách được phép

Guard dùng danh sách được phép, không dùng danh sách cấm: DuckDB có nhiều cách đọc file (`read_csv`, `glob`,
`FROM 'x.csv'`), đọc cấu hình, gọi mạng, và có thêm hàm mới qua từng phiên bản.

So tên không phân biệt hoa thường, kể cả tên trong nháy kép, như DuckDB; ngoài hoa thường, tên phải khớp đúng từng ký
tự (`sales` viết bằng chữ Kirin `а` bị từ chối). Bảng kiểm trước cột, để `SELECT region FROM orders` báo sai bảng.

**Bảng.** Chỉ nhận bảng logic trong danh sách và bảng tạm đặt bằng `WITH`; truy vấn con, `JOIN`, mọi vế `UNION` đều được
kiểm. Bị từ chối:

- Tên kèm schema: `main.sales`, `information_schema.tables`, `pg_catalog.pg_tables`.
- Mọi hàm ở chỗ bảng, kể cả hàm vô hại: `read_csv(...)`, `read_parquet(...)`, `glob(...)`, `range(10)`,
  `duckdb_tables()`. Chỉ guard được đổi bảng logic sang file; mô hình tự viết đường dẫn thì đọc được file của dataset
  hay workspace khác.
- Tên file viết như tên bảng: `FROM 'secret.csv'`, `FROM "secret.csv"`.
- `UNNEST`, `PIVOT` và các dạng lấy dữ liệu khác ngoài bảng, truy vấn con và `JOIN`.
- `WITH RECURSIVE`: có thể chạy không dừng, câu hỏi phân tích không cần.
- Bảng tạm trùng tên bảng dữ liệu (`WITH sales AS (...)`): nhận thì bước 5 đổi mọi `FROM sales` sang file Parquet và câu
  ra kết quả sai mà không báo lỗi.

Bí danh trùng tên bảng khác chỉ đổi tên gọi: `FROM customers AS sales` vẫn đọc `customers`.

**Hàm.** Chỉ nhận hàm có tên trong `ALLOWED_FUNCTIONS`: tổng hợp (`sum`, `count`, `median`, `quantile_cont`...), cửa
sổ (`row_number`, `rank`, `lag`...), số, null và điều kiện (`coalesce`, `nullif`, `if`...), chuỗi, ngày giờ
(`date_trunc`, `year`, `strftime`...). Hàm ngoài danh sách bị từ chối ở mọi chỗ có biểu thức, kể cả `ORDER BY`,
`HAVING`, `CASE`, `FILTER`, `OVER`, điều kiện `JOIN`; ví dụ `getenv`, `current_setting`, `read_text`, `COLUMNS(...)`.
Chỉ thêm hàm vào danh sách khi chắc nó không đọc file, cấu hình hay mạng, và không dựng được giá trị khổng lồ từ tham số.

Vài hàm có luật riêng vì tham số quyết định kích thước kết quả (`engine/guard/sizes.rs`). DuckDB tính biểu thức hằng
ngay lúc chuẩn bị câu, khi giới hạn thời gian chưa có tác dụng và ngoài trần bộ nhớ: `SELECT length(lpad('x',
300000000, 'y'))` chạy 32 giây với giới hạn 1 giây (đo bằng `run_query` trên máy phát triển, 25/10/2026). Độ dài so
theo byte: một ký tự UTF-8 dài tới 4 byte.

- `lpad`, `rpad`: độ dài là số viết thẳng, tối đa 250 ký tự, tức tối đa 1000 byte như chuỗi hằng.
- Gọi hàm dạng phương thức (`x.lpad(3, '0')`, `(a).concat(b)`, `main.sum(a)`) bị từ chối: biểu thức trước dấu chấm là
  tham số đầu mà các luật ở đây không thấy. Lý do gợi ý viết dạng hàm thường.
- `strftime(ngày giờ, định dạng)`: định dạng viết thẳng, tối đa 100 byte, vì mỗi mã hai byte in ra nhiều byte.
- `replace`: chuỗi tìm và chuỗi thay viết thẳng, chuỗi thay không dài hơn chuỗi tìm.
- `regexp_replace`: mẫu và chuỗi thay viết thẳng; chuỗi thay rỗng, hay một ký tự ASCII khi mẫu không khớp được chuỗi
  rỗng (`regexp_replace(region, '\s+', ' ', 'g')`). Guard xét theo chữ: mẫu không có `*`, `?`, `{`, `|`, `\Q` và có ít
  nhất một phần chiếm ký tự.

**Số tầng, mảnh ghép, độ rộng.** Câu còn dựng được giá trị lớn bằng cách ghép giá trị của tầng dưới nhiều lần qua các
tầng câu con, hay chép một giá trị lớn ra nhiều cột, nhiều dòng. Luật trên cả câu:

- Tối đa 4 tầng câu (`depth.rs`): câu con ở mọi chỗ và bảng tạm `WITH` đều tính; bảng tạm đọc bảng tạm khác tính như
  câu con lồng trong nó; bảng tạm không ai đọc vẫn tính. Hộp `(SELECT * FROM bảng WHERE ...)` không tính tầng của chính
  nó (câu con trong `WHERE` của hộp vẫn tính), vì hộp chỉ lọc dòng; nhờ vậy câu đã lưu có hộp lọc qua lại guard được khi
  truy dòng nguồn.
- Tối đa 20 mảnh ghép trong cả câu (`pieces.rs`): toán hạng của `||`, tham số của `concat`, `concat_ws`, phần tử của
  danh sách, struct, map viết thẳng; dùng lại bí danh cộng số mảnh của bí danh. Tuple, mảng ở vế phải của `IN`,
  `= ANY(...)`, `= ALL(...)` không tính.
- Chuỗi hằng tối đa 1000 byte, ở mọi chỗ (`width.rs`).
- Mỗi danh sách cột của SELECT tối đa 200 mục. `*` và `t.*` là một mục khi `FROM` có một nguồn; nhiều nguồn thì tính
  theo số cột thật trong hồ sơ của bảng (hộp lọc của t tính như t), và bị từ chối khi có bảng tạm hay câu con khác.
- Mỗi `FROM` tối đa 6 nguồn, tính cả `JOIN`, bảng tạm, câu con. Số dòng do `JOIN` nhân lên chỉ bị giới hạn lúc chạy
  (`engine/query.rs`).
- Tối đa 10 nhánh `UNION`, `INTERSECT`, `EXCEPT` trong cả câu, cộng qua mọi tầng: mỗi nhánh `UNION ALL` chép lại cùng
  giá trị thành một dòng, nên nhân như chép cột.

Trường hợp xấu nhất về độ dài một giá trị: 20 mảnh chia đều qua 4 tầng, tức 625 lần chuỗi hằng 1000 byte; chép ra 200
cột thì còn 3 tầng ghép (7, 7, 6), tức 294 lần mỗi cột. 6 tầng đã đo vượt mức khoảng 300 MB (chép 200 cột 486 MB,
`UNION ALL` 74 nhánh 2,4 GB), nên giữ 4.

Số đo dưới đây: qua guard rồi chạy bằng `run_query` trên máy phát triển, giới hạn 1 giây, mỗi ca một tiến trình có
`ulimit -v` 4 GB, đọc VmHWM (RSS đỉnh), ngày 23 tới 25/10/2026, khi trần là 4 nguồn mỗi `FROM`; chưa đo lại với 6.

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
| `UNION ALL` 10 nhánh, mỗi nhánh 200 cột, giá trị 10/10 mảnh | 5,4 KB | 534 MB | 10 dòng |
| `UNION ALL` 10 nhánh, mỗi nhánh 200 cột, ghép 7 chuỗi hằng rồi 7, 6 lần | 11,4 KB | 1,37 GB | 10 dòng |
| chuỗi 1000 byte trên 50 000 dòng của bảng | 1 KB | 127 MB | xong |
| `string_agg` chuỗi 1000 byte trên 50 000 dòng | 1,1 KB | 227 MB | chuỗi 50 MB |
| chuỗi 1000 byte, 50 000 dòng, 200 cột | 1,4 KB | 1,7 GB | dừng vì quá giờ |
| 500 dòng, 100 cột, giá trị 2/2 mảnh | 1,3 KB | 1,03 GB | xong |
| 5000 dòng, 200 cột, giá trị 10/10 mảnh | 1,6 KB | 1,9 GB | OutOfMemory |

Các ca cho một dòng kết quả ở dưới 270 MB. Các ca nhân theo số dòng vẫn tới khoảng 1,4 đến 1,9 GB (mục Hạn chế đã
biết). Thời điểm DuckDB kiểm cờ dừng đổi giữa các lần chạy, nên số của ca bị dừng vì quá giờ dao động.

**Cột.** Tên cột phải là cột của một bảng câu đang dùng, hay tên chính câu đặt ra (`AS tong`, `WITH t(a, b)`,
`(SELECT ...) AS x(a, b)`). Dạng `bang.cot` thì `bang` phải là bảng, bảng tạm hay bí danh có trong câu. Tên ba phần
(`main.sales.region`) bị từ chối.

**Nối bảng** (`engine/guard/joins.rs` và các file `join_*.rs`). Không có luật này, `JOIN orders o ON i.order_item_id =
o.customer_id` chạy được và ra số sai mà trông hợp lý. Guard nhận quan hệ của các bảng (`JoinLink`: bảng giữ khóa ngoại,
cột, bảng được trỏ tới, cột), cộng quan hệ suy ra từ điều kiện lan theo khóa ngoại (bước 3). Luật chạy sau các luật kích
thước, trước bước 3, xét từng SELECT ở mọi tầng.

- Nguồn giữ nguyên dòng là bảng dữ liệu, hay câu con, bảng tạm chỉ lọc và chọn cột của một bảng. Hai nguồn như vậy nối
  với nhau thì các phép bằng giữa cột của hai nguồn, ở `ON`, `USING` hay `WHERE`, gộp lại phải đúng bằng cột của một khóa
  ngoại giữa hai bảng: khóa nhiều cột thì đủ cả bộ; thứ tự cặp và chiều phép bằng tùy ý. Thiếu, sai hay thừa một phép
  bằng đều bị từ chối.
- Vế `ON` đụng hai bảng chỉ được là phép bằng giữa hai cột; vế chỉ lọc một bảng thì được. `OR`, so sánh lớn nhỏ hay
  biểu thức giữa hai bảng bị từ chối. Ở `WHERE`, vế không phải phép bằng giữa hai cột là điều kiện lọc.
- Mọi bảng giữ nguyên dòng trong một `FROM` phải nối với nhau, trực tiếp hay qua nguồn khác: `CROSS JOIN`,
  `JOIN ... ON true`, `FROM a, b` không điều kiện nối, `NATURAL JOIN` bị từ chối. `USING (c)` chỉ qua khi c là khóa cùng
  tên ở hai bảng.
- Bảng nối với chính nó chỉ qua khóa ngoại tự trỏ (`employees.manager_id` trỏ `employees.employee_id`).
- Nguồn đã gộp nhóm (bảng tạm một dòng mỗi đơn, mỗi khách) và nửa nối (`x IN (SELECT y ...)`, `EXISTS`, câu con tương
  quan): mỗi phép bằng trên cột lần được về bảng dữ liệu phải so hai cột cùng miền giá trị: cùng một cột, một cặp của
  khóa ngoại, hay hai khóa ngoại trỏ cùng cột của bảng cha; phần là khóa ngoại phải đủ cột. Cột tính ra (tháng, năm)
  không phải cột của bảng nào nên không kiểm.
- Dataset file không có quan hệ: câu một bảng như mọi khi; chỉ câu nối bảng với chính nó bị từ chối.
- Cột không ghi bảng mà có ở hai nguồn trong `ON` bị từ chối, lý do bảo ghi rõ bảng.
- Lý do từ chối nêu hai bảng và đúng khóa nối được, ví dụ "Hai bảng order_items và orders chỉ nối theo khóa ngoại, đủ
  mọi cột của khóa: order_items.order_id = orders.order_id", hay nói hai bảng không có quan hệ trong database.
- Câu do code dựng đi cùng luật mà không cần ngoại lệ: phân rã theo bảng liên quan nối `LEFT JOIN` đúng khóa; truy dòng
  nguồn giữ nguyên các `JOIN` của câu đã chạy và lấy `<bảng>.*` của một bảng (luật số cột đếm `t.*` trên hộp lọc của t
  theo số cột của t).
- Hạn chế: không lần cột qua `UNION`; câu con tương quan chỉ có so lớn nhỏ không bị kiểm.

## Bước 3: lọc dòng theo vai dữ liệu

Mỗi `AllowedTable` mang `row_filters` do service dựng (`service/ask/row_filters.rs`):

- `RowFilter::Column { column, allowed_values }`: một luật (một dòng `rls_policies`) của vai người gọi mang.
- `RowFilter::Parent { columns, parent_columns, parent }`: điều kiện lan theo khóa ngoại, mục bên dưới.
- `RowFilter::AnyOf(groups)`: người gọi mang nhiều vai có điều kiện trên bảng này. Mỗi nhóm là điều kiện của một vai
  (các luật nối AND), các nhóm nối OR: `(("region" IN ('South')) OR ("region" IN ('North')))`. Một vai thì service
  truyền thẳng điều kiện của vai đó.

Rỗng thì bảng đó không lọc. Guard không tự đọc chính sách và không quyết ai được thấy hết.

Guard thay mỗi chỗ đọc bảng có điều kiện bằng một câu con chỉ chứa dòng được phép, giữ bí danh của câu. Ví dụ luật
`region` chỉ `South` (lấy từ test):

```
Câu vào:      SELECT region FROM sales
shown:        SELECT region FROM (SELECT * FROM sales WHERE "region" IN ('South')) AS sales LIMIT 100
runnable:     SELECT region FROM (SELECT * FROM read_parquet('/data/sales/*.parquet', union_by_name = true) AS sales WHERE "region" IN ('South')) AS sales LIMIT 100
row_filters:  "region" IN ('South')
```

- Bọc mọi chỗ đọc: `JOIN` mọi kiểu, `FROM a, b`, câu con ở mọi chỗ, `LATERAL`, thân bảng tạm, mọi vế `UNION`,
  `INTERSECT`, `EXCEPT`. Bảng đọc nhiều lần thì bọc từng lần. Lọc theo bảng được đọc, không theo bí danh.
- Câu con chứ không thêm vế vào `WHERE`: điều kiện trong câu con chạy trước mọi `JOIN`, `WHERE`, `OR` của câu ngoài.
  Thêm vế vào `WHERE` thì lọt: `WHERE region = 'North' OR true AND "region" IN ('South')` ra cả dòng North vì `AND` tính
  trước `OR`. Câu con không làm chậm: `EXPLAIN` của câu bọc và của câu viết `WHERE` tay ra cùng kế hoạch.
- `allowed_values` rỗng thành `WHERE false`. Ô trống (NULL) ở cột lọc bị ẩn, vì NULL không khớp `IN`. Giá trị so dạng
  chuỗi: khác hoa thường thì không khớp.
- Điều kiện dựng trên cây cú pháp, không ghép chuỗi: giá trị `South') OR ('1' = '1` chỉ là một chuỗi để so.
- Câu chạy lỗi không chép ô của dòng bị lọc vào lời báo: `EXPLAIN` cho thấy DuckDB đặt bộ lọc có `CAST` của câu ngoài
  trên điều kiện lọc, kể cả khi điều kiện được đẩy xuống `READ_PARQUET` hay là nửa nối theo khóa ngoại. Đây là
  cách bộ tối ưu của DuckDB làm, không phải guard bảo đảm; `service/ask/repair_row_filter_tests.rs` chạy 17 dạng câu
  (`WHERE`, `JOIN ... ON`, câu con, `EXISTS`, `ORDER BY ... LIMIT`, `UNION`, cửa sổ) với 4 dạng điều kiện để bắt khi
  nâng DuckDB.
- Chặn cả câu (`GuardError::Internal`, không vào vòng sửa) khi cột của chính sách không có trong bảng (không chặn thì
  DuckDB lấy cột cùng tên của bảng ở câu ngoài và mọi dòng lọt ra), hay khi gặp chỗ đọc dữ liệu bước 3 không nhận ra.
- `GuardedSql.row_filters` liệt kê điều kiện đã thêm, in đúng như trong `shown`, để thẻ Cách tính gắn nhãn "Lọc theo
  quyền xem". Không có điều kiện thì `shown` và `runnable` như khi chưa có bước này.

### Điều kiện lan theo khóa ngoại

Luật chỉ gắn với bảng có cột lọc. Người bị lọc theo `customers.region` mà đọc `orders` (không có cột vùng) sẽ thấy đơn
của mọi vùng. Vì vậy với mỗi quan hệ trong `dataset_relations` mà bảng được trỏ tới có điều kiện với người hỏi, service
thêm cho bảng trỏ đi một `RowFilter::Parent`, guard in thành (lấy từ test):

```
Câu vào:      SELECT COUNT(*) AS n FROM orders
shown:        SELECT COUNT(*) AS n FROM (SELECT * FROM orders WHERE "customer_id" IN (SELECT "id" FROM "customers" WHERE "region" IN ('South'))) AS orders LIMIT 100
row_filters:  "customer_id" IN (SELECT "id" FROM "customers" WHERE "region" IN ('South'))
```

- Điều kiện do code dựng từ quan hệ đã ghi lúc chụp, nằm trong hộp lọc, nên áp cả khi câu chỉ đọc `orders`.
- Nửa nối (`IN` câu con) chứ không `JOIN` bảng cha: `JOIN` nhân dòng khi khóa bên cha lặp; `IN` chỉ giữ hay bỏ từng dòng.
- Câu con trên bảng cha mang chính điều kiện của bảng cha, nên lan nhiều bước (`order_items` qua `orders` tới
  `customers`). Service đi hết các quan hệ theo chiều khóa ngoại.
- Khóa nhiều cột: `("maker", "sku") IN (SELECT "maker", "sku" FROM "products" WHERE ...)`.
- Khóa ngoại rỗng, một đường: dòng bị ẩn. Nhiều đường tới bảng bị lọc (`order_items.order_id` qua `orders`, và
  `order_items.gift_customer_id`): mỗi đường thành `("fk" IS NULL OR "fk" IN (SELECT ...))`, các đường nối `AND`, cộng
  điều kiện ít nhất một khóa không rỗng. Khóa tùy chọn rỗng thì đường đó được bỏ qua; mọi đường rỗng thì dòng ẩn. Khóa
  nhiều cột rỗng một phần thì không khớp `IN` và dòng bị ẩn.
- Bảng có chính sách riêng giữ chính sách đó và cộng điều kiện lan (`AND`). Quan hệ một-một lan như nhiều-một.
- Vòng quan hệ dừng ở bảng đã có trên đường đang đi. Bảng cha bị lọc mà chưa đọc được (chưa có phiên bản) thì bảng con
  thành `WHERE false`.
- Bảng cha đọc từ Parquet của phiên bản đang dùng kể cả khi mô hình không được đọc bảng đó; bước 2 vẫn từ chối câu đọc
  thẳng bảng cha không có trong danh sách và bảng tạm trùng tên bảng cha.
- Cột khóa phải có trong hồ sơ của cả hai bảng, số cột hai bên bằng nhau; sai thì `GuardError::Internal`.
- Hiệu năng: trên dữ liệu tổng hợp cỡ schema `sales` của database mẫu (20 652 khách, 65 752 đơn, 180 519 dòng hàng),
  `SELECT COUNT(*), SUM(qty) FROM order_items` với điều kiện lan hai bước mất khoảng 7 ms, không lọc khoảng 0,7 ms (trung
  vị 7 lần, bản release, máy 16 nhân, 30/10/2026; test tạm, không giữ trong repo). Chưa đo trên câu nhiều bảng.

## Bước 4: giới hạn số dòng

`max_rows` do nơi gọi đặt; luồng hỏi dùng `ASK_MAX_ROWS` (mặc định 100 000). Giới hạn đặt trên truy vấn ngoài cùng, nên
áp cho cả chuỗi `UNION`.

| Câu vào | Guard làm |
|---|---|
| Không có `LIMIT` | Thêm `LIMIT max_rows` |
| `LIMIT n` với n không quá `max_rows`, có hay không `OFFSET m` | Giữ nguyên |
| `LIMIT n` với n lớn hơn `max_rows` | Hạ còn `max_rows` |
| `LIMIT` là biểu thức hay truy vấn con | Thay bằng `max_rows` |
| `FETCH FIRST n ROWS ONLY`, `LIMIT n BY cot` | Từ chối, lý do nhắc dùng `LIMIT` |

## Bước 5: hai bản shown và runnable

| Bản | Dùng để | Nội dung |
|---|---|---|
| `shown` | Hiện cho người dùng, lưu vào `queries.sql_guarded` | Câu đã kiểm, còn tên bảng logic, có `LIMIT`; không có đường dẫn file |
| `runnable` | Chỉ đưa cho DuckDB | Bảng logic thành `read_parquet(...)` đọc thư mục Parquet của phiên bản; bảng tạm đổi tên |

```
Câu vào:   SELECT sales.region FROM sales
shown:     SELECT sales.region FROM sales LIMIT 100
runnable:  SELECT sales.region FROM read_parquet('/data/sales/*.parquet', union_by_name = true) AS sales LIMIT 100
```

- Tên bảng cũ thành bí danh khi câu chưa đặt bí danh, để `sales.region` vẫn trỏ đúng bảng.
- Bảng tạm đổi thành `guard_cte_1`, `guard_cte_2`...: giữ tên thì một bảng tạm tên `"secret.csv"` gọi ngoài chỗ định
  nghĩa sẽ thành đọc file `secret.csv`.

## Kiểm cộng lặp sau guard

Câu qua guard vẫn có thể ra số sai mà trông hợp lý: nối `students` với `student_assessments` rồi
`SUM(s.studied_credits)` thì tín chỉ của mỗi sinh viên được cộng một lần cho mỗi bài nộp (trên OULAD của database mẫu,
vùng East Anglian Region ra 1 357 190 thay vì 260 380, đo bằng psql). Vì vậy luồng hỏi gọi `find_double_count` của
`service/ask/fanout.rs` sau guard và trước khi chạy. Kiểm tra này không thuộc guard vì cần chiều của quan hệ
(`dataset_relations`, bảng giữ khóa ngoại là phía nhiều).

- Bị bắt khi đối số của `SUM` hay `AVG` chỉ gồm cột của một bảng P (cộng hằng số), và `FROM` có bảng C làm dòng của P
  lặp: C trỏ tới P theo quan hệ `many_to_one`, hay trỏ tới một bảng mà P đi tới được không nhân dòng. Ví dụ
  `order_items` và `returns` cùng trỏ `orders`: `SUM(oi.qty)` khi nối cả ba bảng cũng bị bắt.
- Đọc đối số bỏ qua ngoặc, `CAST`, dấu âm, `COALESCE`, `IFNULL`, `ROUND`, `ABS`, `DISTINCT`; `CASE` chỉ xét nhánh
  `THEN` và `ELSE`.
- Không bị bắt: nhân cột của P với cột của C (`SUM(oi.quantity * p.price)`), quan hệ `one_to_one`, `MIN`, `MAX`,
  `COUNT(DISTINCT ...)`. Không chắc thì không bắt: cột không ghi bảng có ở hai bảng, `FROM` có bảng tạm hay câu con, bí
  danh lạ, hàm khác.

Bị bắt thì câu không chạy, bước `guard` ghi lý do, và mô hình nhận gợi ý trong vòng sửa: gộp C theo khóa nối trước,
hoặc giữ một dòng cho mỗi dòng P rồi mới cộng, hoặc bỏ phép nối. Câu sửa vẫn cộng lặp thì trả `rejected`, lời nói rõ cột
của bảng nào bị tính lặp.

Hạn chế: không lần theo `WITH` hay bảng con (SUM trên bảng tạm đã nối mà chưa gộp không bị bắt); không xét `GROUP BY`
(nhóm theo khóa của C thì SUM cột P không lặp nhưng vẫn bị bắt); `COUNT(*)` sau khi nối cũng đếm lặp, chưa xét.

## Hạn chế đã biết

- Cây do `sqlparser` dựng, không phải DuckDB, nên với cú pháp hiếm hai bên có thể hiểu khác nhau.
- Cột kiểm trên cả câu, không theo phạm vi từng truy vấn con: cột của truy vấn con này dùng ở truy vấn con khác vẫn qua
  và chỉ bị DuckDB báo lỗi. Kiểm cột để mô hình sửa tên, không để chặn dữ liệu; chặn dữ liệu là việc của danh sách bảng
  và hàm.
- Trường của cột kiểu struct (`cot.truong`) bị từ chối, vì guard hiểu `cot` là tên bảng.
- Guard chỉ giới hạn số dòng trả về. Thời gian (`QUERY_TIMEOUT_SECS`) và bộ nhớ (1 GB mỗi câu) giới hạn lúc chạy, ở
  `engine/query.rs`. Biểu thức hằng DuckDB tính lúc chuẩn bị câu, hay một lời gọi hàm dựng chuỗi rất dài, không dừng
  được giữa chừng; bước 2 thu hẹp phần này. Câu nhân theo số dòng vẫn lên khoảng 1,4 đến 1,9 GB trên các dạng đã đo,
  vì phần đổi kết quả sang Arrow và sang giá trị của backend nằm ngoài trần của DuckDB. Đó là số đo, không phải bảo đảm;
  bảo đảm cần chạy câu trong tiến trình con có trần RAM và thời gian của hệ điều hành, như bộ đọc Excel
  ([#119](https://github.com/pgiahuy/xdi-platform/issues/119)).
- `row_filters` rỗng là không lọc: service phải luôn truyền đủ điều kiện của người hỏi, quên truyền thì người đó thấy
  mọi dòng. Mọi nơi chạy SQL trên dữ liệu lấy điều kiện qua cùng một hàm (`load_row_filters`).
- Điều kiện lan chỉ đi theo quan hệ đã ghi lúc chụp database. Dataset file không có quan hệ nên không nhận điều kiện lan.
- Người bị lọc đọc thêm bảng cha trong mỗi câu chạy trên bảng con; số đo ở bước 3 là một câu trên dữ liệu tổng hợp.
