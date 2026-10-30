# Bộ đánh giá

Đo hệ thống trả lời đúng tới đâu bằng bộ câu có SQL đáp án viết tay, chấm theo kết quả chạy (#37, #202). Code ở
`backend/src/service/eval.rs` (chạy, chấm) và `backend/src/service/eval_cases.rs` (định dạng, luật kiểm file).

| File | Vai trò |
|---|---|
| `cases/<dataset>.json` | Bộ câu của một dataset: câu hỏi và SQL đáp án. Có `dataco.json` (DataCo mẫu, chạy được với mock) và `oulad.json` (OULAD `studentInfo.csv`, cần Gemini) |
| `metrics/<dataset>.json` | Chỉ số của dataset không có sẵn trong `data/samples/`, ví dụ OULAD; trỏ tới từ `dataset.metrics` |
| `baseline.json` | Tỷ lệ đúng tối thiểu; `make eval` thoát mã 1 khi thấp hơn |
| `result.json` | Kết quả của lần chạy gần nhất, ghi đè mỗi lần chạy |
| `classify.json` | Bộ câu riêng của bước phân loại câu hỏi (#197), chạy bằng `make eval-classify`; định dạng riêng, không theo file này |

## Chạy

```bash
make eval   # mock, chỉ các câu có "mock": true; CI chạy lệnh này
```

`make eval` luôn dùng mock: mock trả câu viết sẵn trong `backend/src/engine/llm/mock_answers.json`, nên đạt hết chỉ
nói đường ống chạy đúng, không nói mô hình viết SQL giỏi tới đâu. Chạy với Gemini: [docs/RELEASING.md](../docs/RELEASING.md).

Mỗi câu: hệ thống hỏi `question` qua đúng luồng hỏi của giao diện, SQL đáp án chạy qua cùng guard và DuckDB, rồi so hai
bảng kết quả: bỏ qua tên cột, cùng số cột và số dòng, số lệch tương đối dưới 1e-6, không xét thứ tự dòng trừ khi
`ordered`. Câu hỏi tìm nguyên nhân thì bảng của hệ thống là bảng tổng code dựng từ form (một dòng mỗi kỳ hay mỗi nhóm),
nên đáp án viết đúng bảng đó. Câu phải từ chối đạt khi status là `rejected`.

## Định dạng file

```json
{
  "dataset": {
    "name": "DataCo",
    "file": "data/samples/dataco.csv",
    "metrics": "data/samples/dataco.metrics.json"
  },
  "cases": [
    {
      "id": "dc-why-01",
      "kind": "why_periods",
      "question": "Tại sao doanh thu quý 3 năm 2017 giảm",
      "variants": ["tai sao doanh thu quy 3 nam 2017 giam"],
      "expected_sql": ["SELECT ... FROM dataco ..."],
      "ordered": true,
      "mock": true,
      "author": "nlminuong",
      "reviewers": []
    }
  ]
}
```

`dataset.name` là tên dataset khi nạp, cũng cho tên bảng logic mà SQL đáp án dùng (`DataCo` thành `dataco`).
`dataset.metrics` không bắt buộc: có thì chỉ số được ghi vào dataset sau khi nạp, như dataset mẫu người dùng thấy; câu
hỏi tìm nguyên nhân nêu chỉ số theo tên nên cần nó.

| Trường | Bắt buộc | Ý nghĩa |
|---|---|---|
| `id` | Có | Duy nhất trong file, ví dụ `dc-why-01` |
| `kind` | Có | Loại câu, bảng dưới |
| `question` | Có | Câu hỏi như người dùng gõ |
| `variants` | Không | Các cách hỏi khác cùng nghĩa (văn nói, không dấu, đảo trật tự), cùng đáp án |
| `follow_up_of` | Không | Id của câu đứng trước trong cùng cuộc trò chuyện; câu đó phải nằm trước trong file |
| `expected_sql` | Có, trừ câu phải từ chối | SQL đáp án trên tên bảng logic; nhiều câu khi câu hỏi hiểu được nhiều cách, khớp một là đạt |
| `check_sql` | Không | SQL người duyệt tự viết để tính lại đáp án |
| `required_columns` | Không | Vị trí (từ 0) các cột của đáp án kết quả bắt buộc có; rỗng là mọi cột |
| `ordered` | Không | So cả thứ tự dòng: top N, chuỗi thời gian có ORDER BY |
| `must_refuse` | Không | Câu hệ thống phải từ chối; không có `expected_sql` |
| `mock` | Không | Mock có câu trả lời cho `question`; chạy với mock thì chỉ hỏi các câu này |
| `author` | Có | Tài khoản GitHub của người viết |
| `reviewers` | Không | Tài khoản GitHub của người duyệt, không gồm người viết; #202 cần hai người |

Loại câu (`kind`):

| Giá trị | Câu |
|---|---|
| `plain_simple` | Câu thường một bước: gộp theo một chiều, lọc, top N |
| `plain_same_period` | So cùng kỳ năm trước |
| `plain_top_n_in_group` | Top N trong từng nhóm |
| `plain_no_diacritics` | Giá trị lọc gõ không dấu, sai hoa thường |
| `plain_join_trap` | Nối bảng dễ nhân dòng |
| `why_periods` | Tìm nguyên nhân, so hai kỳ |
| `why_groups` | Tìm nguyên nhân, so hai nhóm |
| `why_ratio` | Tìm nguyên nhân trên chỉ số tỷ lệ |
| `why_no_keyword` | Tìm nguyên nhân mà không có chữ "vì sao" |
| `why_false_premise` | Tiền đề sai: hỏi giảm mà số tăng |
| `why_no_period` | Tìm nguyên nhân không nêu kỳ |
| `follow_up` | Câu tiếp nối, cần `follow_up_of` |
| `refuse` | Câu phải từ chối, cần `must_refuse` |

File được kiểm cả trước khi hỏi câu nào, sai thì lệnh dừng với mã 2 và in từng chỗ sai kèm id: trùng id, trường lạ
(gõ nhầm tên trường), loại câu không có trong bảng, câu thiếu đáp án, câu phải từ chối mà có đáp án, cách hỏi trùng câu
hỏi, câu tiếp nối trỏ tới câu không đứng trước, người viết tự duyệt.

## Đang làm (#202)

Đã có: định dạng và luật kiểm file, câu phải từ chối, chỉ số của dataset, bộ câu DataCo và OULAD. Chưa có, theo thứ tự sẽ làm:

- Chấm ba trạng thái (đạt, trượt, cần xem lại), số làm tròn 4 chữ số có nghĩa, cho phép cột thừa theo
  `required_columns`, chạy `check_sql`.
- Hỏi cả `variants`, chạy câu tiếp nối qua cuộc trò chuyện (hiện câu có `follow_up_of` được ghi là bỏ qua), nhiều file
  bộ câu một lần, nghỉ giữa các câu vì hạn mức Gemini, chạy lặp để đo độ dao động.
- Báo cáo theo loại câu kèm khoảng tin cậy và số câu hồi quy so với lần trước.
