# Dữ liệu và quyền riêng tư

Trang này nói dữ liệu nằm ở đâu, ai thấy dòng nào, gửi gì cho mô hình ngôn ngữ và cách xóa.

## Nguyên tắc

- Phần mềm chạy trên máy hay máy chủ của bạn. Dữ liệu nằm trong Postgres và thư mục dữ liệu (`DATA_DIR`) của bản cài
  đó.
- Không có telemetry: phần mềm không gửi gì về cho nhóm phát triển.
- Con số do DuckDB tính trên máy của bạn. Mô hình ngôn ngữ viết mô tả cột, phân loại câu hỏi, viết SQL (hay điền form
  của câu hỏi nguyên nhân), viết lời giải thích và kiểm nghĩa của lời đó.
- Mô hình nhận metadata (tên bảng, tên cột, kiểu, mô tả), danh sách giá trị của cột phân loại không nhạy cảm và các con
  số code đã tính kèm tên nhóm. Mô hình không nhận dòng dữ liệu gốc, nội dung file hay dòng của bảng kết quả, trừ hai
  ngoại lệ ghi ở mục [Gửi gì cho mô hình](#gửi-gì-cho-mô-hình): lời báo lỗi của DuckDB ở vòng sửa SQL, và kết quả chỉ
  một dòng ngắn.
- Người bị lọc dòng theo vai dữ liệu không gửi danh sách giá trị nào cho mô hình; các con số gửi đi tính trên đúng các
  dòng họ được thấy.
- Mô tả bảng và cột tổ chức ghi trong database nguồn (`COMMENT ON`) được gửi cho mô hình như mô tả cột. Comment có chép
  dữ liệu thật (ví dụ tên một khách hàng) thì chữ đó cũng tới mô hình.

## Dữ liệu nằm ở đâu

| Dữ liệu | Nơi lưu |
|---|---|
| File nạp lên | Parquet trong `DATA_DIR` (volume `xdidata` khi chạy compose), mỗi bảng, mỗi phiên bản một thư mục. File gốc chỉ nằm trong `DATA_DIR/staging` lúc nạp, xóa ngay sau đó; backend dừng giữa chừng thì lần khởi động sau xóa |
| Bảng chụp từ database Postgres | Parquet trong `DATA_DIR`, như file nạp lên. Bảng quá trần chỉ lưu metadata, không có dòng nào (mục dưới) |
| File tạm lúc chụp | Cách `direct` (mặc định): mật khẩu và CA ghi vào một thư mục tạm trong `DATA_DIR/staging`, thư mục quyền 0700, file quyền 0600, xóa khi lần chụp xong hay lỗi. Cách `copy`: file CSV trung gian trong `DATA_DIR/staging`, xóa sau lần chụp |
| Thông tin kết nối nguồn database: máy chủ, cổng, database, tài khoản, mật khẩu, sslmode, CA | Postgres, bảng `sources`, mã hóa AES-256-GCM bằng `SOURCE_SECRET_KEY`; máy chủ, cổng, database, tài khoản, sslmode có thêm bản chữ thường để hiện cho admin (đoạn dưới bảng) |
| Bảng chỉ lưu metadata: tên bảng, số dòng, dung lượng, cột (tên, kiểu, khóa chính, mô tả), mô tả bảng | Postgres, bảng `source_metadata_tables`; mọi thành viên workspace đọc được qua `GET .../catalog` |
| Danh sách bảng của database nguồn ở màn kết nối (tên, số dòng ước lượng, dung lượng, comment, khóa ngoại) | Đọc từ catalog của database nguồn mỗi lần mở, không lưu |
| Quan hệ giữa các bảng đã chụp (tên ràng buộc khóa ngoại, cột khóa hai đầu) | Postgres, bảng `dataset_relations`; mọi thành viên workspace đọc được |
| Hồ sơ cột: tên, kiểu, vai trò, min, max, số ô trống, giá trị kèm số dòng của cột tới 200 giá trị, đơn vị, cờ nhạy cảm, mô tả, bí danh, câu hỏi gợi ý | Postgres |
| Lịch sử câu hỏi: người hỏi, câu hỏi, SQL mô hình viết và SQL sau guard, trạng thái, lỗi, thời gian các bước, số dòng kết quả, thẻ "Cách tính" dạng chữ ngắn, lời trả lời đã hiện kèm kết quả kiểm số và kiểm nghĩa; với câu hỏi nguyên nhân, form và bảng phân rã | Postgres, bảng `queries`. Bảng kết quả không lưu. Lời báo lỗi và lời trả lời có thể chứa giá trị và con số của dữ liệu. Câu hỏi của một người chỉ người đó xem lại được, trừ câu họ bấm Tốt (dòng dưới) |
| Đánh giá câu trả lời: Tốt hay Chưa đạt, lý do, ghi chú, ai đánh giá và lúc nào; ai duyệt câu làm câu mẫu | Postgres, cùng dòng lịch sử. Admin thấy câu hỏi, SQL và thẻ "Cách tính" của câu được bấm Tốt ở màn Câu mẫu |
| Nhật ký lỗi: câu hỏi, bảng, bước sai, lời báo lỗi, câu mô hình trả ở bước đó, ghi chú Chưa đạt | Postgres, bảng `error_log`; chỉ admin của workspace xem, thấy câu của mọi thành viên |
| Bài học: chữ admin viết, phạm vi (một bảng hay cả workspace), bật hay tắt, người tạo, người duyệt | Postgres, bảng `lessons`; chỉ admin xem |
| Báo cáo: tiêu đề, đoạn mở đầu, thứ tự mục, mỗi mục trỏ tới một câu hỏi đã lưu; báo cáo tự động ghi thêm dataset và số lần gọi mô hình | Postgres, bảng `reports`, `report_items`; chỉ người tạo đọc được, admin cũng không. Mỗi phần của báo cáo tự động là một dòng lịch sử câu hỏi. File PDF, Excel tải về nằm ở máy người tải |
| Vai dữ liệu, luật lọc dòng, ai mang vai nào | Postgres, bảng `data_roles`, `member_data_roles`, `rls_policies` |
| Database mẫu (`data/postgres-demo`, dịch vụ `demo-db` của `--profile demo`) | Dữ liệu công khai (DataCo, OULAD) trong volume `xdi-demo-db_demo-db-data`, cổng chỉ mở trên `127.0.0.1` |
| Tài khoản: email, tên, mật khẩu đã băm (Argon2id) | Postgres; không lưu mật khẩu gốc |
| Phiên đăng nhập: mã băm SHA-256, người dùng, hạn | Postgres; mã gốc chỉ nằm trong cookie của trình duyệt, xóa khi đăng xuất |

Mật khẩu của nguồn database không trả qua API, không ghi log và không nằm trong chuỗi kết nối gửi cho DuckDB. Cột
`_source_file` của bảng chụp chỉ ghi máy chủ, database và tên bảng. `SOURCE_SECRET_KEY` để trống thì backend tự sinh
khóa và lưu ở `DATA_DIR/source_secret.key` (quyền 0600), cùng volume với dữ liệu: bản sao lưu của `make backup` có cả
khóa lẫn bản mã, nên giữ bản sao lưu như giữ mật khẩu. Bản chạy thật nên đặt biến để khóa nằm ngoài volume.

Phạm vi chụp là quyền `SELECT` của tài khoản nguồn: màn kết nối liệt kê mọi bảng và view tài khoản đó đọc được, tick
sẵn, admin bỏ tick bảng không cần. Muốn giới hạn dữ liệu thì cấp quyền hẹp cho tài khoản, không dựa vào việc bỏ tick.

## Ai thấy dòng nào

- Admin của workspace thấy mọi dòng, kể cả khi mang vai dữ liệu: họ tự sửa được luật, nên lọc với họ không bảo vệ gì.
- Member không mang vai nào thấy mọi dòng. Member mang vai chỉ thấy dòng các vai của mình cho phép: trong một vai các
  luật nối AND, giữa các vai nối OR, chỉ tính vai có luật trên bảng đang đọc hay bảng nó trỏ tới. Bảng trỏ tới bảng bị
  lọc (đơn hàng của khách) bị lọc theo.
- Luật áp cho mọi SQL chạy thay mặt người đó: câu hỏi, phân rã, truy dòng nguồn, tổng quan dataset, báo cáo. Mở báo
  cáo hay xuất Excel thì câu SQL đã lưu chạy lại với quyền của người mở; vai đã đổi từ lúc hỏi thì lời đã lưu không
  hiện. Người bị lọc không thấy số đếm của hồ sơ cột ở trang Tổng quan.
- Vai, luật và người mang vai chỉ admin xem và sửa (màn **Vai dữ liệu**, `/data-roles`). Member chỉ thấy tên và mô tả
  vai của mình ở trang Thành viên; thẻ "Cách tính" ghi điều kiện đã áp ("Lọc theo quyền xem: ...").
- Đổi vai hay luật có hiệu lực từ request kế tiếp. Lời trả lời cũ trong lịch sử giữ số đã tính lúc hỏi.

## Nhà cung cấp mô hình

| `LLM_PROVIDER` | Dữ liệu đi đâu |
|---|---|
| `mock` (mặc định) | Không gửi đi đâu |
| `ollama` | Tới máy chạy Ollama ở `OLLAMA_URL`. Ollama chạy trong mạng của bạn thì dữ liệu không ra ngoài |
| `gemini` | Tới API của Google, theo [điều khoản Gemini API](https://ai.google.dev/gemini-api/terms). Gói miễn phí (dự án Google Cloud chưa gắn thanh toán): Google được dùng nội dung gửi lên và câu trả lời để cải thiện sản phẩm, có thể có người đọc. Gói trả phí không dùng cho việc đó, nhưng Google vẫn lưu một thời gian để phát hiện vi phạm |

Dữ liệu nhạy cảm thì dùng `mock` hay Ollama chạy nội bộ.

## Gửi gì cho mô hình

Mỗi lượt gọi và phần chính của nó. Chữ cố định của từng prompt (luật, khuôn câu, chỗ điền dữ liệu) nằm trong
[`backend/prompts/`](https://github.com/pgiahuy/xdi-platform/tree/main/backend/prompts); riêng prompt viết lời câu hỏi
nguyên nhân dựng trong `backend/src/service/explain/narrate_prompt.rs`.

| Lượt gọi | Khi nào | Gửi |
|---|---|---|
| Lập hồ sơ ngữ nghĩa | Sau khi nạp hay chụp một bảng, ba prompt song song | Metadata cột, min, max, giá trị của cột phân loại |
| Phân loại | Mỗi câu hỏi một lần | Câu hỏi, metadata cột, câu hỏi các lượt trước |
| Viết SQL | Câu hỏi số liệu | Câu hỏi, metadata, danh sách giá trị, chỉ số, quan hệ, câu mẫu, bài học, các lượt trước |
| Sửa SQL | SQL lỗi hay bị guard từ chối, một lần | Prompt viết SQL, câu SQL, lời báo lỗi |
| Gửi lại kèm bảng hay giá trị | SQL cần bảng chỉ có tên, hay lọc theo giá trị không có | Prompt viết SQL, thêm chi tiết bảng hay đủ danh sách giá trị |
| Điền form | Câu hỏi nguyên nhân | Như viết SQL, chỉ bảng bắt đầu, chỉ số không có công thức |
| Sửa form | Form sai, một lần | Prompt form, form, lỗi từng trường |
| Viết lời câu hỏi nguyên nhân | Sau khi có bảng phân rã | Câu hỏi, số của bảng phân rã, bài học |
| Viết lời câu hỏi số liệu | Sau khi có kết quả gộp nhóm hay một dòng gửi được | Câu hỏi, các câu code đã viết (số đã tính, tên nhóm), bài học |
| Kiểm nghĩa | Sau mỗi lời qua kiểm số | Lời vừa viết và các số hay câu nó dựa vào |
| Báo cáo tự động | Mỗi phần của báo cáo | Như viết lời câu hỏi số liệu và kiểm nghĩa |

Không gửi: dòng dữ liệu gốc, nội dung file, đường dẫn file trên máy chủ, danh sách giá trị của cột chữ tự do, cột khóa
và cột nhạy cảm, dữ liệu của workspace khác, mật khẩu, khóa API, token đăng nhập, thông tin kết nối nguồn database. Một
giá trị đơn lẻ vẫn có thể tới mô hình khi nó nằm trong câu hỏi, trong câu SQL của lượt trước hay của câu mẫu, hay trong
lời báo lỗi của DuckDB (mục Sửa SQL).
Truy dòng nguồn, trang Tổng quan, chỉ số, mở lại báo cáo và xuất PDF, Excel không gọi mô hình.

### Lập hồ sơ ngữ nghĩa

- Tên dataset, tên bảng logic, số dòng, mô tả bảng từ database nguồn nếu có.
- Mỗi cột: tên, kiểu, vai trò, % ô trống, số giá trị khác nhau, mô tả từ database nguồn nếu có; min, max của cột thời
  gian và cột số đo số thực không nhạy cảm.
- Giá trị kèm số dòng của cột phân loại không nhạy cảm: đủ danh sách khi cột tới 50 giá trị, không thì 20 giá trị nhiều
  dòng nhất; tổng tối đa 400 giá trị một lần; bỏ giá trị dài quá 200 byte.
- Không gửi giá trị của cột chữ tự do, cột khóa, cột nhạy cảm.

### Phân loại câu hỏi

- Câu hỏi, câu hỏi của tối đa 4 lượt gần nhất trong cùng cuộc trò chuyện.
- Metadata cột của các bảng gửi chi tiết (tên, kiểu, vai trò, định dạng ngày, đơn vị, cách gộp, mô tả, bí danh), kể cả
  cột nhạy cảm (chỉ tên, không giá trị).
- Database nhiều bảng mà metadata vượt nửa trần prompt: các bảng còn lại, kể cả bảng chỉ lưu metadata, nằm trong một
  danh sách gọn, mỗi bảng tên và một dòng mô tả (dòng đầu của `COMMENT ON TABLE`, không có thì tên vài cột đầu).
- Không gửi danh sách giá trị, chỉ số, SQL hay thẻ "Cách tính" của các lượt trước.

### Viết SQL và điền form

- Câu hỏi, lựa chọn người hỏi gửi kèm, khối các lượt trước (dưới).
- Metadata cột như lượt phân loại. Tổng metadata quá 60 000 ký tự thì cột không liên quan chỉ còn tên, kiểu, vai trò.
- Danh sách giá trị kèm bí danh của cột phân loại không nhạy cảm: của mọi cột khi tổng tới 1000 giá trị, vượt thì chỉ
  cột liên quan tới câu hỏi, tối đa 200 giá trị mỗi cột. Cụm của câu hỏi bằng đúng một giá trị được gửi làm gợi ý.
- Định nghĩa chỉ số (tên, công thức, điều kiện, cột thời gian, đơn vị, mô tả); prompt form chỉ có tên và mô tả.
- Quan hệ khóa ngoại giữa các bảng gửi chi tiết, và danh sách gọn các bảng còn lại như lượt phân loại.
- Tối đa 3 câu mẫu admin đã duyệt của cùng dataset có chung từ với câu hỏi: câu hỏi kèm SQL (prompt SQL) hay kèm form
  (prompt form).
- Tối đa 3 bài học đã duyệt, đang bật, liên quan tới câu hỏi. Chỉ chữ admin đã duyệt; ghi chú Chưa đạt và nhật ký lỗi
  không vào prompt.
- Khái niệm của gói ngành gắn với dataset; chưa có cách gắn gói, nên hiện không gửi.
- Prompt form của câu hỏi nguyên nhân chỉ có cột của bảng bắt đầu.

Người bị lọc dòng theo vai dữ liệu trên một bảng gửi chi tiết không gửi danh sách giá trị hay gợi ý giá trị nào, chỉ
metadata, và không có lần gửi lại kèm giá trị.

Hỏi tiếp trong một cuộc trò chuyện: prompt SQL và prompt form có thêm tối đa 4 lượt gần nhất của cùng cuộc, mỗi lượt là
câu hỏi, trạng thái, câu SQL và thẻ "Cách tính" dạng chữ ngắn (không gồm điều kiện lọc theo vai), cả khối tối đa 4 000
ký tự. Câu SQL và bộ lọc có thể chứa giá trị của cột phân loại. Không gửi kết quả, lời trả lời hay lời báo lỗi của các
lượt trước.

### Sửa SQL, sửa form và gửi lại

- Sửa SQL (một lần): prompt của lần hỏi, câu SQL vừa viết, lời báo lỗi của DuckDB hay lý do guard từ chối. Lời của
  DuckDB có thể chép một giá trị của dữ liệu (ví dụ một chuỗi ngày không đọc được). Câu đọc cột nhạy cảm thì mọi chuỗi
  trong nháy đơn hay nháy kép của lời báo, trừ chuỗi có sẵn trong câu SQL và tên bảng, tên cột, được thay bằng
  `<hidden value>`, cả dòng `strptime` chép lại giá trị đó. Đường dẫn file được thay bằng tên bảng. Với người bị lọc dòng,
  giá trị trong lời báo là của dòng họ được thấy: DuckDB tính biểu thức của câu sau điều kiện lọc của guard. Đó là cách
  DuckDB xếp bộ lọc, đã kiểm trên bản DuckDB đang dùng với 17 dạng câu và 4 dạng điều kiện lọc
  (`service/ask/repair_row_filter_tests.rs`, `api/ask_row_filter_error_tests.rs`); nâng DuckDB thì các test này kiểm lại.
- Sửa form (một lần): prompt form, form vừa điền, lỗi từng trường; lỗi có thể kèm tối đa 20 giá trị của một cột phân
  loại và ngày đầu, ngày cuối của cột thời gian.
- Gửi lại kèm bảng (một lần): SQL đọc bảng chỉ có trong danh sách gọn thì prompt SQL được dựng lại kèm chi tiết bảng đó.
- Gửi lại kèm giá trị (một lần): SQL lọc theo giá trị cột không có mà danh sách của cột chưa đủ trong prompt thì gửi lại
  kèm đủ danh sách của cột đó.

### Viết lời câu hỏi nguyên nhân

- Câu hỏi, bài học liên quan, và bảng phân rã đã tính: độ đo, nhãn và giá trị hai bên, chênh lệch, % thay đổi, nhóm
  chính; hai chiều đầu, mỗi chiều ba nhóm đầu kèm giá trị nhóm, phần đóng góp, % thay đổi và cờ khác thường; tên các
  chiều bỏ qua; tách lượng và giá nếu có.
- Lời trượt kiểm số hay kiểm nghĩa thì gửi lại prompt đó một lần kèm lý do (có thể chép chữ của lời vừa viết).
- Chênh lệch bằng 0 thì không gọi mô hình.
- Không gửi dòng dữ liệu gốc.

### Viết lời câu hỏi số liệu

- Câu hỏi, bài học liên quan, các câu code đã viết: số đã tính (tổng, chênh lệch, phần trăm), nhãn kỳ, tên độ đo, tên
  vài nhóm của bảng kết quả (nhóm đứng đầu, nhóm lệch hẳn mức trung bình, hai hạng đầu), và khi có phân rã thì tên cột
  chiều cùng một giá trị nhóm.
- Kết quả một dòng tới 5 cột: code kể dòng đó bằng lời, tức là các ô của dòng, mỗi ô cắt ở 80 ký tự. Chỉ gửi khi mọi ô
  là số tổng hợp hay giá trị của cột phân loại, cột thời gian không nhạy cảm.
- Không gọi mô hình khi: nhóm theo cột nhạy cảm, cột khóa hay cột chữ tự do; kết quả là danh sách dòng; kết quả một
  dòng có ô không gửi được; `INSIGHT_TEXT=false`. Khi đó lời là câu của code.
- Lời trượt thì viết lại một lần, như câu hỏi nguyên nhân.
- Không gửi dòng của bảng kết quả (ngoài kết quả một dòng ở trên), dòng của bảng phân rã, câu SQL.

### Kiểm nghĩa

- Một lần gọi riêng sau khi lời qua kiểm số, không kèm câu hỏi hay prompt viết lời.
- Câu hỏi nguyên nhân: lời vừa viết; tên độ đo, kiểu so sánh, nhãn hai bên, tên lượng và giá, cờ quy mô là ý chính; các
  khẳng định của lời (số, tên cột, giá trị nhóm đã khớp bảng phân rã).
- Câu hỏi số liệu: lời vừa viết và các câu code đã viết, như lượt viết lời.

### Báo cáo tự động

- Câu SQL do code dựng, không có lượt phân loại hay viết SQL.
- Mỗi phần (độ đo theo thời gian, theo từng cột chiều) gửi như lượt viết lời câu hỏi số liệu, với câu hỏi là tiêu đề do
  code dựng ("Doanh thu theo khu vực"), không kèm bài học, rồi kiểm nghĩa. Thường hai lần gọi mỗi phần, bốn khi phải viết
  lại.

## Lưu ý về cột nhạy cảm

Giá trị của cột phân loại là dữ liệu thật, ví dụ tên khu vực hay tên sản phẩm. Lúc nạp, backend đánh dấu cột nhạy cảm
theo tên cột (email, mật khẩu, số điện thoại, địa chỉ, họ tên) và theo giá trị có dạng email hay số điện thoại; lúc lập
hồ sơ ngữ nghĩa, mô hình có thể đánh dấu thêm. Một cột không khớp luật (ví dụ tên khách hàng đặt tên khác thường) có
thể bị gửi giá trị cho tới khi được đánh dấu. Chưa có màn để admin đánh dấu tay.

## Xóa dữ liệu

- Cả bản cài compose: `docker compose down -v` xóa hai volume (`pgdata` của Postgres, `xdidata` chứa file dữ liệu).
  Không có `-v` thì giữ dữ liệu.
- Workspace: admin xóa ở trang Thành viên (hay `DELETE /api/workspaces/{ws}`). Mọi bảng, hồ sơ, chỉ số, lịch sử câu hỏi,
  báo cáo, vai dữ liệu và thành viên của workspace mất theo, cùng thư mục Parquet của nó.
- Một bảng: nút Xóa của bảng ở trang Dữ liệu (hay `DELETE /api/workspaces/{ws}/datasets/{id}`). Xóa mọi phiên bản và
  file Parquet, hồ sơ cột, chỉ số, bài học, cuộc trò chuyện và lịch sử câu hỏi của bảng, kèm các mục báo cáo trỏ tới
  những câu đó. Nhật ký lỗi giữ lại, không còn trỏ tới bảng. Câu hỏi bắt đầu ở bảng khác mà có đọc bảng này vẫn giữ.
- Cả một database: xóa hết bảng của nó, rồi xóa nguồn ở khung Kết nối database; nguồn còn bảng thì không xóa được. Báo
  cáo tự động của database đó mất theo.
- Tài khoản: báo cáo và cuộc trò chuyện của người đó mất theo.
- Bản sao lưu (`make backup`) làm trước khi xóa vẫn còn dữ liệu cũ; xóa hẳn thì xóa cả các bản sao lưu đó.

## Bản demo công khai (chưa có)

Chế độ demo chưa có. Thiết kế dự kiến: mỗi phiên trình duyệt một workspace tạm, tự xóa sau 24 giờ; workspace mẫu chỉ
đọc; nội dung ở mục [Gửi gì cho mô hình](#gửi-gì-cho-mô-hình) đi tới nhà cung cấp mô hình mà bản demo dùng. Đừng nạp
dữ liệu nhạy cảm lên bản demo của người khác.
