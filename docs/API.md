# API

API của backend là REST và JSON, nằm dưới `/api`, cùng địa chỉ với giao diện. Tài liệu này giải thích những gì chung
cho mọi endpoint: đặc tả ở đâu, đăng nhập thế nào, workspace và vai, lỗi trả về dạng gì. Danh sách endpoint, tham số
và kiểu dữ liệu nằm trong đặc tả OpenAPI, không chép lại ở đây.

Đường dẫn trong tài liệu này viết đầy đủ, có `/api`. Trong đặc tả, đường dẫn tính từ địa chỉ gốc `/api`: `/auth/login`
trong đặc tả là `/api/auth/login`.

## Xem đặc tả

| Địa chỉ | Nội dung |
|---|---|
| `/api/docs` | Swagger UI: xem và gọi thử từng endpoint trên trình duyệt |
| `/api/openapi.json` | Đặc tả OpenAPI 3.1 dạng JSON |
| `openapi.json` ở gốc repo | Cùng nội dung với `/api/openapi.json`, để đọc mà không cần chạy server |

Xem đặc tả không cần đăng nhập. Gọi thử endpoint trong Swagger UI thì cần cookie phiên như giao diện: đăng nhập ở giao
diện trên cùng địa chỉ trước, hoặc gọi `POST /api/auth/login` ngay trong Swagger UI.

Đặc tả sinh từ code (`utoipa`): route có trong router thì có trong đặc tả. Mô tả trong đặc tả viết tiếng Anh vì người
đọc là người dùng API. Cách đặc tả được dựng và vì sao Swagger UI kèm sẵn trong binary: [Kiến trúc](ARCHITECTURE.md),
mục 8.

## Đăng nhập và phiên

- `POST /api/auth/login` (hoặc `POST /api/auth/setup` ở lần chạy đầu) đặt cookie `xdi_session`. Cookie có `HttpOnly`,
  `SameSite=Lax`, `Path=/`, và `Secure` khi `PUBLIC_URL` bắt đầu bằng `https://`.
- Phiên lưu trong Postgres (bảng giữ SHA-256 của mã phiên), hết hạn sau 30 ngày kể từ lúc đăng nhập. Đăng xuất hay bị
  khóa tài khoản thì phiên hết ngay; đổi mật khẩu thì các phiên khác của tài khoản bị đăng xuất, phiên đang dùng
  giữ lại.
- Không có JWT hay khóa API. Script gọi API dùng cookie như trình duyệt; ví dụ bằng `curl` ở
  [Cài đặt](INSTALL.md), mục Nạp dữ liệu.
- Sai email và sai mật khẩu nhận cùng một lời báo 401, để người ngoài không dò được email nào có tài khoản.
- Thao tác công khai (không cần cookie): `GET /api/health`, `GET /api/version`, `GET` và `POST /api/auth/setup`,
  `POST /api/auth/login`, `POST /api/auth/logout`, `POST /api/auth/demo-login` (chưa làm, trả 501). Mọi thao tác khác
  thiếu cookie hợp lệ thì trả 401 trước khi đọc body. Test `every_session_operation_requires_login`
  (`backend/src/api/mod.rs`) duyệt đặc tả và gọi từng thao tác không cookie, nên route mới quên kiểm đăng nhập sẽ làm
  test lỗi.

## Workspace trong đường dẫn

API dữ liệu nằm dưới workspace: `/api/workspaces/{ws}/...`, trong đó `{ws}` là slug của workspace (trường `slug` trong
kết quả `GET /api/auth/me` hoặc `GET /api/workspaces`). Workspace đang làm việc không lưu trong phiên, nên mỗi tab trình
duyệt làm việc độc lập.

Mỗi request dưới `/api/workspaces/{ws}/` kiểm hai việc: đã đăng nhập (không thì 401), và người gọi là thành viên
workspace đó (không thì 404). Workspace không tồn tại và workspace mình không thuộc nhận cùng một lời báo 404, để
không dò được tên workspace. Id dataset của workspace khác cũng trả 404 như id không tồn tại.

## Vai

| Vai | Thuộc về | Gọi việc không đủ quyền |
|---|---|---|
| `admin`, `member` | Một tài khoản trong một workspace (trường `role` của `Membership`) | `member` gọi việc của admin (nạp file, liệt kê sheet, xóa dataset, đổi tên hay xóa workspace, thêm, đổi vai hay xóa người khác, xem, duyệt hay xuất câu mẫu, xem nhật ký lỗi, viết hay bật bài học): 403 |
| Quản trị hệ thống | Tài khoản (`is_system_admin`), không theo workspace | Người khác gọi `/api/admin/...`: 403 |

Thành viên thường nhận 403 thay vì 404 vì họ đã biết workspace có thật, chỉ thiếu quyền. Quản trị hệ thống không tự có
quyền vào dữ liệu của workspace mình không thuộc: gọi vào đó nhận 404 như mọi người ngoài. Bảng quyền đầy đủ:
[Kiến trúc](ARCHITECTURE.md), mục 4b.

## Lỗi

Lỗi trả cùng một dạng body (`ErrorBody`), trừ hai ngoại lệ ở cuối mục này:

```json
{"code": "not_found", "message": "Không có dataset này"}
```

- `code`: mã tiếng Anh, ổn định, mỗi mã ứng với một mã HTTP. Client rẽ nhánh theo `code` hoặc mã HTTP, không theo
  `message`.
- `message`: lời giải thích tiếng Việt cho người dùng cuối, hiện nguyên văn được. Riêng lỗi 500 chỉ có câu chung, chi
  tiết nằm trong log của backend.
- `field`: chỉ có khi lỗi 400 nằm ở một trường của body (hiện chỉ API chỉ số), là tên trường JSON đó, ví dụ
  `expression`; giao diện hiện lời báo ngay dưới ô tương ứng. Lỗi khác không có trường này.

| `code` | HTTP | Khi nào |
|---|---|---|
| `bad_request` | 400 | Request sai: body không phải JSON, thiếu trường hay sai kiểu, thiếu header `Content-Type: application/json`, file nạp lên không đọc được, giá trị không hợp lệ |
| `unauthorized` | 401 | Chưa đăng nhập, phiên hết hạn, sai email hoặc mật khẩu |
| `forbidden` | 403 | Đã đăng nhập nhưng không đủ quyền (mục Vai); sai mã thiết lập hoặc bản cài đã thiết lập |
| `not_found` | 404 | Đường dẫn dưới `/api` không tồn tại, workspace mình không thuộc, dataset không có; tham số đường dẫn không đọc được (ví dụ `%FF`) |
| `too_large` | 413 | Tổng dung lượng một lần nạp vượt `MAX_UPLOAD_MB`; file chép ra khi chụp bảng vượt `SOURCE_TABLE_MAX_MB`; file Excel vượt trần kích thước giải nén, bảng chuỗi dùng chung hay dung lượng CSV ghi ra ([Dữ liệu](DATA.md)) |
| `internal` | 500 | Lỗi phía máy chủ, ví dụ mất kết nối Postgres |
| `not_implemented` | 501 | Route đã có trong đặc tả nhưng chưa cài đặt: đăng nhập demo |
| `upstream` | 502 | Dịch vụ mô hình ngôn ngữ lỗi, quá tải hoặc không trả lời |
| `unavailable` | 503 | Máy chủ thiếu cấu hình cho mô hình ngôn ngữ, ví dụ chưa đặt khóa API |

Hai mã 502 và 503 do nhà cung cấp mô hình trả (`backend/src/engine/llm/`); route hỏi dữ liệu trả chúng khi mô hình
lỗi hay thiếu cấu hình trước khi có câu SQL nào ([#33](https://github.com/pgiahuy/xdi-platform/issues/33)).

Trong đặc tả, phần lớn thao tác khai lỗi phía client gộp là `4XX`, phần mô tả ghi mã cụ thể thao tác đó trả (ví dụ
"401, 403 not admin, 404 not a member"); `GET /api/auth/me` và `GET /api/workspaces` khai riêng `401`; các thao tác
công khai như `health`, `version`, `logout` không khai lỗi phía client. Lỗi 500 được thêm cho mọi thao tác chạm database
hay file.

Hai ngoại lệ trả body rỗng, không có `ErrorBody`:

- Đường dẫn có thật gọi bằng phương thức khác, ví dụ `GET /api/auth/login`: 405, header `Allow` ghi phương thức đúng.
  Trả cả khi chưa đăng nhập, vì router từ chối trước khi tới handler. Router API (`backend/src/api/mod.rs`) chỉ đặt
  fallback cho đường dẫn không tồn tại, không đặt cho phương thức sai.
- Đường dẫn dưới `/api/docs/` không tồn tại, ví dụ `/api/docs/nope`: 404, vì thuộc phần Swagger UI.

Enum lỗi nằm ở `backend/src/error.rs`.

## Dataset và bảng

Một dataset là một database đã kết nối hay một file đã nạp; bảng nằm bên trong dataset (#257). File có đúng một bảng;
database có mọi bảng đã chụp từ nó. Phiên bản, hồ sơ cột, chỉ số của bảng, câu hỏi và bằng chứng vẫn theo từng bảng.
Code: `backend/src/service/catalog/` (hợp đồng ở `contract.rs`), `backend/src/api/catalog.rs`.

| Route | Ai gọi | Việc |
|---|---|---|
| `GET /api/workspaces/{ws}/catalog` | mọi thành viên | Mọi dataset, cũ trước, mỗi dataset kèm bảng và quan hệ |
| `GET .../catalog/{id}` | mọi thành viên | Một dataset |
| `POST .../catalog/{id}/snapshot` | admin | Chụp lại mọi bảng của dataset là database, trong một giao dịch |
| `POST .../catalog/{id}/ask` | mọi thành viên | Hỏi trên dataset; `table_id` để ghim bảng bắt đầu |
| `GET .../catalog/{id}/conversations` | mọi thành viên | Cuộc trò chuyện của người gọi trên mọi bảng của dataset |

- `DatasetSummary`: `id` (với database, trùng id nguồn ở `/sources/{id}`), `kind` (`database` hay `file`), `name` (tên
  database, hay tên đặt lúc nạp file), `domain` (gói ngành), `tables`, `relations` (quan hệ khóa ngoại giữa các bảng của
  dataset, dạng `DatasetRelation`), `metadata_only` (bảng của database mới có cột, kiểu, khóa và mô tả, chưa có dữ liệu:
  cùng dạng `MetadataOnlyTable` như response chụp, rỗng với file), `updated_at` (lần chụp hay nạp gần nhất),
  `created_at`.
- `DatasetTable`: `id` là id bảng mà các route bảng nhận (`/datasets/{id}/overview`, `/metrics`, `/drill`,
  `/conversations/{conversation_id}`, `DELETE /datasets/{id}`); `name`, `logical_table` (tên trong SQL, chỉ không trùng
  trong một dataset: hai database cùng có `orders` thì cả hai là `orders`), `schema` và `source_table` (null với file),
  `description` (comment của bảng trong database), `row_count`, phiên bản hiện hành, `current_profile_status`,
  `semantic_unavailable`, `suggested_questions`, và `columns` của phiên bản hiện hành (`name`, `data_type`, `role`,
  `description`, `description_source`, `is_sensitive`, `aliases`). Bảng xếp theo `source_table` rồi theo tên, để giao
  diện nhóm theo schema. Database đã kết nối mà chưa chụp bảng nào có `tables` rỗng.
- Hỏi trên dataset nhận `question`, `choices`, `conversation_id` như route hỏi của bảng, thêm `table_id`. Không có
  `table_id` thì backend chọn bảng bắt đầu (`backend/src/service/ask/scope.rs`): bảng giữ cột số đo câu hỏi nhắc
  tới; rồi bảng có nhiều cụm từ của câu hỏi trùng tên bảng, tên cột hay bí danh cột nhất; rồi trùng mô tả; rồi bảng của
  cuộc đang tiếp; rồi bảng giữ nhiều khóa ngoại nhất; rồi bảng nhiều dòng hơn. Câu hỏi đọc được mọi bảng đã chụp của
  dataset (#249, [ARCHITECTURE.md](ARCHITECTURE.md) mục 6). `table_id`
  không thuộc dataset trả 404. Response là `AskResponse` như route hỏi của bảng; `AskResponse.table_id` là bảng đã bắt
  đầu, dùng cho truy dòng nguồn. Cuộc trò chuyện nằm trên bảng bắt đầu của câu đầu (`ConversationItem.table_id`) và
  tiếp được từ bất kỳ bảng nào của cùng dataset.
- Chụp lại dataset: mọi bảng dataset đang giữ, gồm bảng chỉ có metadata, không gồm bảng admin bỏ tick; không giới hạn
  số bảng, trần dòng và dung lượng như lần chụp đầu (mục Nguồn database); response `SnapshotResponse` như ở đó, bảng
  không còn đọc được ghi trong `unreadable`. Dataset là file trả 400. 201 khi có bảng đổi, 200 khi không bảng nào đổi.

Route cũ vẫn chạy, đặc tả đánh dấu `deprecated`: `GET .../datasets` (mỗi mục là một bảng, `parent_id` là dataset của
nó), `POST .../datasets/{id}/ask` (thay bằng hỏi trên dataset kèm `table_id`), `GET .../datasets/{id}/conversations`.
Tên `dataset` trong các route và kiểu cũ (`Dataset`, `dataset_id`, `UploadResult.dataset`, `SnapshotResponse.datasets`)
nghĩa là bảng.

## Hỏi dữ liệu

`POST /api/workspaces/{ws}/datasets/{id}/ask` (id là id bảng; hỏi trên cả dataset ở mục trên) trả lời một câu hỏi trên
phiên bản đang dùng của bảng (`backend/src/service/ask/mod.rs`): lấy ngữ cảnh từ hồ sơ cột, nhờ mô hình viết SQL, guard
kiểm, DuckDB chạy, sửa một lần khi lỗi, ghi lịch sử câu hỏi. Câu không trả lời được (guard từ chối, câu mơ hồ, SQL chạy
lỗi) vẫn là 200 với `status` trong body, không phải mã lỗi. Mã lỗi: 400 câu hỏi rỗng, body sai, dataset chưa có dữ liệu,
hồ sơ chưa lập xong hay lập lỗi, Gemini từ chối vì nội dung câu hỏi, hoặc câu mock không có câu trả lời mẫu; 404 dataset
không có; 502, 503 khi mô hình lỗi hay chưa cấu hình trước khi có câu SQL nào.

Câu answered có thẻ "Hệ thống đã hiểu là", vai trò cột và gợi ý biểu đồ. Câu thường answered có ít nhất một dòng kết quả
thì có `answer_text`: đoạn trả lời ngắn tiếng Việt. Đó là `insight.summary` khi lời này do mô hình viết và đã qua bước
kiểm (mô hình chỉ nhận các câu backend đã tính); còn lại là câu khuôn do backend viết: một ô ("Tổng doanh thu là khoảng
36,8 triệu (36.784.735,01)."), nhóm đứng đầu khi cột số đo xếp giảm dần ("Caguas có tổng doanh thu cao nhất: ..., tiếp
theo là Chicago và Brooklyn."), danh sách dòng không gộp ("Có 37 dòng khớp điều kiện: ..."). Kết quả một dòng, tới 5
cột, mà không phải một con số đơn (`insight.kind` là `rows`) dùng lời của `insight` kể dòng đó: lời của mô hình khi qua
kiểm, không thì câu của code, như "New York City có tổng doanh thu cao nhất: 436.061,54.". Số từ một triệu trở lên có
dạng rút gọn đứng trước số đủ. `answer_text_by_model` cho biết đoạn đó do mô hình
viết (`true`) hay theo khuôn (`false`). Hai trường null với câu "tại sao" (đã có `explanation.summary`) và câu không
answered. Đoạn trả lời không có lần gọi mô hình hay bước riêng trong `steps`. `ANSWER_TEXT=false` thì luôn là câu khuôn.

Câu answered (câu thường và câu "tại sao") có `data_range` khi bảng có cột thời gian dùng được: `from`, `to` (YYYY-MM-DD)
là ngày đầu, ngày cuối của cột đó trong hồ sơ lúc nạp, `column` và `label` (bí danh đầu, null khi không có) là cột đã
dùng: cột thời gian đầu tiên có tên trong câu SQL đã chạy, không có thì cột thời gian đầu tiên của bảng; cột nhạy cảm
không dùng. `partial` là `true` khi câu SQL có mốc ngày (dạng `'YYYY-MM-DD'`) mà ngày liền trước mốc đó vẫn sau ngày
cuối của dữ liệu: kỳ được hỏi chưa đủ dữ liệu. Ngày lấy từ hồ sơ của cả bảng, không theo vai dữ liệu của người hỏi.
`data_range` null với câu không answered, hay khi không có cột thời gian dùng được.

Câu thường answered có ít nhất một ô có giá trị thì có `insight`: phần giải thích chọn theo hình dạng kết quả (`kind`:
`single_value`, `time_series`, `groups`, `top_n`, `rows`), gồm `title` và `points` (`lead`, `text`) do backend viết từ
số đã tính, `summary` là lời của mô hình khi `summary_by_model`, không thì là câu chính nối các ý. Lời của mô hình qua
hai cổng (#201): code kiểm số, chiều tăng giảm và từ chỉ nguyên nhân; rồi một lần gọi mô hình chấm riêng so nghĩa của
lời với `title` và `points`. Trượt hay có chỗ chưa rõ thì mô hình viết lại một lần kèm các lỗi đó; vẫn trượt thì dùng
câu của code. `summary_verdict` là kết luận: `match`, `warning` (lời vẫn dùng, các chỗ chưa rõ ở `summary_warnings`),
`mismatch` (lời trượt mọi lần viết, `summary` là câu của code), null khi không cổng nào chạy xong; và `notable` là
`false` khi số liệu không cho thấy điều gì đáng kể (chênh lệch dưới 2%, một kỳ dưới 30 dòng, các nhóm khá đều, hay danh
sách dòng). Với `single_value` so được kỳ trước và `time_series`, `explanation` là phân rã của kỳ đổi mạnh nhất so với
kỳ liền trước (cùng hình dạng như câu "tại sao", lưu cùng câu hỏi nên truy dòng nguồn của một dòng phân rã như câu "tại
sao"); chênh lệch không đáng kể thì `explanation` null. Hai bước thêm trong `steps`: `explain` (phần do code tính) và
`narrate` (lời). `insight` null với câu "tại sao" và câu không answered. `INSIGHT_TEXT=false` thì không gọi mô hình cho
phần này.

Câu hỏi cần thông tin không cột nào có (mức hài lòng khi không có cột đánh giá, dự báo tương lai) thì mô hình báo
`unanswerable`, không câu SQL nào chạy, `status` là `rejected` với `sql_generated` null và `message` nêu lý do cùng các
cột gần nhất có thể hỏi; câu vẫn ghi vào lịch sử như câu bị từ chối.

Câu "tại sao" (nhận bằng từ khóa) đi đường form: mô hình điền form, code dựng SQL, `result` là bảng tổng theo kỳ hay
theo nhóm, `explanation` là bảng phân rã; không phân rã được thì `explanation` null kèm cảnh báo
`explanation_unavailable`, câu vẫn answered.

- Câu nói một hướng ("vì sao giảm", "vì sao cao hơn") mà chênh lệch tính được ngược hướng hay bằng 0 thì kèm cảnh báo
  `premise_mismatch` nêu chênh lệch thực tế; phân rã và lời giải thích vẫn trả như thường, theo chênh lệch thực tế.
- Câu so hai kỳ không nêu kỳ thì code chọn kỳ gần nhất có dữ liệu (kỳ cuối có chưa tới một nửa số dòng của kỳ trước nó
  thì lùi một kỳ) và ghi thành giả định `why:compare` trong `understanding.assumptions`, phương án là kỳ viết như
  `2017-Q4`; gửi lại `{choice_id: "why:compare", option_id: "2017-Q3"}` để giải thích kỳ khác.
- `explanation.measure_kind` cho biết cách đọc số: `total` (tổng, đếm; đóng góp là hiệu hai lát cắt) hay `ratio` (chỉ
  số tỷ lệ hay trung bình; giá trị là tỷ lệ, đóng góp của nhóm là `mix_effect + rate_effect`, mỗi chiều có tổng
  `mix_effect`, `rate_effect`). Với `total` bốn trường này là null.
- `explanation.comparison` là `periods` (hai kỳ) hay `groups` (hai nhóm cùng lúc). Với `groups`, `slice_column` và
  `slice_column_label` là cột của hai nhóm và nghĩa tiếng Việt của nó.
- `explanation.summary_title` và `summary_points` là insight: câu chính và các ý, mỗi ý có `kind` (`reason`,
  `outliers`, `even`, `largest`, `against`, `dropped`), `lead` (đầu ý, in đậm) và `text`. Khi `summary_by_model`, mô
  hình viết cả hai từ số backend đã tính và insight đã qua hai cổng kiểm (#201); mọi ý của mô hình có `kind` là
  `reason`. Không thì backend viết theo khuôn. `summary` là câu chính nối các ý thành một đoạn.
- `explanation.summary_warnings` là các chỗ trong insight của mô hình mà mô hình chấm (cổng 2, #201) thấy đọc được
  nhiều nghĩa nhưng không chắc sai, mỗi chỗ một câu tiếng Việt do mô hình chấm viết; hiện cùng insight. Trống khi
  không có chỗ nào như vậy hay insight là câu khuôn.
- `explanation.summary_verdict` là kết luận của hai cổng cho insight của mô hình: `match`, `warning` (insight vẫn dùng,
  các chỗ chưa rõ ở `summary_warnings`), `mismatch` (insight trượt mọi lần viết, `summary` là câu khuôn), null khi không
  cổng nào chạy xong (chênh lệch bằng 0, mô hình lỗi hay quá giờ). Tab Các bước hiện kết luận này thành cờ màu ở bước
  `narrate`.
- `question_type` (ngoài `explanation`) là loại câu bước phân loại chọn: `quantitative` (câu thường) hay `causal` (câu
  "tại sao"); null khi câu dừng trước bước phân loại.
- `explanation.base.rows`, `compare.rows` là số dòng của mỗi lát cắt; `percent_change` là % thay đổi chung (null khi
  lát gốc không dương); `counts_rows` cho biết độ đo là phép đếm.
- Mỗi chiều có `label`: nghĩa tiếng Việt của cột, null khi hồ sơ chưa có. `dimensions` vẫn đủ mọi chiều theo thứ tự
  xếp hạng, kể cả chiều mà ý `dropped` nói là bỏ qua.

Lý do và nghĩa từng trường: [Kiến trúc](ARCHITECTURE.md), mục 8, phần Hỏi dữ liệu. Năm response mẫu nằm trong đặc tả
(example của response 200) và ở `frontend/src/api/examples/ask.json`. Guard kiểm câu SQL trước khi chạy:
[Guard SQL](GUARD.md).

## Cuộc trò chuyện

Câu hỏi thuộc một cuộc trò chuyện trên một dataset (`backend/src/service/conversation/`). `AskRequest.conversation_id`
vắng thì backend mở cuộc mới; `AskResponse.conversation_id` luôn có, gửi lại ở câu sau để mô hình đọc câu đó như câu
tiếp nối của tối đa 4 lượt gần nhất (phần gửi cho mô hình: [Dữ liệu và quyền riêng tư](PRIVACY.md)). Câu `needs_choice`
không thành lượt, nhưng gửi lại kèm lựa chọn và cùng `conversation_id` thì vào đúng cuộc.

Ở câu tiếp nối, mục của `understanding` (độ đo, bộ lọc, nhóm, `time_range`) có `from_previous: true` khi nó giống một
mục của lượt đã trả lời gần nhất (đoạn SQL có nguyên văn trong câu SQL lượt đó, hay cùng nhãn) và câu hỏi không nhắc
tới nó (không chứa tên cột, giá trị, năm trong đoạn SQL hay tên chỉ số). Trường vắng khi false, kể cả khi không đọc
được lượt trước.

`GET /api/workspaces/{ws}/datasets/{id}/conversations` liệt kê tối đa 50 cuộc mới nhất của người gọi trên dataset, bỏ
cuộc chưa có lượt nào. `GET .../conversations/{conversation_id}` trả cuộc đó kèm tối đa 200 lượt gần nhất theo thứ tự
hỏi: `query_id`, câu hỏi, `status`, `sql` (sau guard), `understood` (phần "Hệ thống đã hiểu là" dạng chữ) và `answer`:
lời trả lời đã hiện lúc hỏi (`text`; `by_model`; `verdict` của mô hình chấm, null khi không chạy; `warnings`, các chỗ chưa
rõ của lời do mô hình viết), tức `answer_text` của câu thường hay `explanation.summary` của câu "tại sao"; null với câu
không answered và câu ghi trước khi có trường này (#201). Bảng kết quả không lưu; hỏi lại câu đó để xem bảng và biểu
đồ. Chỉ người tạo đọc và hỏi tiếp được một cuộc, admin của workspace cũng không: cuộc
của người khác, của dataset khác hay id không có đều trả 404.

## Đánh giá câu trả lời và câu mẫu

Người hỏi đánh giá câu trả lời của mình (`backend/src/service/feedback/`), mọi vai:

- `PUT /api/workspaces/{ws}/queries/{query_id}/feedback` với `{rating, reason, note}`: `rating` là `good` (nút "Tốt")
  hay `bad` ("Chưa đạt"); `reason` (`misunderstood`, `wrong_numbers`, `unclear_explanation`, `other`) và `note` (tối đa
  500 ký tự) chỉ đi với `bad`, đều không bắt buộc. Gửi lại thì thay đánh giá cũ. Câu của người khác, kể cả với admin,
  trả 404 như cuộc trò chuyện.
- `DELETE` cùng đường dẫn bỏ đánh giá (204).
- Mỗi lượt của `GET .../conversations/{conversation_id}` có `feedback` (null khi chưa đánh giá), để mở lại cuộc thì nút
  đã bấm hiện lại.

Chỉ admin, `member` nhận 403:

- `GET /api/workspaces/{ws}/examples`: tối đa 200 câu đã trả lời được đánh giá `good` của mọi thành viên, mới đánh giá
  trước, kèm SQL mô hình viết, phần "Hệ thống đã hiểu là", số dòng kết quả, ai đánh giá, ai duyệt và lúc nào.
- `PUT /api/workspaces/{ws}/examples/{query_id}/approval` duyệt câu làm câu mẫu, ghi người duyệt và thời điểm (204);
  `DELETE` bỏ duyệt. Câu chưa đánh giá `good` hay không trả lời được trả 404. Người hỏi đổi sang `bad` hay bỏ đánh giá
  thì lần duyệt mất theo.
- `GET /api/workspaces/{ws}/datasets/{id}/examples/export`: câu mẫu đã duyệt của dataset dạng mảng JSON cùng khuôn với
  file ca trong `eval/` (`id`, `question`, `expected_sql`, `ordered` luôn false). Người viết bộ câu xem lại từng ca,
  sửa `ordered` khi thứ tự dòng là một phần câu trả lời, rồi mới chép vào file ca.

Câu mẫu đã duyệt vào prompt của câu hỏi sau trên cùng dataset có chung ít nhất một từ (không tính từ dừng), tối đa 3
câu, mới nhất trước: câu thường góp câu hỏi và SQL mô hình viết (không phải bản sau guard), câu "tại sao" góp câu hỏi
và form vào prompt form. Bước `llm` trong `steps` khi đó có `detail` "Prompt có N câu mẫu đã duyệt: ...".

## Nhật ký lỗi và bài học

Chỉ admin, `member` nhận 403 (`backend/src/service/lessons/`):

- `GET /api/workspaces/{ws}/error-log`: tối đa 200 dòng mới nhất. Mỗi dòng có câu hỏi, dataset, `step` (`guard`,
  `run`, `fanout`, `why_form`, `narrate`, `feedback`), lời báo lỗi, `model_output` (câu SQL, form JSON hay lời mô hình
  viết ở bước đó), `query_id` (null khi câu chưa được ghi, ví dụ còn chờ chọn cách hiểu). Ghi tự động trong luồng hỏi;
  lỗi ghi nhật ký chỉ vào log máy chủ, không làm câu hỏi lỗi. Dòng `feedback` ghi lý do và ghi chú của "Chưa đạt".
- `GET /api/workspaces/{ws}/lessons`: mọi bài học, mới trước.
- `POST /api/workspaces/{ws}/lessons` với `{text, dataset_id, error_id}`: tạo bài nháp (201). `text` tối đa 300 ký tự;
  `dataset_id` vắng là bài của cả workspace; `error_id` là dòng nhật ký bài viết từ đó. Dataset hay dòng không thuộc
  workspace trả 400.
- `PATCH /api/workspaces/{ws}/lessons/{lesson_id}` với `{enabled}`: bật là duyệt, ghi `approved_by`, `approved_at`; tắt
  giữ lần duyệt cũ. `DELETE` cùng đường dẫn xóa (204).

Bài đã bật vào prompt của câu hỏi trên cùng dataset (hay mọi dataset với bài của cả workspace) khi có chung một từ với
câu hỏi hay nhắc tên một cột câu hỏi khớp, tối đa 3 bài, trong một khối riêng; bước `llm` khi đó có `detail` "Prompt
có N bài học đã duyệt".

## Chỉ số và cách gộp

Chỉ số của một dataset (`backend/src/service/metrics/`): `GET /api/workspaces/{ws}/datasets/{id}/metrics` trả
`metrics` (mỗi tên một mục: bản đã xác nhận mới nhất, kèm `version`, `origin`) và `columns` (cột của phiên bản hiện
hành, `aggregation` là `sum` hay `avg` với cột `measure`, null với cột khác); mọi thành viên gọi được. Chỉ admin, `member`
nhận 403:

- `POST .../metrics` tạo chỉ số (201), `origin` `ui`, đã xác nhận. Tên trùng một chỉ số có sẵn (không phân biệt hoa
  thường và dấu thanh) trả 400 với `field` `name`.
- `PUT .../metrics/{metric_id}` lưu phiên bản mới (`version` + 1) cùng tên, bản cũ giữ lại; đổi tên trả 400.
  `metric_id` là id của bất kỳ phiên bản nào của chỉ số đó.
- `DELETE .../metrics/{metric_id}` xóa chỉ số cùng mọi phiên bản (204). Câu hỏi cũ giữ câu SQL đã chạy.
- `PUT .../columns/aggregation` với `{column, aggregation}` đổi cách gộp của một cột `measure` trong phiên bản hiện hành
  (204); cột không có hay không phải `measure` trả 400 với `field` `column`.

Trước khi lưu, backend kiểm tên không có dấu ngoặc kép, công thức và điều kiện không có chú thích SQL (`--`, `/* */`),
công thức là một hàm gộp trên cột có thật, điều kiện không chứa hàm gộp, cột thời gian có thật, rồi chạy
`SELECT <công thức> FROM <bảng> [WHERE <điều kiện>]` qua guard trên phiên bản hiện hành. Sai thì 400 với `field` là
`name`, `expression`, `filter` hay `time_column`; lý do gốc của guard hay DuckDB chỉ ghi log. Chỉ số lưu xong vào
prompt của câu hỏi kế tiếp.

## Khái niệm ngành

Gói khái niệm ngành của workspace ([DOMAINS.md](DOMAINS.md), `backend/src/service/domains/`):
`GET /api/workspaces/{ws}/domains` trả `packs`, mỗi gói có `key` (giá trị của `sources.domain`), `name`,
`description` và `concepts` theo thứ tự đưa vào prompt (thứ tự của file gói, khái niệm admin thêm đứng sau), kể cả
khái niệm đang tắt. Mỗi khái niệm có `id`, `key` (id trong file gói, null với khái niệm admin thêm), `name`,
`description`, `synonyms`, `calculations`, `dimensions`, `sanity_checks`, `enabled`, `updated_at` (null khi chưa ai sửa
từ lúc chép từ file). Lần gọi đầu trong workspace chép các gói của `data/domains/` vào; mọi thành viên gọi được. Chỉ
admin, `member` nhận 403:

- `POST .../domains/{pack}/concepts` thêm khái niệm vào cuối gói (201).
- `PUT .../domains/{pack}/concepts/{concept_id}` ghi đè khái niệm (200); gửi `enabled: false` để tắt, khái niệm còn
  trong danh sách nhưng không vào prompt.

Body của cả hai: `name`, `description`, `synonyms` (ít nhất một), `calculations` (tối đa ba `{name, description}`),
`dimensions`, `sanity_checks`, `enabled` (mặc định true). Sai thì 400 với `field` là trường sai, ví dụ tên trùng
khái niệm khác của gói hay từ đồng nghĩa đã thuộc khái niệm khác; gói hay khái niệm không có trả 404. Khái niệm lưu
xong vào prompt của câu hỏi kế tiếp trên dataset gắn gói đó.

## Xóa dataset

`DELETE /api/workspaces/{ws}/datasets/{id}` xóa hẳn một dataset: mọi phiên bản, hồ sơ cột, chỉ số, cuộc trò chuyện
và lịch sử câu hỏi (bảng `queries`, kể cả các lượt trò chuyện), bằng khóa ngoại `ON DELETE CASCADE`, rồi file Parquet
trên đĩa (`delete_dataset` trong `backend/src/service/ingest.rs`). Chỉ admin của workspace, `member` nhận 403; 404 khi
dataset không có hoặc thuộc workspace khác, 204 khi xóa xong. Không khôi phục được.

## Tổng quan dataset

`GET /api/workspaces/{ws}/datasets/{id}/overview` cho mọi thành viên biết dataset có gì trước khi hỏi: số dòng, khoảng
thời gian có dữ liệu, phiên bản hiện hành; mỗi cột tên, nhãn tiếng Việt (bí danh đầu), mô tả, vai trò, đơn vị, % ô
trống, số giá trị khác nhau, và giá trị kèm số dòng của cột chia nhóm không nhạy cảm; chỉ số đã định nghĩa; biểu đồ dựng
sẵn (`charts`), mỗi cái có `result` và `chart` cùng dạng với câu trả lời của route hỏi. Không gọi mô hình ngôn ngữ.

- Số dòng theo giá trị của từng cột chia nhóm lấy từ hồ sơ cột, không chạy câu nào. Cột nhạy cảm không trả giá trị.
- Số dòng, khoảng thời gian, độ đo chính theo kỳ (`measure_over_time`) và số dòng theo kỳ (`rows_over_time`) tính bằng
  câu SQL do backend dựng, qua guard với vai dữ liệu của người xem như route hỏi; `sql` của biểu đồ là câu sau guard.
  Kỳ là ngày khi dữ liệu dài tới 92 ngày, tháng khi dài hơn. Kết quả giữ trong bộ nhớ theo phiên bản và câu, mất khi
  khởi động lại.
- Độ đo chính (`main_measure`) là chỉ số đã xác nhận đầu tiên theo tên, chưa có thì cột số đo đơn vị tiền rồi số lượng.
- Người bị lọc dòng theo vai dữ liệu nhận `summary.restricted` là true: hồ sơ đếm trên mọi dòng nên số ô trống, số giá
  trị khác nhau và số dòng theo giá trị không trả.
- `missing` liệt kê phần chưa có kèm lời cho người dùng: hồ sơ đang lập hay lỗi, nhãn và mô tả chưa viết, không có cột
  thời gian, không có độ đo chính, một câu lỗi.

Mã lỗi: 404 khi dataset không có hoặc thuộc workspace khác. Dataset chưa có dữ liệu hay đang lập hồ sơ vẫn trả 200, kèm
`missing`.

## Nguồn database

Chỉ admin của workspace; member nhận 403 ở mọi route, kể cả liệt kê. Chi tiết cách chụp: [Dữ liệu](DATA.md), mục Nguồn
database. Không có khóa mã hóa dùng được (`SOURCE_SECRET_KEY` sai dạng, hay biến trống mà file khóa tự sinh trong `DATA_DIR`
hỏng, không ghi được) thì thêm và dùng nguồn trả 503 (`unavailable`).

- `GET /api/workspaces/{ws}/sources`: nguồn của workspace (máy chủ, cổng, database, tài khoản, sslmode), không có mật
  khẩu.
- `POST /api/workspaces/{ws}/sources`: thêm nguồn Postgres (`host`, `port`, `database`, `user`, `password`, `sslmode`
  là `disable`, `prefer`, `require` hay `verify-full`, và `ssl_root_cert` là chứng chỉ CA dạng PEM, chỉ với
  `verify-full`; TLS ở [DATA.md](DATA.md#nguồn-database-postgres)).
  Backend thử kết nối trước khi lưu; máy chủ ngoài `SOURCE_ALLOWED_HOSTS`, sai mật khẩu, không tới được máy chủ đều trả
  400 kèm lời báo cách sửa. 201 khi lưu xong.
- `POST .../sources/{id}/test`: kết nối lại, trả số bảng và view đọc được.
- `GET .../sources/{id}/tables`: mọi bảng và view tài khoản đọc được (`SourceTableInfo`): `schema`, `name`, `kind`,
  `table` (`schema.table`), `estimated_rows`, `size_bytes`, `comment`, `column_count`, `references` và `referenced_by`
  (khóa ngoại ra, vào giữa các bảng đọc được), `is_referenced` (bảng khác trỏ tới nó), `over_limit`, `state`
  (`snapshotted`, `metadata_only` hay null), `dataset_id`, `reason`. Ý nghĩa từng trường ở
  [DATA.md](DATA.md#chụp-mọi-bảng-đọc-được).
- `POST .../sources/{id}/relations`: thân `{"table": "schema.table"}`. Trả `column_count` (cột đọc được của bảng),
  `many_to_one` (bảng mà bảng này trỏ tới) và `one_to_many` (bảng trỏ vào bảng này). Mỗi mục có `table` (bảng chụp kèm
  được), `constraint`, `from_table`, `from_columns`, `to_table`, `to_columns`, `column_count`, `addable` và `reason`
  (tiếng Việt, khi không chụp kèm được). Chỉ đọc `pg_catalog`, không đọc dòng dữ liệu.
- `POST .../sources/{id}/snapshot`: thân rỗng (`{}`) chụp mọi bảng đọc được; `tables` là đúng các bảng được tick (một
  tên là chụp riêng bảng đó); `table` cùng `name` đặt tên cho một bảng mới; hoặc `dataset_id` (id bảng) để chụp lại
  bảng đó cùng mọi bảng cùng nguồn nối với nó qua quan hệ (gửi kèm `tables` thì 400); chụp lại mọi bảng của database ở
  `POST .../catalog/{id}/snapshot`. Mỗi bảng thành một bảng của dataset là nguồn này; bảng đã chụp từ nguồn này thì
  thêm phiên bản vào bảng cũ. Không giới hạn số bảng. Response `SnapshotResponse`: `datasets` (mỗi bảng chép được một
  `UploadResult`, `created: false` khi bảng không đổi), `relations` (quan hệ của các bảng này, dạng `DatasetRelation`),
  `metadata_only` (bảng quá `SOURCE_TABLE_MAX_MB`, trần tổng `SOURCE_SNAPSHOT_MAX_MB` hay `SOURCE_MAX_ROWS` khi đặt:
  `table`, `cause` là `size_limit` hay `row_limit`, `reason`, `row_count`, `size_bytes`, `description`, `columns`, `dataset_id` của lần chụp trước nếu có)
  và `unreadable` (chỉ khi chụp lại cả dataset). 201 khi có ít nhất một phiên bản mới, 200 khi không bảng nào đổi; 400
  khi có bảng không đọc được; 413 khi file chép ra của một view quá `SOURCE_TABLE_MAX_MB`.
- `DELETE .../sources/{id}`: 204; 400 khi còn dataset chụp từ nguồn này.

Bảng trả qua route cũ (kiểu `Dataset`) có thêm `parent_id` (dataset chứa nó), `row_count`, `description`, `source_kind`,
`source_id`, `source_table`, `current_version_at` (lần nạp hay lần chụp
gần nhất) và `relations`: các dataset nối với nó theo khóa ngoại của database nguồn, mỗi mục có `dataset_id`, `name`,
`direction` (`many_to_one` khi dataset này giữ khóa ngoại, `one_to_many` khi dataset kia giữ), `columns` (cột khóa của
dataset này) và `related_columns`; rỗng với file nạp. Mọi thành viên đọc được trường này.

## Truy dòng nguồn

`POST /api/workspaces/{ws}/datasets/{id}/drill` trả một trang dòng nguồn của một ô trong bảng kết quả hay của một dòng
phân rã: request gửi `query_id` và đúng một trong `cell` (giá trị các cột `dimension`, `time` của dòng được bấm, theo tên
trong `result.columns`) và `breakdown` (lát cắt, cột trong dataset, giá trị nhóm của câu "tại sao"), kèm
`offset`, `limit` (mặc định 100, tối đa 1000); response có câu SQL đã chạy sau guard, các dòng (mọi cột của dataset,
kể cả `_source_file`) và `total_rows` của mọi trang. Câu chạy trên phiên bản dữ liệu của câu hỏi, qua guard với vai
dữ liệu của người đang bấm. Mã lỗi: 400 khi câu hỏi bị từ chối hay chạy lỗi, khóa không khớp cột nhóm, câu có dạng chưa
truy được (lời báo nói lý do), phiên bản đã xóa, hay `breakdown` của câu không có phân rã hoặc chiều không có trong phân
rã; 404 khi `query_id` không thuộc dataset
và workspace trong đường dẫn. Cách dựng câu: [Kiến trúc](ARCHITECTURE.md), mục 8.

## Đổi API và sinh kiểu TypeScript

Sửa handler hoặc struct request, response, rồi chạy:

```bash
make openapi
```

Lệnh này không cần Postgres, ghi lại ba file và cả ba phải commit trong cùng PR:

| File | Nội dung | Kiểm lệch |
|---|---|---|
| `openapi.json` | Đặc tả | Test `openapi_json_is_up_to_date` trong `make check-fast` |
| `frontend/src/api/examples/ask.json` | Response mẫu của ask | Test `ask_examples_json_is_up_to_date` trong `make check-fast` |
| `frontend/src/api/schema.ts` | Kiểu TypeScript sinh bằng `openapi-typescript` | `npm run api:check` trong `make check-frontend` |

Hook trong `frontend/src/api/` lấy kiểu từ `schema.ts`, không khai lại bằng tay. Mô tả cho đặc tả viết bằng chú thích
`///` tiếng Anh trên struct và handler; chú thích tiếng Việt cho đội dùng `//`, test `no_vietnamese_in_spec` báo khi
chữ tiếng Việt lọt vào đặc tả.
