# Gói khái niệm ngành

Một gói khái niệm ngành (domain package) mô tả các khái niệm kinh doanh chung của mọi doanh nghiệp cùng ngành: doanh
thu, số đơn, tỷ lệ giao trễ... Mỗi ngành một file `data/domains/<id>.json`. Gói giúp hệ thống hiểu thuật ngữ trong câu
hỏi và đề xuất chỉ số.

Ba tầng tách riêng:

1. **Engine** không biết ngành: chỉ biết hình dạng của một gói, có hàm liệt kê mọi gói và tìm gói theo id
   (`backend/src/engine/domains.rs`).
2. **Gói** nói về khái niệm bằng lời: cách tính phổ biến, từ đồng nghĩa, chiều hay dùng, luật kiểm hợp lý. Gói không
   ghi tên cột của dataset cụ thể.
3. **Chỉ số của workspace** (bảng `metrics`) mới quyết định công thức trên cột thật của dataset.

Hiện tại có một gói: `retail` (bán lẻ và chuỗi cung ứng), áp dụng được cho dữ liệu đơn hàng như DataCo hay Online
Retail. Gói chưa nối vào prompt (#29) hay bước nạp dataset; khi nối, người dùng chọn một gói lúc nạp, một dataset một
gói.

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
| `calculations` | mảng | Hai đến ba cách tính phổ biến, mỗi mục có `name` và `description` bằng lời, kèm khi nào chọn cách đó; các cách tính của một khái niệm phải cùng đơn vị (không trộn tiền với tỷ lệ) và không ghi công thức trên cột thật |
| `dimensions` | mảng chuỗi | Chiều hay dùng để phân rã khái niệm: thời gian, khu vực, nhóm hàng... |
| `sanity_checks` | mảng | Luật kiểm giá trị có hợp lý không; được để rỗng |

Mỗi luật trong `sanity_checks` có trường `kind`: `non_negative` (giá trị không âm, không kèm trường nào khác) hoặc
`value_range` kèm `min`, `max` là số; một đầu có thể vắng (chỉ `max` là chặn trên, ví dụ biên lợi nhuận:
`{"kind": "value_range", "max": 1.0}`), có cả hai thì `min` không được lớn hơn `max`.

## Thêm một ngành

Thêm một file `data/domains/<id>.json` theo schema trên, không cần sửa code: `backend/build.rs` liệt kê thư mục lúc
biên dịch và nhúng mọi file vào binary (image Docker và binary của `make install` không có thư mục `data/`, cùng lý
do `engine/llm/mock.rs` nhúng câu trả lời mẫu). Thêm hay sửa file gói thì `cargo build` tự nhúng lại; với
`CARGO_TARGET_DIR` dùng chung giữa nhiều bản checkout, chạy `touch backend/src/main.rs data/domains` trước để chắc
chắn build script chạy lại (phải touch cả thư mục `data/domains`, không chỉ `main.rs`: một bản checkout khác thêm file
gói thì thư mục của bản này không đổi, cargo không tự chạy lại build script). Test của `engine/domains.rs` chặn gói sai: mọi file đọc được theo schema, `id` trùng tên
file, id không trùng giữa các gói, mỗi khái niệm có hai đến ba cách tính, có từ đồng nghĩa và chiều phân rã, từ đồng
nghĩa không rỗng và không trùng trong một gói, luật kiểm nhất quán (`non_negative` không có `min`/`max`,
`value_range` có ít nhất một đầu và `min` không quá `max`). CI chạy các test này cả khi PR chỉ đổi `data/domains/`.
