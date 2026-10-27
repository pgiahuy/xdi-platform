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
| `admin`, `member` | Một tài khoản trong một workspace (trường `role` của `Membership`) | `member` gọi việc của admin (nạp file, liệt kê sheet, xóa dataset, đổi tên hay xóa workspace, thêm, đổi vai hay xóa người khác): 403 |
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

| `code` | HTTP | Khi nào |
|---|---|---|
| `bad_request` | 400 | Request sai: body không phải JSON, thiếu trường hay sai kiểu, thiếu header `Content-Type: application/json`, file nạp lên không đọc được, giá trị không hợp lệ |
| `unauthorized` | 401 | Chưa đăng nhập, phiên hết hạn, sai email hoặc mật khẩu |
| `forbidden` | 403 | Đã đăng nhập nhưng không đủ quyền (mục Vai); sai mã thiết lập hoặc bản cài đã thiết lập |
| `not_found` | 404 | Đường dẫn dưới `/api` không tồn tại, workspace mình không thuộc, dataset không có; tham số đường dẫn không đọc được (ví dụ `%FF`) |
| `too_large` | 413 | Tổng dung lượng một lần nạp vượt `MAX_UPLOAD_MB`; file Excel vượt trần kích thước giải nén, bảng chuỗi dùng chung hay dung lượng CSV ghi ra ([Dữ liệu](DATA.md)) |
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

## Hỏi dữ liệu

`POST /api/workspaces/{ws}/datasets/{id}/ask` trả lời một câu hỏi trên phiên bản đang dùng của dataset
(`backend/src/service/ask/mod.rs`): lấy ngữ cảnh từ hồ sơ cột, nhờ mô hình viết SQL, guard kiểm, DuckDB chạy, sửa một lần khi
lỗi, ghi lịch sử câu hỏi. Câu không trả lời được (guard từ chối, câu mơ hồ, SQL chạy lỗi) vẫn là 200 với `status` trong
body, không phải mã lỗi. Mã lỗi: 400 câu hỏi rỗng, body sai, dataset chưa có dữ liệu, hồ sơ chưa lập xong hay lập
lỗi, Gemini từ chối vì nội dung câu hỏi, hoặc câu mock không có câu trả lời mẫu; 404 dataset không có; 502, 503 khi mô
hình lỗi hay chưa cấu hình trước khi có câu SQL nào. Câu answered có thẻ "Đã hiểu là", vai trò cột và gợi ý biểu đồ.
Câu "tại sao" (nhận bằng từ khóa) đi đường form: mô hình
điền form, code dựng SQL, `result` là bảng tổng theo kỳ hay theo nhóm, `explanation` là bảng phân rã; không phân rã
được thì `explanation` null kèm cảnh báo `explanation_unavailable`, câu vẫn answered. Câu nói một hướng ("vì sao
giảm", "vì sao cao hơn") mà chênh lệch tính được ngược hướng hay bằng 0 thì kèm cảnh báo `premise_mismatch` nêu chênh
lệch thực tế; phân rã và lời giải thích vẫn trả như thường, theo chênh lệch thực tế.
Lý do và nghĩa từng trường: [Kiến trúc](ARCHITECTURE.md), mục 8, phần Hỏi dữ liệu. Năm response mẫu nằm trong đặc tả
(example của response 200) và ở `frontend/src/api/examples/ask.json`. Guard kiểm câu SQL trước khi chạy:
[Guard SQL](GUARD.md).

## Xóa dataset

`DELETE /api/workspaces/{ws}/datasets/{id}` xóa hẳn một dataset: mọi phiên bản, hồ sơ cột, chỉ số, lịch sử câu hỏi
(CASCADE) và file Parquet trên đĩa (`delete_dataset` trong `backend/src/service/ingest.rs`). Chỉ admin của workspace,
`member` nhận 403; 404 khi dataset không có hoặc thuộc workspace khác, 204 khi xóa xong. Không khôi phục được.

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
