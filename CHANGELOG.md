# Changelog

Theo [Keep a Changelog](https://keepachangelog.com/en/1.1.0/). Số phiên bản theo [Semantic Versioning](https://semver.org/lang/vi/).

## [Unreleased]

### Added

- `GEMINI_BACKUP_API_KEY` nhận nhiều khóa cách nhau bằng dấu phẩy, dùng lần lượt khi khóa trước hết hạn mức.
- Database mẫu có câu gợi ý chọn tay; câu gợi ý của mô hình được code kiểm trước khi lưu. Hết hạn mức Gemini theo phút lúc lập hồ sơ thì chờ rồi gọi lại trong cùng lượt.
- Cột khóa chính, khóa ngoại của database nguồn thành vai trò `key` lúc chụp; cột số nguyên là mốc ngày, tuần (như số ngày từ đầu học phần) không còn là số đo để cộng.
- Gemini hết hạn mức với mọi khóa thì gọi lại bằng mô hình dự phòng `GEMINI_FALLBACK_MODEL` (mặc định `gemini-3.1-flash-lite`).
- Kết nối database Postgres là luồng chính ở trang Dữ liệu: chụp mọi bảng tài khoản đọc được (admin bỏ tick bảng không cần), mỗi database là một dataset, khóa ngoại thành quan hệ giữa các bảng; TLS `require` và `verify-full` kèm CA riêng; chụp lại bằng tay hay theo lịch `SOURCE_RESNAPSHOT_HOURS` (#165, #230, #247, #257).
- Bảng quá trần dung lượng (`SOURCE_TABLE_MAX_MB`, `SOURCE_SNAPSHOT_MAX_MB`) chỉ lưu cấu trúc; câu hỏi cần bảng đó được báo đúng tên bảng và cách chụp nó (#247, #278).
- Chụp bảng đọc mô tả `COMMENT ON` của database làm mô tả cột; giao diện ghi mô tả lấy "từ database" hay "do mô hình viết" (#245).
- Database mẫu `sales_demo` (DataCo) và `edu_demo` (OULAD) có khóa ngoại, mô tả và nguyên nhân cài sẵn; chạy trong compose bằng `--profile demo`, thiết lập lần đầu thêm thành hai dataset; xem bảng bằng Adminer hay Flask-Admin (#203, #230, #250, #259).
- Câu hỏi đọc được mọi bảng đã chụp của dataset; database lớn thì hệ thống chọn bảng gửi chi tiết theo câu hỏi, bảng còn lại chỉ gửi tên; hai bảng không có quan hệ thì báo rõ (#213, #249).
- Guard chỉ nhận phép nối đúng cột của một khóa ngoại; câu cộng lặp sau khi nối bị chặn và mô hình sửa lại một lần (#214, #256).
- Vai dữ liệu: admin tạo vai, đặt luật lọc dòng theo bảng, cột, giá trị và gán một hay nhiều vai cho thành viên ở màn Vai dữ liệu; luật lan theo khóa ngoại (#45, #167, #215).
- Câu hỏi nguyên nhân hiểu thêm nhiều dạng: một nhóm so với các nhóm còn lại, bộ lọc khoảng, trung bình, tỷ lệ, tuần, ngày, cùng kỳ năm trước, tìm kỳ hay nhóm bất thường; bước phân loại nhờ mô hình nhận ra câu hỏi nguyên nhân, luật từ khóa chỉ là dự phòng (#197, #244).
- Lời giải thích của câu hỏi nguyên nhân và câu hỏi số liệu do mô hình viết từ số code đã tính, qua kiểm số và kiểm nghĩa; trượt thì viết lại một lần, vẫn trượt thì dùng câu của hệ thống; chỗ chưa rõ hiện trong khối nền vàng (#201).
- Mọi câu trả lời có phần giải thích kết quả nói lên điều gì (so kỳ trước, tỷ trọng, mức tập trung); phân rã xét cả cột của bảng liên quan (#227).
- Tab Phân rã có cột Chênh lệch, % tác động, % thay đổi, nhãn nhóm khác thường, nhóm mới hay không còn; bảng dàn đều xếp sau (#186).
- Dòng nguồn truy được cho câu nối bảng: dòng của bảng mà độ đo cộng, kèm cột của bảng nối và tổng của độ đo để so với ô đã bấm.
- Báo cáo: thêm câu trả lời vào báo cáo hay tạo báo cáo tự động cho một dataset, xuất PDF và Excel; số liệu tính lại khi mở theo quyền xem của người đọc.
- Trang Tổng quan dữ liệu, màn Chỉ số (công thức chạy thử trước khi lưu, cách gộp tổng hay trung bình của cột số đo), màn Bài học, màn Khái niệm ngành kèm gói giáo dục; nút Tốt và Chưa đạt dưới mỗi câu trả lời, câu Tốt admin duyệt ở màn Câu mẫu thành ví dụ cho prompt (#185, #212, #228, #229).
- Câu trả lời ghi khoảng ngày của dữ liệu, số lớn có dạng rút gọn kèm số đủ, cột tự chọn khi một từ khớp nhiều cột; mở lại cuộc trò chuyện cũ thấy lại lời trả lời (#201).
- Tab Các bước ghi từng lượt gọi mô hình (thời gian, token); lệnh `xdi-backend llm-stats`; `GEMINI_BACKUP_API_KEY` dùng khi khóa chính hết hạn mức (#184).
- Bộ đánh giá: `make eval` thêm câu trên database mẫu, `make eval-judge`, `make eval-classify`, bộ 57 câu OULAD với báo cáo theo loại câu ([eval/README.md](eval/README.md)) (#201, #202, #230).
- Bộ đánh giá có 100 câu hỏi (thêm 12 câu trên `sales_demo`, `edu_demo`, 11 câu nối bảng), 50 câu phân loại, 80 lời giải thích có nhãn, nửa đúng nửa sai.

- Admin chọn gói ngành cho từng dataset ở màn Khái niệm ngành; dataset mẫu có sẵn gói, khái niệm của gói vào prompt của câu hỏi số liệu và câu hỏi nguyên nhân.

### Changed

- Chữ trên giao diện theo một bộ từ: "câu hỏi nguyên nhân", "câu hỏi số liệu", "Cách tính", "Minh chứng", "kiểm số" và "kiểm nghĩa".
- Dataset là một database hay một file, bảng nằm bên trong; API `/catalog` hỏi và chụp lại theo dataset, route cũ theo bảng còn nhưng đánh dấu deprecated (#257).
- Chụp bảng mặc định để DuckDB đọc thẳng Postgres bằng extension đóng gói sẵn; `SOURCE_SNAPSHOT_MODE=copy` là đường dự phòng (#246).
- Không còn trần số dòng mặc định khi chụp bảng; `.env` cũ có `SOURCE_MAX_ROWS=1000000` thì xóa dòng đó (#278).
- Một từ khớp nhiều cột không còn bị hỏi lại: hệ thống trả lời theo cột khớp nhất, các cột khác đổi được trên thẻ; câu hỏi lại một ý có mỗi phương án là một nút (#201).
- Giao diện: vào workspace là mở trang Cuộc trò chuyện, nút Lịch sử trò chuyện ở đầu thanh bên, các trang dùng hết bề ngang màn hình.
- Xóa một bảng thì xóa luôn lịch sử câu hỏi trên bảng đó (#187).

### Fixed

- Câu hỏi nguyên nhân trên database: bắt đầu từ bảng giữ độ đo và dùng được cột của bảng nối cho kỳ, nhóm, bộ lọc; so đúng hai nhóm câu hỏi nêu, một nhóm thì so với trung bình các nhóm còn lại; lời nêu nhóm chính của từng chiều, viết tỷ lệ bằng phần trăm.
- Docker Compose truyền `GEMINI_BACKUP_API_KEY` tới backend; trước đây khóa dự phòng trong `.env` không có tác dụng.
- Điều kiện lọc có câu con hay "hoặc" đọc thành lời thay cho nguyên câu SQL; câu nối bảng gọi số đo bằng mô tả tiếng Việt của bảng nối; kết quả toàn ô trống nói rõ không có dữ liệu khớp (#201).
- Nạp và chụp bảng lớn tốn ít RAM hơn: DuckDB có trần `DUCKDB_MEMORY_LIMIT_MB` (mặc định 1024), chạm trần thì ghi phần dư ra đĩa; bảng 5 triệu dòng 60 cột từ 3,6 GiB xuống khoảng 1,1 GiB.
- Vòng sửa SQL ẩn cả giá trị của cột nhạy cảm mà lời báo lỗi của DuckDB đặt trong nháy kép (đổi sang ngày, giờ, số thập phân, `strptime`); trước đây chỉ ẩn giá trị trong nháy đơn.

## [0.2.0] - 2026-10-27

### Added

- `sh data/download.sh oulad` tải bộ dữ liệu giáo dục OULAD (Open University Learning Analytics, CC BY 4.0) để thử hỏi về kết quả học tập.
- Lời giải thích câu "tại sao" dễ đọc hơn: câu đầu nói so gì với gì và chênh bao nhiêu, câu sau nói nhóm nào tạo phần lớn chênh lệch bằng lời thường; so hai nhóm thì nói "cao hơn", "thấp hơn" thay cho "tăng", "giảm".
- Câu trả lời "tại sao" gọn hơn: hai kỳ và chênh lệch thành một hàng, bỏ nhãn tăng hay giảm ở từng thanh, tab Phân rã không nhắc lại số và lời giải thích đã có trong câu trả lời.
- Trang hỏi rõ bố cục hơn: ô chọn dataset ở đầu khung chat và dính khi cuộn, khung chat có nền riêng, danh sách cuộc trò chuyện là khung riêng với phần đầu đứng yên, nút thu gọn thanh bên ở góc trên phải, lúc đang trả lời chỉ hiện ba chấm, thanh cuộn mảnh.
- Câu gợi ý của mỗi dataset có thêm câu khó: hai câu "vì sao" cần giải thích chênh lệch giữa hai kỳ hay hai nhóm, một câu tỷ lệ, trung bình hoặc top N.
- Dataset mẫu (#25): thiết lập lần đầu nạp sẵn DataCo (1500 dòng) kèm chỉ số mẫu vào workspace đầu tiên, tắt bằng `SEED_SAMPLE=false`; `data/download.sh` tải bản đầy đủ DataCo và Online Retail II.
- Bộ đánh giá câu hỏi mẫu (#37): `make eval` chạy 14 câu mẫu trên dữ liệu DataCo thu nhỏ (`data/samples/dataco.csv`, CC BY 4.0), so kết quả với SQL đúng viết tay và báo accuracy; CI đỏ khi accuracy giảm dưới `eval/baseline.json`. `make eval` luôn chạy với mock; chạy với mô hình thật thì gọi thẳng `xdi-backend eval` ([docs/RELEASING.md](docs/RELEASING.md)).
- Xóa dataset (#160): admin bấm **Xóa** ở trang Dữ liệu để xóa hẳn một dataset cùng mọi phiên bản, hồ sơ, cuộc trò chuyện và file; lịch sử câu hỏi vẫn giữ.
- Câu "tại sao" nhận thêm cách hỏi ("sao lại", "vì đâu") và cảnh báo khi câu hỏi nói giảm mà số thật tăng hay ngược lại (#164).
- Câu "tại sao" không nêu kỳ ("Tại sao doanh thu giảm") giải thích kỳ gần nhất có dữ liệu, đổi được sang kỳ khác trên thẻ "Đã hiểu là" (#164).
- Thẻ "Đã hiểu là" của câu tiếp nối ghi "từ câu trước" cạnh độ đo, bộ lọc, nhóm hay kỳ lấy từ lượt trước (#164).
- Câu hỏi thường có thêm một đoạn trả lời ngắn bằng lời trên thẻ "Đã hiểu là", mọi số trong đó lấy từ kết quả; tắt bằng `ANSWER_TEXT=false`.
- Câu hỏi cần thông tin dataset không có (mức hài lòng, dự báo) được báo rõ kèm các cột gần nhất có thể hỏi, thay vì chạy một câu SQL lạc đề (#164).
- Trang hỏi thành khung trò chuyện, kèm danh sách cuộc trò chuyện cũ để mở lại và hỏi tiếp (#164).
- Trang hỏi gọn hơn: phần "Đã hiểu là" và bảng thu gọn được, pane đổi tên thành Minh chứng và thu về mép phải, thanh bên và danh sách cuộc trò chuyện ẩn được, lời trả lời giọng báo cáo (#164).
- Nút **Tạo workspace** ở cuối ô chọn workspace và ở màn của người chưa thuộc workspace nào.
- Câu "tại sao" cho chỉ số tỷ lệ và trung bình (ví dụ tỷ lệ giao trễ): chênh lệch tách thành phần do cơ cấu và phần do hiệu suất của từng nhóm.
- Hỏi theo cuộc trò chuyện (#164): câu sau hiểu theo các câu trước trong cùng cuộc, mỗi lượt giữ câu trả lời và bằng chứng riêng, có nút "Cuộc trò chuyện mới".

### Changed

- Hỏi dữ liệu: prompt của mô hình có thêm luật của guard (bảng, hàm, giới hạn kích thước được phép) để mô hình viết câu qua guard ngay lần đầu; guard chặn câu vì bất kỳ lý do nào thì mô hình được sửa một lần, không còn dừng ngay ở lời "Câu SQL bị từ chối"; câu sửa vẫn bị chặn thì người hỏi nhận lời gợi ý hỏi lại theo cách khác, lý do kỹ thuật nằm trong pane bằng chứng.
- Màn đăng nhập và thiết lập lần đầu (#159): form nằm cạnh panel giới thiệu sản phẩm, phiên bản máy chủ ở dưới form.
- Khung ứng dụng (#161): thanh bên có logo và icon cho từng mục, tài khoản chuyển xuống chân thanh bên, nội dung căn giữa trên màn rộng.
- Ở trang quản trị hệ thống, thanh bên vẫn giữ các mục của workspace đang làm việc thay vì ẩn đi.
- Giao diện mới "Mực và hổ phách": thanh bên nền mực, màu nhấn hổ phách cho nút chính và số chênh lệch, font Be Vietnam Pro đóng gói sẵn, chạy không cần mạng.
- Bảng kết quả: bấm vào bất kỳ chỗ nào trong ô số (và trên cả dòng khi bảng có một cột số đo) để xem dòng nguồn, không cần nhắm đúng con số.

## [0.1.0] - 2026-10-26

### Added

- Nền tảng: backend Rust (Axum, DuckDB nhúng, Postgres), giao diện React và Vite do backend phục vụ cùng cổng, một image Docker chứa cả hai; hướng dẫn cài đặt, đưa lên Internet, sao lưu và khôi phục ở `docs/INSTALL.md`.
- Tài khoản và workspace: mã thiết lập in ra log lần chạy đầu, vai admin và member trong từng workspace, trang quản trị cho quản trị hệ thống.
- Nạp dữ liệu: CSV (kể cả windows-1252), Parquet, Excel nhiều sheet; mỗi lần nạp một phiên bản, nạp trùng bỏ qua; hồ sơ cột tự động (kiểu, vai trò, đơn vị, cột nhạy cảm) và câu hỏi gợi ý do mô hình đề xuất.
- Hỏi dữ liệu bằng tiếng Việt: mô hình ngôn ngữ (Gemini, Ollama, hoặc mock chạy offline) sinh SQL, guard chỉ cho truy vấn và tiêm phân quyền, tự sửa một vòng khi SQL lỗi, hỏi lại khi câu mơ hồ.
- Giải thích từng con số: thẻ "Đã hiểu là" với giả định đổi được, pane bằng chứng (câu SQL trước và sau guard, các bước backend), câu "tại sao" phân rã theo từng chiều, bấm ô số hoặc dòng phân rã để xem dòng nguồn, cảnh báo khi kết quả đáng ngờ.
- Biểu đồ chọn theo chuẩn IBCS; số và lời giải thích định dạng kiểu Việt Nam.
- Đặc tả OpenAPI sinh từ code (`/api/openapi.json`, Swagger UI ở `/api/docs`); quy trình phát hành gồm binary Linux, tarball mã nguồn và image trên GitHub Container Registry.
