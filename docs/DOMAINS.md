# Gói khái niệm ngành

Một gói khái niệm ngành (domain package) mô tả các khái niệm kinh doanh chung của mọi doanh nghiệp cùng ngành: doanh
thu, số đơn, tỷ lệ giao trễ... Gói giúp mô hình ngôn ngữ hiểu thuật ngữ trong câu hỏi; đề xuất chỉ số và kiểm hợp lý
theo gói là thiết kế, chưa có code dùng.

Ba tầng tách riêng:

1. **Engine** không biết ngành: chỉ biết hình dạng của một gói và các luật kiểm chung (`backend/src/engine/domains.rs`).
2. **Gói** nói về khái niệm bằng lời: cách tính phổ biến, từ đồng nghĩa, chiều hay dùng, luật kiểm hợp lý. Gói không
   ghi tên cột của dataset cụ thể.
3. **Chỉ số của workspace** (bảng `metrics`) mới quyết định công thức trên cột thật của dataset.

Hiện có hai gói: `retail` (bán lẻ và chuỗi cung ứng, cho dữ liệu đơn hàng như DataCo hay Online Retail) và
`education` (giáo dục và học trực tuyến, cho schema `edu` của database mẫu `data/postgres-demo/` và OULAD).

## Gói trong workspace

Mỗi workspace có bản riêng của các gói, trong hai bảng Postgres `domain_packs` và `domain_concepts`
(`backend/migrations/0014_domain_concepts.sql`). Lần đầu workspace cần gói (mở màn **Khái niệm ngành** hay hỏi trên
dataset có gói), backend chép các file `data/domains/<id>.json` vào đó. Các lần sau chỉ chép gói và khái niệm mà
workspace chưa có (so theo `id` của gói và của khái niệm), không ghi đè: khái niệm admin đã sửa hay đã tắt giữ nguyên.
Hệ quả: bản phát hành sau thêm gói hay khái niệm mới thì workspace cũ nhận được, còn sửa chữ một khái niệm có sẵn trong
file thì workspace đã chép không nhận.

Trên màn **Khái niệm ngành** (thanh bên), mọi thành viên xem các gói; admin thêm khái niệm, sửa tên, mô tả, từ đồng
nghĩa, chiều phân rã, cách tính, và tắt hay bật khái niệm. Trước khi lưu, backend kiểm như kiểm chỉ số
(`backend/src/service/domains/check.rs`): tên, mô tả, từ đồng nghĩa không rỗng, không xuống dòng, trong trần độ dài;
tên không trùng tên khái niệm khác của gói (không phân biệt hoa thường); từ đồng nghĩa không trùng giữa hai khái niệm
của gói (so sau khi chữ thường và bỏ dấu); tên và từ đồng nghĩa không có ngoặc tròn, từ đồng nghĩa không có dấu phẩy
(các dấu đó làm dòng của khái niệm trong prompt đọc sai); tối đa ba cách tính; luật kiểm nhất quán như file. Khái niệm
admin thêm không bắt buộc có cách tính hay chiều phân rã. API ở [API.md](API.md#khái-niệm-ngành).

Prompt sinh SQL và prompt form câu "tại sao" chèn tên gói và `name`, `synonyms`, `description` của từng khái niệm đang
bật khi cột `sources.domain` có giá trị (`build_domain_section` trong `service/ask/prompt.rs`, dùng cả ở
`service/ask/why_prompt.rs`), đọc từ bảng của workspace ở mỗi câu hỏi, nên khái niệm vừa sửa có ngay ở câu kế tiếp.
Workspace chưa sửa gì nhận đúng chữ như đọc từ file, cùng thứ tự khái niệm. Chưa có cách gắn gói cho dataset (lúc nạp
hay qua API), nên hiện chưa prompt nào có phần này ngoài test. `calculations`, `dimensions`, `sanity_checks` chưa được
code nào đọc ngoài màn quản trị và test.

## Schema của file gói

File là JSON một object, mọi trường dưới đây là bắt buộc trừ khi ghi khác; trường lạ bị từ chối (`deny_unknown_fields`
trong `engine/domains.rs`). Tên trường và giá trị khóa (id, `kind` của luật kiểm) là tiếng Anh; tên và mô tả hiện cho
người dùng là tiếng Việt.

| Trường | Kiểu | Nghĩa |
|---|---|---|
| `id` | chuỗi | Khóa của gói, phải trùng tên file không đuôi (`retail.json` thì `id` là `retail`) |
| `name` | chuỗi | Tên ngành hiện cho người dùng |
| `description` | chuỗi | Ngành này là gì, gói áp dụng cho kiểu dữ liệu nào |
| `concepts` | mảng | Các khái niệm của ngành, mỗi mục như bảng dưới |

Mỗi khái niệm trong `concepts`:

| Trường | Kiểu | Nghĩa |
|---|---|---|
| `id` | chuỗi | Khóa trong gói, tiếng Anh, không trùng nhau, ví dụ `revenue` |
| `name` | chuỗi | Tên hiện cho người dùng, ví dụ "Doanh thu" |
| `description` | chuỗi | Khái niệm nói về điều gì, khi nào dùng |
| `synonyms` | mảng chuỗi | Cách gọi khác, tiếng Việt và tiếng Anh; trong cùng một gói không trùng nhau, kể cả giữa hai khái niệm (so sau khi chữ thường và bỏ dấu). Hai gói khác nhau được dùng chung một từ, vì mỗi dataset chỉ gắn một gói |
| `calculations` | mảng | Hai đến ba cách tính phổ biến, mỗi mục có `name` và `description` bằng lời, kèm khi nào chọn cách đó; người viết gói giữ các cách tính của một khái niệm cùng đơn vị (không trộn tiền với tỷ lệ) và không ghi công thức trên cột thật; test không kiểm hai điều này |
| `dimensions` | mảng chuỗi | Chiều hay dùng để phân rã khái niệm: thời gian, khu vực, nhóm hàng... |
| `sanity_checks` | mảng | Luật kiểm giá trị có hợp lý không; được để rỗng |

Mỗi luật trong `sanity_checks` có trường `kind`: `non_negative` (giá trị không âm, không kèm trường nào khác) hoặc
`value_range` kèm `min`, `max` là số; một đầu có thể vắng (chỉ `max` là chặn trên, ví dụ biên lợi nhuận:
`{"kind": "value_range", "max": 1.0}`), có cả hai thì `min` không được lớn hơn `max`.

## Thêm một ngành

Thêm một file `data/domains/<id>.json` theo schema trên qua PR, không cần sửa code: `backend/build.rs` liệt kê thư mục
lúc biên dịch và nhúng mọi file vào binary (image Docker và binary của `make install` không có thư mục `data/`, cùng lý
do `engine/llm/mock.rs` nhúng câu trả lời mẫu). Workspace nhận gói mới ở lần đầu cần gói sau khi nâng cấp. Thêm hay sửa
file gói thì `cargo build` tự nhúng lại; với `CARGO_TARGET_DIR` dùng chung giữa nhiều bản checkout, chạy
`touch backend/src/main.rs data/domains` trước để chắc chắn build script chạy lại (phải touch cả thư mục
`data/domains`, không chỉ `main.rs`: một bản checkout khác thêm file gói thì thư mục của bản này không đổi, cargo không
tự chạy lại build script).

Test của `engine/domains.rs` chặn gói sai: mọi file đọc được theo schema, `id` trùng tên file, id không trùng giữa các
gói, mỗi khái niệm có hai đến ba cách tính, có từ đồng nghĩa và chiều phân rã, từ đồng nghĩa không rỗng và không
trùng trong một gói, luật kiểm nhất quán (`non_negative` không có `min`, `max`; `value_range` có ít nhất một đầu và
`min` không quá `max`), trường lạ bị từ chối. Test của `service/domains/check.rs` kiểm mọi khái niệm của file qua được kiểm của màn quản
trị, để admin mở rồi lưu lại một khái niệm có sẵn không bị báo lỗi. CI chạy các test này cả khi PR chỉ đổi
`data/domains/`.
