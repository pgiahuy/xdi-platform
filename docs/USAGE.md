# Dùng lần đầu

Từ màn thiết lập tới câu trả lời đầu tiên, theo giao diện hiện tại. Cài đặt và cấu hình ở [Cài đặt](INSTALL.md). Địa
chỉ là http://localhost:8000 (cổng theo `BACKEND_PORT`); chạy giao diện bằng `make dev-frontend` thì là
http://localhost:5173.

| Mục | Nội dung |
|---|---|
| [1. Chọn mô hình và dữ liệu mẫu](#1-chọn-mô-hình-và-dữ-liệu-mẫu) | `mock`, Gemini hay Ollama; DataCo, database mẫu; tạo tài khoản quản trị |
| [2. Kết nối database hoặc nạp file](#2-kết-nối-database-hoặc-nạp-file) | Trang **Dữ liệu**: nguồn Postgres, chụp bảng, nạp file, xóa |
| [3. Chờ hồ sơ dữ liệu](#3-chờ-hồ-sơ-dữ-liệu) | Mô tả cột, bí danh, câu gợi ý do mô hình viết |
| [4. Hỏi](#4-hỏi) | Trang hỏi, cuộc trò chuyện, lịch sử, tổng quan dữ liệu |
| [5. Đọc câu trả lời và minh chứng](#5-đọc-câu-trả-lời-và-minh-chứng) | Thẻ Trả lời, thẻ Cách tính, pane Minh chứng |
| [6. Câu hỏi nguyên nhân](#6-câu-hỏi-nguyên-nhân) | Phân rã, lời giải thích, kiểm số và kiểm nghĩa |
| [7. Chỉ số và cách gộp](#7-chỉ-số-và-cách-gộp) | Công thức của tổ chức, tổng hay trung bình |
| [8. Báo cáo](#8-báo-cáo) | Gom câu trả lời, báo cáo tự động, xuất PDF và Excel |
| [9. Mời thành viên](#9-mời-thành-viên) | Tạo tài khoản, thêm vào workspace, vai `admin` và `member` |
| [10. Vai dữ liệu](#10-vai-dữ-liệu) | Thành viên mang vai chỉ thấy các dòng được phép |
| [11. Đánh giá, câu mẫu, bài học](#11-đánh-giá-câu-mẫu-bài-học) | Phản hồi trên câu trả lời và cách admin dùng nó |

## 1. Chọn mô hình và dữ liệu mẫu

Chọn mô hình trước khi nạp dữ liệu: sau mỗi lần nạp hay chụp, backend gọi mô hình đang cấu hình để lập hồ sơ (mục 3).
Cách đặt biến: [Cài đặt](INSTALL.md#mô-hình-ngôn-ngữ). Mô hình đang dùng ghi ở dòng **Mô hình**, góc dưới thanh bên.

| `LLM_PROVIDER` | Cần | Hỏi được |
|---|---|---|
| `mock` (mặc định) | Không cần gì, không gọi ra ngoài | Chỉ các câu mẫu viết sẵn cho DataCo và cho các bảng của database mẫu; câu khác nhận lời báo `Chế độ mock chỉ trả lời các câu hỏi mẫu của dataset mẫu` |
| `gemini` | `GEMINI_API_KEY`, khóa tạo trong Google AI Studio | Câu tự do trên dataset bất kỳ |
| `ollama` | Ollama đang chạy và mô hình đã tải | Câu tự do trên dataset bất kỳ |

**Không tải gì.** Thiết lập lần đầu nạp dataset `DataCo` (bảng `dataco`, 1500 dòng, `data/samples/dataco.csv`) kèm vài
chỉ số mẫu như "Doanh thu", "Lợi nhuận", "Tỷ lệ giao trễ". Câu mẫu của `mock` nằm trong
`backend/src/engine/llm/mock_answers.json`, ví dụ "Doanh thu theo thị trường", "Top 10 danh mục có doanh thu cao nhất",
"Vì sao doanh thu tháng 5 năm 2017 tăng".

**Database mẫu.** Luồng chính là kết nối database. Repo có database mẫu nhiều bảng: `sales_demo` (DataCo) và `edu_demo`
(OULAD), có khóa ngoại, mô tả cột và nguyên nhân cài sẵn để thử câu hỏi nguyên nhân. Tải, nạp và chạy cùng compose theo
[data/postgres-demo](https://github.com/pgiahuy/xdi-platform/blob/main/data/postgres-demo/README.md), **trước** lần
thiết lập đầu. Khi đó thiết lập lần đầu thêm hai dataset `sales_demo`, `edu_demo` (mỗi database một dataset), chụp mọi
bảng tài khoản `analyst` đọc được (13 bảng, 12 quan hệ) cùng chỉ số mẫu của `order_items` và `student_assessments`, và
không nạp DataCo từ file. Câu mẫu của `mock` cho các bảng này nằm trong
`backend/src/engine/llm/mock_answers_database.json`, ví dụ "Doanh thu theo phân khúc khách hàng" (nối `order_items`,
`orders`, `customers`), "Tỷ lệ bỏ học theo môn học", "Điểm trung bình theo loại bài đánh giá". Một database không tới
được thì backend ghi cảnh báo và giữ database còn lại.

Bản cài đã thiết lập rồi, hay đặt `SEED_SAMPLE=false`: kết nối database mẫu ở mục 2 (máy chủ `demo-db`, cổng 5432,
database `sales_demo`, rồi thêm nguồn thứ hai với `edu_demo`; tài khoản và mật khẩu `analyst`; `sslmode` `disable`),
hoặc nạp tay `data/samples/dataco.csv` với tên dataset `DataCo`, để tên bảng là `dataco` như câu mẫu cần.

**Tạo tài khoản quản trị.** Màn **Thiết lập lần đầu**: điền **Mã thiết lập** (lấy trong log, [Cài đặt](INSTALL.md#tạo-tài-khoản-quản-trị)),
**Email quản trị**, **Mật khẩu** (8 đến 128 ký tự, nhập hai lần); **Tên hiển thị** và **Tên workspace đầu tiên** không
bắt buộc. Bấm **Tạo tài khoản quản trị**: giao diện đăng nhập và mở trang hỏi của workspace vừa tạo, với dữ liệu mẫu ở
trên (trừ khi `SEED_SAMPLE=false`). Workspace thứ hai trở đi: ô chọn workspace đầu thanh bên, **Tạo workspace**; người
tạo là `admin` của workspace mới.

## 2. Kết nối database hoặc nạp file

Mục **Dữ liệu** ở thanh bên. Chỉ `admin` của workspace thêm, chụp và xóa được; thành viên xem danh sách.

**Kết nối database.** Máy chủ phải có trong `SOURCE_ALLOWED_HOSTS`. Khung **Kết nối database** mở sẵn khi workspace chưa
có nguồn nào, không thì bấm **Thêm database**. Nhập máy chủ, cổng, database, tài khoản, mật khẩu và `sslmode`
(`prefer`, `require`, `verify-full` kèm ô **Chứng chỉ CA**; nghĩa từng giá trị ở [Dữ liệu](DATA.md#nguồn-database-postgres)).
Bấm **Kết nối**: backend thử kết nối trước khi lưu.

Kết nối xong, khung liệt kê mọi bảng và view tài khoản đọc được, nhóm theo schema, tick sẵn, kèm số dòng ước lượng, dung
lượng, số cột, mô tả. Bảng quá trần ghi **Quá trần dung lượng, chỉ lưu cấu trúc** (hay **Quá trần dòng** khi đặt
`SOURCE_MAX_ROWS`). Bỏ tick bảng không cần; bỏ tick một bảng mà bảng đang tick trỏ tới thì khung báo quan hệ nào mất.
Bấm **Chụp N bảng**: mọi bảng chụp trong một giao dịch, xong thì khung ghi bảng đã chụp và bảng chỉ lưu cấu trúc kèm lý
do. Muốn giới hạn dữ liệu xdi đọc thì cấp quyền hẹp cho tài khoản nguồn, không dựa vào việc bỏ tick.

Sau đó thẻ của database có:

- **Chụp lại**: chụp lại mọi bảng dataset đang giữ; bảng không đổi nội dung không tạo phiên bản mới.
- **Chụp thêm bảng**: mở lại danh sách, tick sẵn các bảng đang giữ.
- **Xem bảng**: bảng theo schema, quan hệ giữa các bảng, nút **Tổng quan**, **Chỉ số**, **Xóa** của từng bảng. Bảng chỉ có
  cấu trúc ghi **Chỉ có cấu trúc** kèm lý do, và có nút **Chụp bảng này** sau khi tăng trần.
- **Xóa nguồn**: xóa thông tin kết nối, chỉ được khi database không còn bảng đã chụp.

**Nạp file**, khi dữ liệu chưa nằm trong database: bấm **Nạp file CSV, Parquet hoặc Excel** dưới khung database.

1. Chọn một hoặc nhiều file CSV hay Parquet cùng cột (gộp thành một bảng), hoặc một file Excel `.xlsx`. File `.xls` thì
   mở bằng Excel rồi lưu lại dạng `.xlsx`.
2. File Excel: chọn sheet trong **Chọn sheet cần nạp**; các sheet khác cột nhau thành nhiều dataset riêng, giao diện báo
   trước số dataset.
3. **Tên dataset** để trống thì lấy tên file. Tên bảng dùng trong SQL đặt theo tên dataset: chữ thường, bỏ dấu, khoảng
   trắng thành `_` ("Bán hàng 2024" thành `ban_hang_2024`).
4. Bấm **Nạp**. Nạp lại đúng file đã có thì không tạo bản mới. Dữ liệu tháng sau thì bấm **Nạp phiên bản mới** ở thẻ của
   file: phiên bản mới thành hiện hành, phiên bản cũ vẫn giữ.

Định dạng, mã hóa CSV, giới hạn và tên cột: [Dữ liệu](DATA.md).

**Xóa.** Nút **Xóa** của một bảng xóa hẳn bảng đó (với file: cả dataset), cùng mọi phiên bản, hồ sơ cột, chỉ số, cuộc
trò chuyện, lịch sử câu hỏi, mục báo cáo và file Parquet của nó; không khôi phục được, trừ từ bản sao lưu làm trước đó.
Xóa cả một database: xóa hết bảng rồi **Xóa nguồn**. Chi tiết: [PRIVACY.md](PRIVACY.md).

## 3. Chờ hồ sơ dữ liệu

Kiểu và vai trò từng cột xong ngay trong lần nạp hay chụp. Phần do mô hình viết (mô tả cột, bí danh, câu gợi ý) chạy nền
sau đó. Trong lúc chờ, trang hỏi ghi "Đang lập hồ sơ dataset: câu gợi ý và mô tả cột sẽ có sau ít phút."; xong thì hiện
câu gợi ý. Hỏi được ngay cả khi hồ sơ chưa xong.

- "Mô hình hiện tại không lập được câu gợi ý cho dataset này; vẫn hỏi được bình thường.": mô hình là `mock` mà bảng không
  có hồ sơ viết sẵn, hay mô hình thật trả lời mà không dùng được. Đổi mô hình rồi khởi động lại backend thì backend thử
  lại.
- Mô hình lỗi lúc lập hồ sơ (khóa sai, quá tải, hết hạn mức): giao diện không báo, log ghi lỗi; backend thử lại ở lần
  khởi động sau.
- "Lập hồ sơ dataset lỗi nên chưa hỏi được phiên bản này": phần cấu trúc lỗi, nạp lại file.

## 4. Hỏi

Vào workspace là mở trang hỏi. Ô **Dataset** đầu khung chọn sẵn database đầu tiên (không có database thì dataset đầu
tiên); đổi dataset là bắt đầu cuộc mới. Với database, ghim một bảng làm bảng bắt đầu là không bắt buộc: câu hỏi đọc được
mọi bảng của dataset và nối chúng theo khóa ngoại. Gõ câu hỏi rồi bấm **Hỏi** (Enter để hỏi, Shift+Enter để xuống dòng),
hoặc bấm một câu gợi ý.

Thử với `mock`: trên `sales_demo`, "Doanh thu theo phân khúc khách hàng"; trên `edu_demo`, "Tỷ lệ bỏ học theo môn học",
rồi "Điểm trung bình theo loại bài đánh giá" (nối `student_assessments` với `assessments`). Trên DataCo từ file, "Doanh
thu theo thị trường", rồi hỏi tiếp "5 danh mục có doanh thu cao nhất ở châu Âu" trong cùng cuộc: câu sau hiểu theo các
câu trước.

- Câu có một cụm khớp nhiều cột thì backend trả lời bằng một cột và ghi giả định trong thẻ **Cách tính**; bấm cột khác
  là hỏi lại theo cột đó.
- Câu hiểu được nhiều cách thì backend hỏi lại; bấm một phương án là hỏi lại theo cách hiểu đó.
- Câu cần thông tin dataset không có (ví dụ "Tại sao khách hàng không hài lòng?" khi không có cột đánh giá, hay hỏi dự
  báo) thì không có bảng số: kết quả nói lý do và gợi ý các cột gần nhất.
- Câu cần một bảng chỉ có cấu trúc thì lời báo nêu tên bảng đó và cách chụp nó.
- Lỗi (mô hình lỗi, SQL bị từ chối hay chạy lỗi) hiện ngay trên ô hỏi; lỗi Gemini: [Cài đặt](INSTALL.md#6-lỗi-hay-gặp).

**Lịch sử trò chuyện.** Nút đầu thanh bên mở danh sách cuộc trò chuyện của bạn trên mọi dataset của workspace, mới
trước; bấm một cuộc để mở lại và hỏi tiếp, nút dấu cộng mở cuộc mới. Lượt cũ hiện câu hỏi, lời trả lời và phần Cách
tính; bảng và biểu đồ không lưu, bấm **Hỏi lại** để chạy lại trên dữ liệu hiện tại. Đường dẫn ghi cuộc đang mở, nên tải
lại trang vẫn đúng cuộc.

**Tổng quan dữ liệu.** Mục trên thanh bên (hay nút **Tổng quan** của một bảng ở trang **Dữ liệu**): chọn dataset rồi
bảng (mặc định bảng giữ nhiều khóa ngoại nhất). Trang ghi số dòng, khoảng thời gian có dữ liệu, các cột theo nhóm (số
đo, chia nhóm, thời gian, mã, chữ khác), vài biểu đồ dựng sẵn và các chỉ số. Bấm một cột, giá trị, chỉ số hay biểu đồ
thì câu hỏi tương ứng được điền vào ô hỏi. Trang này không gọi mô hình. Người có vai dữ liệu không thấy các số tính trên
mọi dòng (ô trống, số giá trị khác nhau, số dòng theo giá trị).

## 5. Đọc câu trả lời và minh chứng

Thẻ **Trả lời** dưới câu hỏi có lời trả lời ngắn, biểu đồ khi kết quả hợp (đường khi nhóm theo thời gian, cột khi nhóm
theo một chiều với một độ đo), bảng **Kết quả** và nút **Tải CSV**. Giao diện nhận tối đa 1000 dòng đầu; kết quả dài
ghi `Hiện 1.000 trên <tổng> dòng.` Số hiện theo kiểu Việt Nam (dấu chấm ngăn nghìn, dấu phẩy thập phân); CSV giữ giá
trị gốc. Chỗ lời giải thích chưa rõ nghĩa hiện ngay dưới lời, để người đọc đối chiếu với số đã tính.

Lời giải thích của câu hỏi số liệu do code chọn ý theo hình dạng kết quả: theo nhóm thì nhóm đứng đầu và tỷ trọng (độ đo
là trung bình hay tỷ lệ thì nhóm cao nhất và thấp nhất), theo hai cột nhóm thì các tổ hợp lớn nhất và nhóm đứng đầu của
từng cột, theo thời gian thì tổng các kỳ, xu hướng và kỳ đổi mạnh nhất. Tỷ lệ viết bằng phần trăm và điểm phần trăm. Câu
hỏi chung kiểu "tình hình doanh thu" được trả lời theo tháng. Lời của mô hình không qua kiểm số hay kiểm nghĩa thì đoạn
trả lời là câu chính hệ thống đã tính.

Thẻ **Cách tính** ghi cách hệ thống hiểu câu hỏi, đọc từ câu SQL đã chạy: **Đo**, **Lọc**, **Nhóm theo**, **Sắp xếp**,
**Giới hạn**, **Khoảng thời gian**, và bộ lọc theo quyền xem khi người hỏi có vai dữ liệu. Câu có giả định thì kèm
**Giả định, bấm để đổi rồi hỏi lại**.

Pane **Minh chứng** nằm bên phải trên màn rộng, dưới khung trò chuyện trên màn hẹp; nút **Minh chứng** ở cuối mỗi câu
trả lời chọn lượt cần xem. Bốn tab:

- **SQL**: câu mô hình viết (ghi "bản đã sửa" khi câu đầu lỗi và mô hình sửa lại) và câu đã chạy sau guard, kèm những gì
  guard thêm, ví dụ `LIMIT`. Guard là gì: [GUARD.md](GUARD.md).
- **Phân rã**: chỉ có với câu hỏi nguyên nhân (mục 6); nghĩa từng cột ghi ngay dưới tiêu đề cột.
- **Dòng nguồn**: bấm một ô số đo của bảng **Kết quả**, hay số của một nhóm trong bảng phân rã, để xem các dòng dữ liệu
  đứng sau nó, kể cả câu nối bảng. Tab ghi dòng đến từ bảng nào, cột nào lấy từ bảng nối, cột `_source_file` (file hay
  bảng gốc của từng dòng), và độ đo tính lại trên mọi dòng đó để so với ô đã bấm. Câu có `HAVING`, hàm cửa sổ, `UNION`,
  `DISTINCT`, `LIMIT` hay bảng tạm gộp nhóm chưa truy được; tab ghi lý do.
- **Các bước**: từng bước backend đã chạy kèm thời gian, đạt hay lỗi: đọc hồ sơ và chọn bảng, phân loại câu hỏi (cờ
  Quantitative hay Causal), viết SQL hay điền form, guard, chạy, phân rã, viết lời (cờ kết luận kiểm nghĩa: match,
  warning, mismatch), lưu lịch sử. Dưới đó là từng lượt gọi mô hình kèm thời gian và số token, để thấy mô hình chỉ phân
  loại, viết SQL hay form và viết lời, còn con số do DuckDB và backend tính.

## 6. Câu hỏi nguyên nhân

Bước phân loại nhờ mô hình nói câu hỏi là câu hỏi số liệu hay câu hỏi nguyên nhân, dựa trên câu hỏi, tên cột và các câu
trước của cuộc, nên nhận cả "Doanh thu giảm do gì?" hay "Còn tháng 6 thì sao?", còn "Số đơn theo lý do hủy" vẫn là câu
hỏi số liệu. Mô hình lỗi hay quá giờ (và với `mock` khi câu không phải câu mẫu) thì backend dùng các cụm "tại sao", "vì
sao", "sao lại", "vì đâu", "lý do", "nguyên nhân", "do đâu", gõ không dấu cũng được.

Với câu hỏi nguyên nhân, mô hình không viết SQL mà điền một form: đo gì, so hai kỳ, hai nhóm hay một nhóm với các nhóm
còn lại, lọc gì. Backend kiểm form rồi tự dựng mọi câu SQL. Câu nêu tên hai nhóm ("giữa Europe và LATAM") thì so đúng
hai nhóm đó; câu chỉ nêu một nhóm ("vì sao AAA ít sinh viên") thì so với trung bình các nhóm còn lại, và lời ghi rõ
"trung bình các ... còn lại". Độ đo là tỷ lệ dòng (tỷ lệ giao trễ, tỷ lệ bỏ học) thì lời viết phần trăm và điểm phần
trăm. Câu không nêu kỳ ("Tại sao doanh thu giảm") thì so kỳ gần
nhất có dữ liệu với kỳ liền trước, lùi một kỳ khi kỳ cuối dừng giữa chừng; thẻ **Cách tính** ghi giả định này kèm nút
chọn kỳ khác.

Đầu thẻ trả lời là hàng ba số (hai bên và chênh lệch), lời giải thích, rồi thanh chênh lệch của từng nhóm theo chiều
giải thích tốt nhất. Tab **Phân rã** có mỗi chiều một bảng: phần chênh lệch của từng nhóm (cộng lại bằng tổng), phần
trăm tác động, phần trăm thay đổi; nhóm khác thường in đậm, bảng có nhóm như vậy đứng trước.

Lời giải thích do mô hình viết rồi qua hai bước kiểm: kiểm số (code so từng số, chiều tăng giảm với bảng đã tính) và
kiểm nghĩa (một lần gọi mô hình riêng chấm nghĩa của lời). Sai thì mô hình viết lại một lần; vẫn sai thì dùng câu do hệ
thống viết. Hỏi "vì sao giảm" mà số thật ra tăng thì kết quả có cảnh báo nói chênh lệch thực tế. Với `mock` trên DataCo,
thử "Vì sao doanh thu tháng 5 năm 2017 tăng" và "Tại sao doanh thu quý 3 năm 2017 giảm". Trên database mẫu, hai câu có
đáp án biết trước ở [data/postgres-demo](https://github.com/pgiahuy/xdi-platform/blob/main/data/postgres-demo/README.md#nguyên-nhân-cài-sẵn)
(cần Gemini hay Ollama).

## 7. Chỉ số và cách gộp

Nút **Chỉ số** của một bảng ở trang **Dữ liệu**. Mọi thành viên xem được, chỉ `admin` sửa được. Tên chỉ số không trùng
trong cả dataset.

- **Thêm chỉ số**: **Tên** (người dùng gõ khi hỏi, ví dụ "Doanh thu"), **Mô tả**, rồi chọn **Phép tính** và **Cột**, hoặc
  **Viết bằng SQL**, ví dụ `SUM("Order Item Total") / COUNT(DISTINCT "Order Id")`. **Điều kiện**, **Cột thời gian**,
  **Đơn vị** không bắt buộc. Backend chạy thử công thức trước khi lưu; sai thì lời báo hiện dưới ô sai. Câu hỏi nhắc tên
  chỉ số thì dùng đúng công thức này (xem ở tab SQL).
- **Sửa** lưu một phiên bản mới cùng tên, bản cũ giữ lại; **Xóa** xóa chỉ số cùng mọi phiên bản.
- **Cách gộp của cột số đo**: **Tổng** cho tiền, số lượng; **Trung bình** cho giá trị mô tả một dòng (tuổi, điểm, số
  ngày). Mô hình đề xuất lúc lập hồ sơ; đổi ở đây có hiệu lực từ câu hỏi kế tiếp.

## 8. Báo cáo

Dưới mỗi câu trả lời có nút **Thêm vào báo cáo**. Trang **Báo cáo** trên thanh bên liệt kê báo cáo của bạn (chưa chia sẻ
được cho người khác) và có hai cách tạo:

- Báo cáo trống có tiêu đề, rồi thêm câu trả lời từ trang hỏi.
- **Tạo báo cáo tự động** cho một dataset: code dựng một bộ phân tích (độ đo chính theo thời gian và theo vài cột chia
  nhóm) với đúng quyền xem của người bấm, mô hình viết lời cho từng phần.

Trong một báo cáo: sửa tiêu đề và đoạn mở đầu, đổi thứ tự, bỏ mục. Mỗi mục có câu hỏi, lời trả lời kèm kết quả kiểm số
và kiểm nghĩa (lời là câu hệ thống tính thì một dòng ghi lời của mô hình đã không qua bước nào), biểu đồ, bảng, phân
rã, Cách tính, SQL, bảng và phiên bản dữ liệu, lúc hỏi. Số liệu tính lại mỗi lần mở, trên đúng phiên bản câu hỏi đã
đọc, với quyền xem của người đang mở; quyền hẹp hơn lúc hỏi thì lời đã lưu không hiện.

**Xuất PDF** mở hộp in của trình duyệt (chọn lưu thành PDF): bản in ẩn thanh bên và nút, mỗi mục một trang. **Tải
Excel** tải file có sheet tóm tắt, mỗi mục một sheet và sheet cách tính.

## 9. Mời thành viên

Chưa có thư mời qua email hay tự đăng ký. Thành viên cần có tài khoản trước:

1. Quản trị hệ thống mở **Quản lý tài khoản** ở thanh bên, khung **Tạo tài khoản**: email, tên hiển thị, mật khẩu ban
   đầu (8 đến 128 ký tự). Người đó đổi mật khẩu ở **Tài khoản và mật khẩu** trong menu tài khoản.
2. `admin` của workspace mở **Thành viên**, khung **Thêm thành viên**: email của tài khoản vừa tạo, vai **Admin** hay
   **Thành viên**.

**Thành viên** hỏi, xem dữ liệu và chỉ số, làm báo cáo của mình. **Admin** thêm kết nối và nạp, xóa dữ liệu, sửa chỉ
số, quản lý thành viên, vai dữ liệu, câu mẫu, bài học, đổi tên và xóa workspace. Trang **Thành viên** ghi vai dữ liệu của
từng người; thành viên chỉ thấy vai của mình.

## 10. Vai dữ liệu

Mục **Vai dữ liệu** trên thanh bên (chỉ `admin` thấy): tạo vai, đặt luật lọc dòng bằng cách chọn dataset, bảng, cột
chia nhóm rồi tick giá trị (hay gõ giá trị khác), và tick thành viên mang vai. Ví dụ vai "Châu Âu" với luật `orders.market` là
`Europe`: người mang vai hỏi "Doanh thu theo thị trường" chỉ thấy Europe, kể cả câu trên `order_items` (lọc lan qua khóa
ngoại tới `orders`).

- Các luật trong một vai nối bằng AND, giữa các vai của một người nối bằng OR.
- Lọc áp khi hỏi, xem tổng quan, truy dòng nguồn, mở báo cáo. Admin không bị lọc. Đổi luật có hiệu lực ở request kế tiếp.
- Không gán được vai chưa có luật, không xóa được vai đang gán, hai vai không được trùng đúng bộ luật.

Người mang vai còn bị bớt những gì gửi cho mô hình: [PRIVACY.md](PRIVACY.md).

## 11. Đánh giá, câu mẫu, bài học

Dưới mỗi câu trả lời có hai nút **Tốt** và **Chưa đạt** (kèm lý do, ghi chú không bắt buộc). Admin dùng các phản hồi đó
ở hai mục trên thanh bên, chỉ `admin` thấy:

- **Câu mẫu**: duyệt câu trả lời được đánh giá Tốt làm câu mẫu; câu mẫu đã duyệt vào prompt của các câu hỏi giống nó,
  và xuất được thành file theo khuôn của bộ đánh giá.
- **Bài học**: đọc nhật ký lỗi, viết bài học tay hay từ một dòng nhật ký; bài học bật lên thì vào prompt.

Mục **Khái niệm ngành** chứa khái niệm chung của một ngành (ví dụ giáo dục); đầu màn này admin chọn gói cho từng
dataset: [DOMAINS.md](DOMAINS.md).
