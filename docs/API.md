# API

API của backend là REST và JSON, nằm dưới `/api`, cùng địa chỉ với giao diện. Trang này nói phần chung (đặc tả, đăng
nhập, workspace, vai, lỗi) và nghĩa của các route chính. Danh sách đủ của endpoint, tham số và kiểu dữ liệu nằm trong
đặc tả OpenAPI, không chép lại ở đây.

Đường dẫn trong trang này viết đầy đủ, có `/api`. Trong đặc tả, đường dẫn tính từ `/api`: `/auth/login` là
`/api/auth/login`. `...` thay cho `/api/workspaces/{ws}`.

## Xem đặc tả

| Địa chỉ | Nội dung |
|---|---|
| `/api/docs` | Swagger UI: xem và gọi thử từng endpoint trên trình duyệt |
| `/api/openapi.json` | Đặc tả OpenAPI 3.1 dạng JSON |
| `openapi.json` ở gốc repo | Cùng nội dung, đọc được mà không cần chạy server |

Xem đặc tả không cần đăng nhập. Gọi thử trong Swagger UI cần cookie phiên: đăng nhập ở giao diện cùng địa chỉ trước,
hoặc gọi `POST /api/auth/login` ngay trong Swagger UI. Đặc tả sinh từ code (`utoipa`), nên route có trong router thì
có trong đặc tả; mô tả trong đặc tả viết tiếng Anh.

Các route chỉ có trong đặc tả, không tả lại ở đây: workspace (tạo, đổi tên, xóa), thành viên (thêm, đổi vai, xóa),
đổi mật khẩu, quản trị tài khoản (`/api/admin/...`), nạp file (`POST .../datasets`, `POST .../datasets/sheets`) và
`GET .../datasets/{id}`.

## Đăng nhập và phiên

- `POST /api/auth/login` (hoặc `POST /api/auth/setup` ở lần chạy đầu) đặt cookie `xdi_session`: `HttpOnly`,
  `SameSite=Lax`, `Path=/`, và `Secure` khi `PUBLIC_URL` bắt đầu bằng `https://`.
- Phiên lưu trong Postgres (giữ SHA-256 của mã phiên), hết hạn sau 30 ngày kể từ lúc đăng nhập. Đăng xuất hay bị khóa
  tài khoản thì phiên hết ngay; đổi mật khẩu thì các phiên khác của tài khoản bị đăng xuất, phiên đang dùng giữ lại.
- Không có JWT hay khóa API. Script dùng cookie như trình duyệt; ví dụ bằng `curl` ở [Cài đặt](INSTALL.md).
- Sai email và sai mật khẩu nhận cùng một lời báo 401, để người ngoài không dò được email nào có tài khoản.
- Thao tác không cần cookie: `GET /api/health`, `GET /api/version`, `GET` và `POST /api/auth/setup`,
  `POST /api/auth/login`, `POST /api/auth/logout`, `POST /api/auth/demo-login` (chưa làm, trả 501). Mọi thao tác khác
  thiếu cookie hợp lệ trả 401 trước khi đọc body.

## Workspace và vai

API dữ liệu nằm dưới `/api/workspaces/{ws}/`, `{ws}` là slug của workspace (trường `slug` của `GET /api/auth/me` hay
`GET /api/workspaces`). Workspace đang làm việc không lưu trong phiên, nên mỗi tab trình duyệt làm việc độc lập.

Mỗi request dưới `/api/workspaces/{ws}/` kiểm đã đăng nhập (không thì 401) và người gọi là thành viên workspace đó
(không thì 404). Workspace không tồn tại và workspace mình không thuộc nhận cùng lời báo 404; id dataset hay bảng của
workspace khác cũng trả 404 như id không có.

| Vai | Thuộc về | Gọi việc không đủ quyền |
|---|---|---|
| `admin`, `member` | Một tài khoản trong một workspace (`Membership.role`) | `member` gọi việc của admin: 403 |
| Quản trị hệ thống | Tài khoản (`is_system_admin`), không theo workspace | Người khác gọi `/api/admin/...`: 403 |

Việc chỉ admin làm: nạp file, kết nối và chụp database, xóa bảng, đổi tên hay xóa workspace, quản lý thành viên, vai dữ
liệu, chỉ số, khái niệm ngành, câu mẫu, nhật ký lỗi và bài học. Thành viên nhận 403 thay vì 404 vì họ đã biết workspace
có thật. Quản trị hệ thống không tự có quyền vào workspace mình không thuộc: gọi vào đó nhận 404. Bảng quyền đầy đủ ở
[Kiến trúc](ARCHITECTURE.md), mục Workspace và phân quyền.

## Lỗi

Lỗi trả cùng một dạng body (`ErrorBody`, enum ở `backend/src/error.rs`):

```json
{"code": "not_found", "message": "Không có dataset này"}
```

- `code`: mã tiếng Anh, ổn định, mỗi mã ứng với một mã HTTP. Client rẽ nhánh theo `code` hay mã HTTP, không theo
  `message`.
- `message`: lời tiếng Việt cho người dùng cuối, hiện nguyên văn được. Lỗi 500 chỉ có câu chung, chi tiết ở log.
- `field`: tên trường JSON bị sai khi lỗi 400 nằm ở một trường của body, ví dụ `expression` (API chỉ số, khái niệm
  ngành, vai dữ liệu). Lỗi khác không có trường này.

| `code` | HTTP | Khi nào |
|---|---|---|
| `bad_request` | 400 | Body không phải JSON, thiếu trường hay sai kiểu, thiếu `Content-Type: application/json`, file không đọc được, giá trị không hợp lệ |
| `unauthorized` | 401 | Chưa đăng nhập, phiên hết hạn, sai email hoặc mật khẩu |
| `forbidden` | 403 | Không đủ quyền (mục Workspace và vai); sai mã thiết lập hay bản cài đã thiết lập |
| `not_found` | 404 | Đường dẫn dưới `/api` không có, workspace mình không thuộc, dataset hay bảng không có; tham số đường dẫn không đọc được (ví dụ `%FF`) |
| `too_large` | 413 | Một lần nạp vượt `MAX_UPLOAD_MB`; file chép ra từ một view vượt `SOURCE_TABLE_MAX_MB`; file Excel vượt trần giải nén ([Dữ liệu](DATA.md#file-excel)) |
| `internal` | 500 | Lỗi phía máy chủ, ví dụ mất kết nối Postgres |
| `not_implemented` | 501 | Route có trong đặc tả nhưng chưa làm: đăng nhập demo |
| `upstream` | 502 | Dịch vụ mô hình ngôn ngữ lỗi, quá tải hay không trả lời |
| `unavailable` | 503 | Máy chủ thiếu cấu hình: chưa đặt khóa mô hình, không có khóa mã hóa nguồn dùng được |

Route hỏi dữ liệu trả 502, 503 khi mô hình lỗi hay thiếu cấu hình trước khi có câu SQL nào. Hai ngoại lệ trả body rỗng:
đường dẫn có thật gọi sai phương thức (405, header `Allow` ghi phương thức đúng, kể cả khi chưa đăng nhập), và đường
dẫn không có dưới `/api/docs/` (404 của Swagger UI).

## Dataset và bảng

Một dataset là một database đã kết nối hay một file đã nạp; bảng nằm trong dataset. File có đúng một bảng; database có
mọi bảng đã chụp từ nó. Phiên bản, hồ sơ cột, chỉ số, câu hỏi và minh chứng theo từng bảng. Code:
`backend/src/service/catalog/` (hợp đồng ở `contract.rs`).

| Route | Ai gọi | Việc |
|---|---|---|
| `GET .../catalog` | mọi thành viên | Mọi dataset, cũ trước, mỗi dataset kèm bảng và quan hệ |
| `GET .../catalog/{id}` | mọi thành viên | Một dataset |
| `POST .../catalog/{id}/ask` | mọi thành viên | Hỏi trên dataset (mục Hỏi dữ liệu) |
| `GET .../catalog/{id}/conversations` | mọi thành viên | Cuộc trò chuyện của người gọi trên dataset |
| `POST .../catalog/{id}/snapshot` | admin | Chụp lại mọi bảng của dataset là database |
| `PUT .../catalog/{id}/domain` | admin | Chọn gói ngành của dataset (mục Khái niệm ngành) |
| `DELETE .../datasets/{id}` | admin | Xóa hẳn một bảng (`{id}` là id bảng) |

- `DatasetSummary`: `id` (với database, trùng id nguồn ở `.../sources/{id}`), `kind` (`database` hay `file`), `name`,
  `domain` (gói ngành), `tables`, `relations` (khóa ngoại giữa các bảng, `DatasetRelation`), `metadata_only` (bảng mới
  có cấu trúc, chưa có dữ liệu; rỗng với file), `suggested_questions` (câu chọn tay của database mẫu, rỗng với dataset
  khác; trang hỏi hiện khi chưa ghim bảng), `updated_at`, `created_at`.
- `DatasetTable`: `id` là id bảng mà các route `/datasets/{id}/...` nhận; `logical_table` là tên trong SQL, chỉ không
  trùng trong một dataset; `schema`, `source_table` (null với file), `description`, `row_count`, phiên bản hiện hành,
  trạng thái hồ sơ, `suggested_questions` và `columns` của phiên bản hiện hành. Bảng xếp theo `source_table` rồi theo
  tên.
- Chụp lại dataset: mọi bảng dataset đang giữ, gồm bảng chỉ có cấu trúc, không gồm bảng admin bỏ tick; cùng trần như
  lần chụp đầu. Response `SnapshotResponse` (mục Nguồn database); bảng không còn đọc được ghi ở `unreadable`. Dataset
  là file trả 400. 201 khi có bảng đổi, 200 khi không bảng nào đổi.
- Xóa bảng: mọi phiên bản, hồ sơ cột, chỉ số, cuộc trò chuyện, lịch sử câu hỏi và mục báo cáo của bảng, rồi file
  Parquet trên đĩa. Bảng của file thì dataset file mất theo. Database chỉ xóa được ở `DELETE .../sources/{id}` sau khi
  xóa hết bảng. Không khôi phục được; 204 khi xong.

Route cũ vẫn chạy, đặc tả ghi `deprecated`; trong các route và kiểu cũ (`Dataset`, `dataset_id`,
`UploadResult.dataset`, `SnapshotResponse.datasets`), chữ `dataset` nghĩa là bảng:

- `GET .../datasets`: mỗi mục là một bảng, `parent_id` là dataset chứa nó.
- `POST .../datasets/{id}/ask`: hỏi với `{id}` là id bảng, cùng response; thay bằng `.../catalog/{id}/ask` kèm `table_id`.
- `GET .../datasets/{id}/conversations`: cuộc trò chuyện theo bảng; thay bằng `.../catalog/{id}/conversations`.

## Hỏi dữ liệu

`POST .../catalog/{id}/ask` nhận `question`, `conversation_id`, `choices` và `table_id` (không bắt buộc, ghim bảng bắt
đầu; bảng không thuộc dataset trả 404). Code: `backend/src/service/ask/mod.rs`, hợp đồng ở `contract.rs`.

- Không có `table_id` thì backend chọn bảng bắt đầu (`ask/scope.rs`): bảng giữ cột số đo câu hỏi gọi đúng tên, rồi
  bảng khớp nhiều cụm từ của câu hỏi nhất (tên bảng, tên cột, bí danh), rồi khớp mô tả, bảng của cuộc đang tiếp, bảng
  giữ nhiều khóa ngoại nhất, bảng nhiều dòng hơn. `AskResponse.table_id` là bảng đã bắt đầu.
- Câu hỏi đọc được mọi bảng đã chụp của dataset. Bảng nào được gửi chi tiết cho mô hình: [Kiến trúc](ARCHITECTURE.md);
  bảng đã đọc ở `tables_read`.
- Câu không trả lời được vẫn là 200, `status` nói lý do: `answered`, `needs_choice` (chọn một phương án ở mỗi mục
  `clarifications` rồi gửi lại kèm `choices`), `rejected` (guard từ chối, hay không cột nào có thông tin câu hỏi cần),
  `failed` (SQL chạy lỗi sau một lần sửa, hay quá thời gian, bộ nhớ). Nghĩa đủ của từng trạng thái ở mô tả `AskStatus`
  trong đặc tả.
- Mã lỗi: 400 câu hỏi rỗng, body sai, bảng chưa có dữ liệu, hồ sơ chưa lập xong hay lập lỗi, nhà cung cấp mô hình từ
  chối vì nội dung câu hỏi, hay câu `mock` không có câu trả lời mẫu; 404 dataset không có; 502, 503 như mục Lỗi.
- Câu không trả lời được mà cần một bảng mới có cấu trúc thì `message` nêu tên bảng đó và cách chụp nó. SQL nối hai
  bảng không có chuỗi khóa ngoại nào thì `message` nói hai bảng không có quan hệ.
- `question_type`: `quantitative` (câu hỏi số liệu) hay `causal` (câu hỏi nguyên nhân), loại bước phân loại chọn; null
  khi câu dừng trước bước đó. Mô hình lỗi hay quá `CLASSIFY_TIMEOUT_SECS` thì dùng luật từ khóa.
- `model_calls`: mỗi lượt gọi mô hình của câu hỏi (`task`, `duration_ms`, `ok`, `input_tokens`, `output_tokens`); cách
  đo ở [Hiệu năng](PERFORMANCE.md). `steps` là các bước đã chạy kèm thời gian và chi tiết.
- Câu `answered` có `understanding` (thẻ "Cách tính": độ đo, bộ lọc, nhóm, kỳ, giả định), vai trò cột và gợi ý biểu đồ.
- `data_range` (câu `answered`, bảng có cột thời gian dùng được): `from`, `to` (YYYY-MM-DD) theo hồ sơ của cả bảng,
  `column`, `label`; `partial` là true khi câu SQL có mốc ngày mà kỳ được hỏi chưa đủ dữ liệu.

Năm response mẫu (`simple`, `why`, `needs_choice`, `rejected`, `failed`) nằm trong đặc tả và ở
`frontend/src/api/examples/ask.json`. Guard kiểm câu SQL trước khi chạy: [Guard SQL](GUARD.md).

### Câu hỏi số liệu

- `answer_text`: đoạn trả lời ngắn, có khi câu `answered` có ít nhất một dòng. Là `insight.summary` khi lời của mô hình
  qua kiểm; không thì câu khuôn của code (một ô, nhóm đứng đầu, danh sách dòng). `answer_text_by_model` cho biết lời
  của mô hình hay câu khuôn. `ANSWER_TEXT=false` thì luôn là câu khuôn. Null với câu hỏi nguyên nhân.
- `insight`: lời giải thích, có khi kết quả có ít nhất một ô có giá trị. `kind` theo hình dạng kết quả
  (`single_value`, `time_series`, `groups`, `top_n`, `rows`); `title` và `points` (`lead`, `text`) do code viết từ số
  đã tính; `summary` là lời của mô hình khi `summary_by_model`, không thì câu chính nối các ý. `notable` là false khi
  số liệu không cho thấy điều gì đáng kể (chênh lệch dưới 2%, một kỳ dưới 30 dòng, các nhóm khá đều, danh sách dòng).
  `INSIGHT_TEXT=false` thì không gọi mô hình cho phần này.
- Với `single_value` so được kỳ trước và `time_series`, `explanation` là phân rã của kỳ đổi mạnh nhất so với kỳ liền
  trước, cùng dạng như câu hỏi nguyên nhân; chênh lệch không đáng kể thì null.

### Câu hỏi nguyên nhân

Mô hình điền form, code dựng SQL. `result` là bảng tổng theo kỳ hay theo nhóm, `explanation` là bảng phân rã; không
phân rã được thì `explanation` null kèm cảnh báo `explanation_unavailable`, câu vẫn `answered`.

- `explanation.comparison`: `periods` (hai kỳ) hay `groups` (hai nhóm, cột ở `slice_column`, `slice_column_label`).
- `explanation.measure_kind`: `total` (tổng, đếm; đóng góp là hiệu hai lát cắt) hay `ratio` (tỷ lệ, trung bình; đóng
  góp là `mix_effect + rate_effect`). `volume_price` tách thay đổi thành phần lượng và phần giá khi độ đo là lượng nhân
  giá; null với độ đo khác.
- `base.rows`, `compare.rows` là số dòng mỗi lát cắt; `percent_change` là % thay đổi chung (null khi lát gốc không
  dương); `counts_rows` cho biết độ đo là phép đếm. `dimensions` đủ mọi chiều theo thứ tự xếp hạng, mỗi chiều có
  `label`; `dropped_dimensions` là chiều bị bỏ kèm lý do.
- `summary_title`, `summary_points` là lời giải thích: câu chính và các ý (`kind` là `reason`, `outliers`, `even`,
  `largest`, `against` hay `dropped`; ý của mô hình luôn là `reason`). `summary` nối chúng thành một đoạn.
- Câu nói một hướng ("vì sao giảm") mà chênh lệch thực tế ngược hướng hay bằng 0: cảnh báo `premise_mismatch`, phân rã
  vẫn theo chênh lệch thực tế.
- Câu so hai kỳ không nêu kỳ: code chọn kỳ gần nhất có dữ liệu (kỳ cuối có chưa tới một nửa số dòng của kỳ trước thì
  lùi một kỳ) và ghi giả định `why:compare`; gửi lại `{choice_id: "why:compare", option_id: "2017-Q3"}` để chọn kỳ
  khác.

### Kiểm lời của mô hình

Lời của mô hình, ở `insight` và ở `explanation`, qua hai bước: kiểm số (code so từng số, chiều tăng giảm và từ chỉ
nguyên nhân với bảng đã tính) và kiểm nghĩa (một lần gọi mô hình riêng chấm nghĩa của lời). Trượt hay có chỗ chưa rõ
thì mô hình viết lại một lần kèm các lỗi đó; vẫn trượt thì dùng câu của code. Cùng trường ở cả hai chỗ:

- `summary_verdict`: `match`, `warning` (lời vẫn dùng, chỗ chưa rõ ở `summary_warnings`), `mismatch` (dùng câu của
  code), null khi chưa bước nào chạy xong (chênh lệch bằng 0, mô hình lỗi hay quá giờ).
- `summary_warnings`: các chỗ đọc được nhiều nghĩa nhưng không chắc sai, mỗi chỗ một câu tiếng Việt; trống khi không
  có hay khi lời là câu của code.

Tab Các bước hiện kết luận thành cờ ở bước `narrate`.

## Cuộc trò chuyện

`AskRequest.conversation_id` vắng thì backend mở cuộc mới; `AskResponse.conversation_id` luôn có, gửi lại ở câu sau
để mô hình đọc câu đó như câu tiếp nối của tối đa 4 lượt gần nhất. Câu `needs_choice` không thành lượt, nhưng gửi lại
kèm lựa chọn và cùng `conversation_id` thì vào đúng cuộc. Cuộc gắn với bảng bắt đầu của câu đầu
(`ConversationItem.table_id`) và hỏi tiếp được trên mọi bảng của cùng dataset.

- Ở câu tiếp nối, mục của `understanding` có `from_previous: true` khi nó lấy từ lượt trước mà câu hỏi không nhắc lại.
- `GET .../catalog/{id}/conversations`: tối đa 50 cuộc mới nhất của người gọi, bỏ cuộc chưa có lượt nào.
- `GET .../datasets/{id}/conversations/{conversation_id}`: cuộc đó kèm tối đa 200 lượt gần nhất: `query_id`, câu hỏi,
  `status`, `sql` (sau guard), `understood` (phần "Cách tính" dạng chữ), `feedback`, và `answer` (lời trả lời đã
  hiện: `text`, `by_model`, `verdict`, `warnings`; null với câu không `answered`). Bảng kết quả không lưu; hỏi lại
  để xem bảng và biểu đồ.
- Chỉ người tạo đọc và hỏi tiếp được một cuộc, admin cũng không: cuộc của người khác, của dataset khác hay id không có
  đều trả 404.

## Truy dòng nguồn

`POST .../datasets/{id}/drill` (code: `backend/src/service/drill/`) trả một trang dòng nguồn của một con số. Request gửi
`query_id` và đúng một trong hai: `cell` (giá trị các cột `dimension`, `time` của dòng được bấm, theo tên trong
`result.columns`) hay `breakdown` (lát cắt, cột, giá trị nhóm của một dòng phân rã); kèm `offset`, `limit` (mặc định
100, tối đa 1000). Request không bao giờ gửi SQL.

- Câu chạy trên phiên bản dữ liệu của câu hỏi, qua guard với vai dữ liệu của người đang bấm.
- Response: `sql` sau guard, `result` (một trang), `total_rows` (mọi trang). Câu đọc một bảng: trang có mọi cột của
  bảng, kể cả `_source_file`. Câu nối bảng: `source_table` là bảng độ đo cộng (bên nhiều), trang có cột của nó rồi
  `joined_columns` (cột khóa nhóm và cột lọc lấy từ bảng nối).
- `measures`: mỗi độ đo của con số tính lại trên mọi dòng nguồn (`rule` là `sum`, `count` hay `other`; với `sum`,
  `row_column` là cột giữ phần của từng dòng). Bằng ô đã bấm khi người bấm thấy cùng các dòng với người đã hỏi.
- Mã lỗi: 400 khi câu bị từ chối hay chạy lỗi, khóa không khớp cột nhóm, câu có dạng chưa truy được (`UNION`,
  `HAVING`, hàm cửa sổ, `ROLLUP`, `DISTINCT`, `LIMIT`, bảng tạm hay câu con gộp nhóm; lời báo nói lý do), phiên bản đã
  xóa, hay `breakdown` không có trong phân rã; 404 khi `query_id` không thuộc bảng và workspace trong đường dẫn.

## Báo cáo

Báo cáo gom các câu trả lời đã chọn (code: `backend/src/service/report/`). Mọi thành viên tạo được; như cuộc trò
chuyện, chỉ người tạo đọc và sửa một báo cáo, admin cũng không: báo cáo của người khác hay id không có trả 404.

| Route | Việc |
|---|---|
| `GET .../reports` | Báo cáo của người gọi, sửa gần nhất trước, không kèm mục |
| `POST .../reports` | Tạo báo cáo trống: `title` tới 200 ký tự, `intro` tới 5000 ký tự |
| `PATCH .../reports/{id}`, `DELETE` | Đổi `title`, `intro`; xóa báo cáo (câu hỏi của các mục vẫn còn) |
| `POST .../reports/{id}/items` | Thêm vào cuối một câu `answered` của chính người gọi (`query_id`); tối đa 30 mục |
| `DELETE .../reports/{id}/items/{item_id}` | Bỏ một mục |
| `PUT .../reports/{id}/order` | Thứ tự mới: `item_ids` gồm đúng mọi mục, mỗi mục một lần |
| `GET .../reports/{id}` | Báo cáo kèm mọi mục dựng lại |
| `GET .../reports/{id}/xlsx` | File Excel của cùng bản dựng lại |
| `POST .../reports/automatic` | Báo cáo tự động cho một dataset (`dataset_id` là `DatasetSummary.id`) |

Thêm câu của người khác trả 404; câu không `answered` hay đã có trong báo cáo trả 400.

- Dựng lại: Postgres không lưu dòng kết quả, nên mỗi lần mở backend cho câu SQL đã lưu qua guard lại với vai dữ liệu
  của người xem và chạy trên đúng các phiên bản câu hỏi đã đọc. Mỗi mục có câu hỏi, loại câu, bảng và phiên bản
  (`is_current` false khi bảng đã có phiên bản mới hơn), `answer` đã lưu kèm `number_check` (kiểm số: `passed`,
  `replaced`, `not_needed`) và `meaning_check` (kiểm nghĩa, null khi không chạy), `facts` (câu chính và các ý code tính
  lại, không gọi mô hình), `understanding`, `sql`, `result` (tối đa 1000 dòng, `total_rows` là tổng), `chart`,
  `breakdown` (chiều đầu của phân rã, tối đa 5 nhóm), `notes` (chỗ khác lúc hỏi) và `error` khi câu không chạy lại
  được.
- Điều kiện lọc dòng hiện tại của người xem không có trong câu đã chạy lúc hỏi thì `answer` và `breakdown` đã lưu là
  null, vì chúng có thể nói về dòng người xem không còn được thấy.
- Excel: sheet `Tóm tắt` (mỗi mục một dòng: câu hỏi, lời trả lời, kiểm số, kiểm nghĩa, số dòng), mỗi mục một sheet
  `Mục N` (bảng kết quả, phân rã), sheet `Cách tính` (độ đo, bộ lọc, nhóm, kỳ, SQL, bảng và phiên bản, lúc hỏi).
- Báo cáo tự động: code dựng mọi câu SQL, mô hình không viết SQL. Độ đo chính (chỉ số đã xác nhận đầu tiên, không có
  thì cột số đo đơn vị tiền rồi số lượng) theo tháng (theo ngày khi dữ liệu ngắn), rồi theo hai cột chiều (chiều đầu
  tới 12 nhóm, chiều sau tới 30 nhóm); cột thời gian và cột chiều lấy ở bảng của độ đo hay ở bảng nó trỏ tới một bước
  khóa ngoại. Mỗi phần qua guard với vai dữ liệu của người bấm, được ghi thành một câu đã trả lời, có lời giải thích
  như câu hỏi số liệu. `report.model_calls` là số lần gọi mô hình (mỗi phần tối đa 2, 4 khi phải viết lại); `skipped`
  nói phần nào không dựng được và vì sao. 400 khi dataset không có độ đo nào hay không phần nào chạy được.
- PDF là việc của giao diện: nút Xuất PDF mở hộp in của trình duyệt với kiểu in riêng của trang báo cáo.

## Vai dữ liệu

Vai dữ liệu quyết định một `member` thấy dòng nào (code: `backend/src/service/data_roles/`). Chỉ admin, trừ
`GET .../data-roles/mine`.

| Route | Việc |
|---|---|
| `GET .../data-roles` | Mọi vai: `id`, `name`, `description`, `policies` (`table_id`, `column`, `allowed_values`), `member_ids` |
| `GET .../data-roles/mine` | Tên và mô tả các vai người gọi mang, không có luật; mọi thành viên gọi được |
| `POST .../data-roles` | Tạo vai chưa có luật (201): `name` 1 tới 60 ký tự, không trùng (không phân biệt hoa thường), `description` tới 500 ký tự |
| `PATCH .../data-roles/{role_id}`, `DELETE` | Đổi tên, mô tả; xóa vai cùng luật (vai đang gán cho ai trả 400) |
| `PUT .../data-roles/{role_id}/policies` | Thay cả bộ luật: `{policies: [{table_id, column, allowed_values}]}` |
| `PUT .../members/{user_id}/data-roles` | Thay cả bộ vai của một thành viên: `{role_ids}`, rỗng là gỡ hết (204) |
| `GET .../datasets/{id}/filter-columns` | Cột chiều của bảng kèm `label` và giá trị đã biết, cho màn đặt luật |

- Luật: `table_id` là `DatasetTable.id`; `column` là cột chiều của phiên bản hiện hành; mỗi bảng và cột một luật. Giá
  trị không có trong hồ sơ cột vẫn lưu, response có `warnings`; `allowed_values` rỗng là không thấy dòng nào của bảng.
  400 khi bảng hay cột không có, cột không phải cột chiều, hai vai trùng đúng bộ luật, hay vai đang gán mà bộ luật mới
  rỗng. Không gán được vai chưa có luật.
- Ghép luật: trong một vai các luật nối AND, giữa các vai nối OR, chỉ tính vai có điều kiện trên bảng đang đọc hay bảng
  nó trỏ tới. Không mang vai nào thì thấy mọi dòng; admin không bị lọc. Vai và luật đọc lại mỗi request.
- Điều kiện đã áp hiện trong `understanding.filters` (nhãn "Lọc theo quyền xem: ...") và trong `sql_guarded`. Cách
  guard áp luật: [Guard SQL](GUARD.md).

## Chỉ số và cách gộp

`GET .../datasets/{id}/metrics` (`{id}` là id bảng; code: `backend/src/service/metrics/`) trả `metrics` (mỗi tên bản
đã xác nhận mới nhất, kèm `version`, `origin`) và `columns` (cột của phiên bản hiện hành, `aggregation` là `sum` hay
`avg` với cột `measure`); mọi thành viên gọi được. Chỉ admin:

- `POST .../metrics` tạo chỉ số (201, `origin` `ui`, đã xác nhận). Tên trùng một chỉ số của bất kỳ bảng nào cùng
  dataset (không phân biệt hoa thường và dấu thanh) trả 400 với `field` `name`.
- `PUT .../metrics/{metric_id}` lưu phiên bản mới (`version` + 1), bản cũ giữ; đổi tên trả 400. `metric_id` là id của
  bất kỳ phiên bản nào của chỉ số.
- `DELETE .../metrics/{metric_id}` xóa chỉ số cùng mọi phiên bản (204); câu hỏi cũ giữ câu SQL đã chạy.
- `PUT .../datasets/{id}/columns/aggregation` với `{column, aggregation}` đổi cách gộp của một cột `measure` (204).

Công thức chạy thử qua guard trên phiên bản hiện hành trước khi lưu; sai thì 400 với `field` là `name`, `expression`,
`filter` hay `time_column`. Chỉ số lưu xong vào prompt của câu hỏi kế tiếp.

## Khái niệm ngành

`GET .../domains` (code: `backend/src/service/domains/`, xem [DOMAINS.md](DOMAINS.md)) trả `packs`, mỗi gói có `key`
(giá trị của `sources.domain`), `name`, `description` và `concepts` theo thứ tự đưa vào prompt, kể cả khái niệm đang
tắt. Lần gọi đầu trong workspace chép các gói của `data/domains/` vào; mọi thành viên gọi được. Chỉ admin:

- `POST .../domains/{pack}/concepts` thêm khái niệm vào cuối gói (201).
- `PUT .../domains/{pack}/concepts/{concept_id}` ghi đè khái niệm; `enabled: false` để tắt.

Body: `name`, `description`, `synonyms` (ít nhất một), `calculations` (tối đa ba `{name, description}`), `dimensions`,
`sanity_checks`, `enabled` (mặc định true). Sai thì 400 với `field`; gói hay khái niệm không có trả 404.

`PUT .../catalog/{id}/domain` với `{"domain": "retail"}` gắn gói cho dataset `{id}`; `{"domain": null}` (hay chuỗi rỗng)
bỏ gói. Chỉ admin (member 403). Trả `DatasetSummary` sau khi lưu. Khóa không có trong workspace trả 400 với `field`
`domain`; dataset không có trả 404. Câu hỏi kế tiếp trên mọi bảng của dataset nhận các khái niệm đang bật của gói.

## Phản hồi, câu mẫu, bài học

Người hỏi đánh giá câu trả lời của mình (mọi vai; câu của người khác trả 404, kể cả với admin):

- `PUT .../queries/{query_id}/feedback` với `{rating, reason, note}`: `rating` là `good` ("Tốt") hay `bad` ("Chưa
  đạt"); `reason` (`misunderstood`, `wrong_numbers`, `unclear_explanation`, `other`) và `note` (tối đa 500 ký tự) chỉ
  đi với `bad`. Gửi lại thì thay. `DELETE` cùng đường dẫn bỏ đánh giá (204).

Chỉ admin:

- `GET .../examples`: tối đa 200 câu `answered` được đánh giá `good`, mới trước, kèm SQL, phần "Cách tính", số dòng,
  người đánh giá, người duyệt.
- `PUT .../examples/{query_id}/approval` duyệt câu mẫu (204); `DELETE` bỏ duyệt. Câu chưa được đánh giá `good` trả
  404; người hỏi đổi sang `bad` hay bỏ đánh giá thì lần duyệt mất theo.
- `GET .../datasets/{id}/examples/export`: câu mẫu đã duyệt của bảng dạng mảng JSON cùng khuôn với file ca trong
  `eval/` (`ordered` luôn false, người viết bộ câu sửa khi cần).
- `GET .../error-log`: tối đa 200 dòng mới nhất; mỗi dòng có câu hỏi, bảng, `step` (`guard`, `run`, `fanout`,
  `why_form`, `narrate`, `feedback`), lời báo lỗi, `model_output`, `query_id`. Ghi tự động trong luồng hỏi.
- `GET .../lessons`, `POST .../lessons` với `{text, dataset_id, error_id}` (bài nháp, `text` tối đa 300 ký tự,
  `dataset_id` vắng là bài của cả workspace), `PATCH .../lessons/{lesson_id}` với `{enabled}` (bật là duyệt),
  `DELETE` (204).

Câu mẫu đã duyệt vào prompt của câu hỏi sau trên cùng dataset khi có chung từ với câu hỏi; bài học đã bật vào prompt
khi có chung từ hay nhắc tên một cột câu hỏi khớp. Tối đa 3 mỗi loại; bước `llm` khi đó ghi số câu, số bài đã dùng. Chi tiết gửi gì: [Dữ liệu và quyền riêng tư](PRIVACY.md).

## Tổng quan bảng

`GET .../datasets/{id}/overview` cho mọi thành viên biết bảng có gì trước khi hỏi, không gọi mô hình: số dòng, khoảng
thời gian, phiên bản hiện hành; mỗi cột tên, nhãn, mô tả, vai trò, đơn vị, % ô trống, số giá trị khác nhau, giá trị
kèm số dòng của cột chia nhóm không nhạy cảm; chỉ số; biểu đồ dựng sẵn (`charts`, cùng dạng `result` và `chart` như
câu trả lời).

- Số dòng theo giá trị lấy từ hồ sơ cột. Số dòng, khoảng thời gian, độ đo chính theo kỳ (`measure_over_time`) và số
  dòng theo kỳ (`rows_over_time`) tính bằng câu SQL do backend dựng, qua guard với vai dữ liệu của người xem. Kỳ là
  ngày khi dữ liệu dài tới 92 ngày, tháng khi dài hơn. Kết quả giữ trong bộ nhớ theo phiên bản, mất khi khởi động lại.
- Độ đo chính (`main_measure`): chỉ số đã xác nhận đầu tiên theo tên, chưa có thì cột số đo đơn vị tiền rồi số lượng.
- Người bị lọc dòng nhận `summary.restricted` true, và không có số ô trống, số giá trị khác nhau, số dòng theo giá trị
  (hồ sơ đếm trên mọi dòng).
- `missing` liệt kê phần chưa có kèm lời cho người dùng. Bảng chưa có dữ liệu hay đang lập hồ sơ vẫn trả 200.

## Nguồn database

Chỉ admin, kể cả liệt kê. Cách chụp, trần và TLS: [Dữ liệu](DATA.md#nguồn-database-postgres). Không có khóa mã hóa dùng được
(`SOURCE_SECRET_KEY` sai dạng, hay file khóa tự sinh trong `DATA_DIR` hỏng, không ghi được) thì thêm và dùng nguồn trả
503.

| Route | Việc |
|---|---|
| `GET .../sources` | Nguồn của workspace (máy chủ, cổng, database, tài khoản, sslmode), không có mật khẩu |
| `POST .../sources` | Thêm nguồn Postgres; thử kết nối trước khi lưu (201) |
| `POST .../sources/{id}/test` | Kết nối lại, trả số bảng và view đọc được |
| `GET .../sources/{id}/tables` | Mọi bảng và view tài khoản đọc được (`SourceTableInfo`) |
| `POST .../sources/{id}/snapshot` | Chụp bảng |
| `POST .../sources/{id}/relations` | Khóa ngoại quanh một bảng; giao diện không dùng |
| `DELETE .../sources/{id}` | Xóa nguồn (204); 400 khi nguồn còn bảng đã chụp |

- Thêm nguồn: `host`, `port`, `database`, `user`, `password`, `sslmode` (`disable`, `prefer`, `require`,
  `verify-full`) và `ssl_root_cert` (CA dạng PEM, chỉ với `verify-full`). Máy ngoài `SOURCE_ALLOWED_HOSTS`, sai mật
  khẩu, không tới được máy chủ đều trả 400 kèm lời báo cách sửa.
- `SourceTableInfo`: `schema`, `name`, `kind`, `table` (`schema.table`), `estimated_rows`, `size_bytes`, `comment`,
  `column_count`, `references` và `referenced_by` (khóa ngoại ra, vào), `is_referenced`, `over_limit` (`size_limit`,
  `row_limit` hay null), `state` (`snapshotted`, `metadata_only` hay null), `dataset_id`, `reason`.
- Chụp: thân rỗng (`{}`) chụp mọi bảng đọc được; `tables` là đúng các bảng được tick; `table` cùng `name` đặt tên cho
  một bảng mới; `dataset_id` (id bảng) chụp lại bảng đó cùng các bảng cùng nguồn nối với nó (không đi cùng `tables`).
  Bảng đã chụp từ nguồn này thì thêm phiên bản vào bảng cũ. Không giới hạn số bảng.
- `SnapshotResponse`: `datasets` (mỗi bảng chép được một `UploadResult`, `created: false` khi bảng không đổi),
  `relations`, `metadata_only` (bảng quá trần: `table`, `cause` là `size_limit` hay `row_limit`, `reason`,
  `row_count`, `size_bytes`, `description`, `columns`, `dataset_id` của lần chụp trước nếu có), `unreadable` (chỉ khi
  chụp lại cả dataset). 201 khi có phiên bản mới, 200 khi không bảng nào đổi; 400 khi có bảng không đọc được; 413 khi
  file chép ra của một view quá `SOURCE_TABLE_MAX_MB`.
- `relations`: thân `{"table": "schema.table"}`, trả `many_to_one` và `one_to_many` quanh bảng đó; chỉ đọc catalog,
  không đọc dòng. `GET .../tables` đã có khóa ngoại của mọi bảng.
- Kiểu `Dataset` của route cũ có thêm `parent_id`, `source_kind`, `source_id`, `source_table`, `current_version_at` và
  `relations` (bảng nối với nó, `direction` là `many_to_one` hay `one_to_many`).

## Đổi API và sinh kiểu TypeScript

Sửa handler hoặc struct request, response, rồi chạy `make openapi` (không cần Postgres). Lệnh ghi lại ba file, cả ba
commit trong cùng PR:

| File | Nội dung | Kiểm lệch |
|---|---|---|
| `openapi.json` | Đặc tả | Test `openapi_json_is_up_to_date` trong `make check-fast` |
| `frontend/src/api/examples/ask.json` | Response mẫu của hỏi dữ liệu | Test `ask_examples_json_is_up_to_date` trong `make check-fast` |
| `frontend/src/api/schema.ts` | Kiểu TypeScript sinh bằng `openapi-typescript` | `npm run api:check` trong `make check-frontend` |

Hook trong `frontend/src/api/` lấy kiểu từ `schema.ts`, không khai lại bằng tay. Mô tả cho đặc tả viết bằng chú thích
`///` tiếng Anh; chú thích tiếng Việt dùng `//`, test `no_vietnamese_in_spec` báo khi chữ tiếng Việt lọt vào đặc tả.
Test `every_session_operation_requires_login` (`backend/src/api/mod.rs`) gọi từng thao tác không cookie, nên route mới
quên kiểm đăng nhập làm test lỗi.
