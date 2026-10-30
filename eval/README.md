# Bộ câu đánh giá

| File | Nội dung | Lệnh |
|---|---|---|
| `dataco.json` | 14 câu trên DataCo mẫu, chạy với mock trong CI; `baseline.json` là tỷ lệ đúng tối thiểu, `result.json` là kết quả lần chạy gần nhất | `make eval` |
| `classify.json` | 48 câu thử phân loại câu hỏi (câu thường hay câu tìm nguyên nhân) | `make eval-classify` |
| `oulad.json` | Câu trên OULAD theo định dạng của [#202](https://github.com/pgiahuy/xdi-platform/issues/202): nhóm `single` hỏi trên từng file CSV, nhóm `multi` cần JOIN các bảng `edu` của database mẫu | `make eval-oulad`, `make eval-oulad-check` |

## Định dạng một câu (#202)

File là một mảng JSON, mỗi phần tử một câu gốc. `dataco.json` chỉ dùng bốn trường đầu; các trường còn lại có giá trị
mặc định nên file cũ vẫn đọc được.

| Trường | Ý nghĩa |
|---|---|
| `id` | Mã câu, không trùng trong file |
| `question` | Câu hỏi gốc, tiếng Việt |
| `expected_sql` | Một hoặc nhiều câu SQL đáp án; khớp một câu là đúng. Câu hỏi hiểu được nhiều cách (đếm dòng hay đếm mã sinh viên khác nhau, dạng dài hay dạng rộng) thì mỗi cách một câu. Rỗng khi câu phải từ chối |
| `ordered` | Có so thứ tự dòng không: chỉ đúng với câu có top N hay ORDER BY |
| `variants` | Hai cách hỏi khác của cùng câu (văn nói, không dấu, đảo trật tự), chấm với cùng đáp án |
| `check_sql` | Mỗi câu của `expected_sql` một câu tính lại viết riêng, cùng thứ tự; `make eval-oulad-check` so hai bên |
| `required_columns` | Tên cột của đáp án mà câu trả lời phải có (theo bí danh trong `expected_sql`) |
| `kind` | Loại câu, bảng dưới |
| `must_refuse` | Dữ liệu không có câu trả lời; hệ thống từ chối (`rejected`) là đúng |
| `tables` | Bảng logic câu hỏi đọc; bảng đầu là dataset để hỏi |
| `group` | `single`: một bảng; `multi`: nhiều bảng, cần JOIN (chờ [#213](https://github.com/pgiahuy/xdi-platform/issues/213)) |
| `earlier` | Không bắt buộc: các câu hỏi trước của cuộc trò chuyện, cũ trước |
| `author`, `reviewers` | Người viết và người duyệt (tên GitHub); `reviewers` để trống tới khi đội duyệt |
| `note` | Không bắt buộc: cách hiểu đã chọn, giá trị đáng chú ý, bẫy của câu |

| `kind` | Loại |
|---|---|
| `why_periods`, `why_groups` | Câu cần giải thích, so hai kỳ hay hai nhóm, trên số đếm hoặc tổng |
| `ratio` | Câu cần giải thích trên chỉ số tỷ lệ (tỷ lệ đạt, tỷ lệ bỏ học) |
| `why_implicit` | Câu cần giải thích không có cụm "vì sao", "tại sao" |
| `false_premise` | Tiền đề sai: dữ liệu cho thấy ngược lại điều câu hỏi giả định |
| `why_no_period` | Câu cần giải thích không nêu kỳ |
| `follow_up` | Câu tiếp nối, cần `earlier` |
| `refuse` | Phải từ chối, `must_refuse` là true |
| `plain`, `same_period`, `top_n_group`, `no_diacritics`, `fanout_trap` | Câu thường: thường, so cùng kỳ, top N trong từng nhóm, viết không dấu, bẫy nhân dòng khi nối bảng một-nhiều ([#214](https://github.com/pgiahuy/xdi-platform/issues/214)) |

Với câu cần giải thích, đáp án là số liệu chính của phép so: chỉ số của hai kỳ hay hai nhóm được so. Tỷ lệ viết dạng
phân số từ 0 đến 1. OULAD không có cột ngày; "kỳ" là giá trị của `code_presentation` (2013J là kỳ bắt đầu tháng 10
năm 2013), nên so hai kỳ trên OULAD là so hai giá trị của cột này.

## Quy ước của bộ OULAD

- Tên bảng nhóm `single` là tên file viết thường, như khi nạp từng file mà không đặt tên: `studentInfo.csv` thành
  `studentinfo`. Cột giữ tên và kiểu DuckDB đoán khi nạp: `score`, `date_registration`, `date_unregistration`, `date`
  của `assessments` thành chữ vì nguồn ghi `?` khi thiếu, nên đáp án dùng `TRY_CAST`. Không có câu nào trên
  `studentVle.csv` (454 MB, quá `MAX_UPLOAD_MB` mặc định); số lần bấm hỏi ở nhóm `multi` qua `student_daily_clicks`.
- Tên bảng nhóm `multi` là tên bảng trong schema `edu` của [database mẫu](../data/postgres-demo/README.md), như khi
  chụp bảng. SQL viết để chạy được cả trên DuckDB và Postgres.
- "Số sinh viên" là số dòng của `studentinfo` (một sinh viên trong một học phần); khi đếm mã sinh viên khác nhau cho ra
  số khác thì có thêm đáp án đếm `COUNT(DISTINCT id_student)`. "Đạt" là `Pass` hoặc `Distinction`, mẫu số gồm cả
  `Withdrawn`.

## Chạy

- `make eval-oulad-check OULAD_DIR=...`: chạy mọi `expected_sql` và `check_sql` qua guard và DuckDB trên OULAD thật
  (mặc định `data/downloads/oulad`, tải bằng `sh data/download.sh oulad`), so sau khi làm tròn 4 chữ số có nghĩa.
  Nhóm `single` nạp CSV bằng đúng hàm nạp của app; nhóm `multi` dựng bảng `edu` từ cùng các CSV theo
  `data/postgres-demo/sql/edu_load.sql`. Không cần Postgres hay mô hình.
- `make eval-oulad LLM_PROVIDER=gemini PAUSE_MS=4000`: hỏi từng câu và từng cách hỏi của nhóm `single` qua đúng luồng
  hỏi, mỗi file CSV là một dataset. Kết quả ghi vào `backend/target/eval-oulad-result.json`. Mock không trả lời được
  các câu này.

Runner (`xdi-backend eval`) chưa làm phần chấm của #202: so số theo sai số tương đối 1e-6 chứ chưa làm tròn 4 chữ số
có nghĩa, cột thừa là sai, chưa có trạng thái "cần xem lại", chưa đọc `kind` và `required_columns`. Nó bỏ qua nhóm
`multi` và câu có `earlier`, in ra danh sách bỏ qua.
