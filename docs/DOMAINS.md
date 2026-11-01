# Gói khái niệm ngành

Một gói khái niệm ngành (domain package) mô tả các khái niệm kinh doanh chung của mọi doanh nghiệp cùng ngành: doanh
thu, số đơn, tỷ lệ giao trễ... Mục tiêu là giúp mô hình ngôn ngữ hiểu thuật ngữ trong câu hỏi.

Trạng thái: gói, màn quản trị, chọn gói cho từng dataset và phần chèn vào prompt đã có. Thiết lập lần đầu gắn sẵn gói
cho dataset mẫu. Đề xuất chỉ số và kiểm hợp lý theo gói chưa có code dùng.

Ba tầng tách riêng:

1. **Engine** không biết ngành: chỉ biết hình dạng của một gói và các luật kiểm chung (`backend/src/engine/domains.rs`).
2. **Gói** nói về khái niệm bằng lời: cách tính phổ biến, từ đồng nghĩa, chiều hay dùng, luật kiểm hợp lý. Gói không
   ghi tên cột của dataset cụ thể.
3. **Chỉ số của workspace** (bảng `metrics`) mới quyết định công thức trên cột thật.

Hiện có hai gói: `retail` (bán lẻ và chuỗi cung ứng, cho dữ liệu đơn hàng như DataCo hay Online Retail) và
`education` (giáo dục và học trực tuyến, cho database mẫu `edu_demo` với dữ liệu OULAD).

## Gói trong workspace

Mỗi workspace có bản riêng của các gói, trong bảng `domain_packs` và `domain_concepts`. Lần đầu workspace cần gói (mở
màn **Khái niệm ngành** hay hỏi trên dataset có gói), backend chép các file `data/domains/<id>.json` vào. Các lần sau
chỉ chép gói và khái niệm workspace chưa có (so theo `id`), không ghi đè: khái niệm admin đã sửa hay đã tắt giữ nguyên.
Vì vậy bản phát hành sau thêm khái niệm mới thì workspace cũ nhận được, còn sửa chữ một khái niệm có sẵn trong file thì
workspace đã chép không nhận.

Trên màn **Khái niệm ngành**, mọi thành viên xem các gói; admin thêm khái niệm, sửa tên, mô tả, từ đồng nghĩa, chiều
phân rã, cách tính, và tắt hay bật khái niệm. Backend kiểm trước khi lưu: chữ không rỗng, không trùng trong gói, tối đa
ba cách tính, không có ký tự làm hỏng dòng của khái niệm trong prompt (`backend/src/service/domains/check.rs`). API ở
[API.md](API.md#khái-niệm-ngành).

## Gói của một dataset

Mỗi dataset dùng tối đa một gói, lưu ở cột `sources.domain` (khóa của gói, null là không dùng gói); mọi bảng của
dataset đọc cùng gói đó. Đầu màn **Khái niệm ngành** có danh sách dataset kèm gói đang dùng: admin chọn gói hay
"Không dùng gói" trong ô chọn của từng dataset, member chỉ xem. API: `PUT .../catalog/{id}/domain`
([API.md](API.md#khái-niệm-ngành)). Dataset mới nạp hay mới kết nối chưa có gói cho tới khi admin chọn.

Thiết lập lần đầu gắn gói cho dataset mẫu theo `data/samples/domains.json` (khóa là tên dataset, giá trị là khóa gói):
`sales_demo` và `DataCo` dùng `retail`, `edu_demo` dùng `education`. Bản cài đã thiết lập trước khi có file này thì
dataset mẫu chưa có gói; admin chọn trên màn **Khái niệm ngành**.

Prompt viết SQL và prompt form của câu hỏi nguyên nhân chèn tên gói và `name`, `synonyms`, `description` của từng khái
niệm đang bật khi `sources.domain` có giá trị (`build_domain_section` trong `backend/src/service/ask/prompt.rs`). Bảng
của workspace được đọc ở mỗi câu hỏi, nên khái niệm vừa sửa có ngay ở câu kế tiếp. `calculations`, `dimensions`,
`sanity_checks` chưa được code nào đọc ngoài màn quản trị và test.

## Schema của file gói

File là JSON một object; mọi trường dưới đây bắt buộc trừ khi ghi khác, trường lạ bị từ chối. Tên trường và khóa (id,
`kind` của luật kiểm) là tiếng Anh; tên và mô tả hiện cho người dùng là tiếng Việt.

| Trường | Kiểu | Nghĩa |
|---|---|---|
| `id` | chuỗi | Khóa của gói, trùng tên file không đuôi (`retail.json` thì `id` là `retail`) |
| `name` | chuỗi | Tên ngành hiện cho người dùng |
| `description` | chuỗi | Ngành này là gì, gói áp dụng cho kiểu dữ liệu nào |
| `concepts` | mảng | Các khái niệm của ngành, mỗi mục như bảng dưới |

Mỗi khái niệm trong `concepts`:

| Trường | Kiểu | Nghĩa |
|---|---|---|
| `id` | chuỗi | Khóa trong gói, tiếng Anh, không trùng nhau, ví dụ `revenue` |
| `name` | chuỗi | Tên hiện cho người dùng, ví dụ "Doanh thu" |
| `description` | chuỗi | Khái niệm nói về điều gì, khi nào dùng |
| `synonyms` | mảng chuỗi | Cách gọi khác, tiếng Việt và tiếng Anh; không trùng trong một gói (so sau khi chữ thường và bỏ dấu). Hai gói khác nhau được dùng chung một từ |
| `calculations` | mảng | Hai đến ba cách tính phổ biến, mỗi mục có `name` và `description` bằng lời, kèm khi nào chọn; cùng đơn vị, không ghi công thức trên cột thật (test không kiểm hai điều này) |
| `dimensions` | mảng chuỗi | Chiều hay dùng để phân rã: thời gian, khu vực, nhóm hàng... |
| `sanity_checks` | mảng | Luật kiểm giá trị có hợp lý không; được để rỗng |

Mỗi luật trong `sanity_checks` có `kind`: `non_negative` (giá trị không âm, không kèm trường khác) hay `value_range`
kèm `min`, `max` là số; một đầu được vắng (ví dụ biên lợi nhuận: `{"kind": "value_range", "max": 1.0}`), có cả hai
thì `min` không lớn hơn `max`.

## Thêm một ngành

Thêm file `data/domains/<id>.json` theo schema trên qua PR, không cần sửa code: `backend/build.rs` nhúng mọi file của
thư mục vào binary lúc biên dịch (image Docker và binary của `make install` không có thư mục `data/`). Workspace nhận
gói mới ở lần đầu cần gói sau khi nâng cấp. Dùng chung `CARGO_TARGET_DIR` giữa nhiều bản checkout thì chạy
`touch backend/src/main.rs data/domains` trước khi build để build script chạy lại.

Test của `engine/domains.rs` chặn file gói sai schema hay sai luật; test của `service/domains/check.rs` kiểm mọi khái
niệm của file qua được kiểm của màn quản trị. CI chạy các test này cả khi PR chỉ đổi `data/domains/`.
