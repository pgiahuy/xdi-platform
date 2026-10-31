# Changelog

Theo [Keep a Changelog](https://keepachangelog.com/en/1.1.0/). Số phiên bản theo [Semantic Versioning](https://semver.org/lang/vi/).

## [Unreleased]

### Added

- Chụp bảng Postgres đọc luôn mô tả bảng và cột ghi trong database (`COMMENT ON`) làm mô tả trong hồ sơ cột; mô hình không viết lại các mô tả này, giao diện ghi nguồn "từ database" hay "do mô hình viết" (#245).
- Chụp bảng Postgres nhanh hơn: DuckDB đọc thẳng bảng bằng extension `postgres` đóng gói sẵn (không tải lúc chạy) và ghi Parquet một bước; bảng 1,8 triệu dòng của database mẫu chụp trong khoảng 0,9 giây thay vì 16 giây. `SOURCE_SNAPSHOT_MODE=copy` giữ cách cũ, và là đường dự phòng khi extension không nạp được (#246).
- Insight của câu hỏi tìm nguyên nhân do mô hình viết toàn bộ (câu mở đầu và mục "Nguyên nhân chính") từ số và cờ code đã tính, prompt không còn câu do code viết hay ví dụ theo bộ dữ liệu; khi insight qua kiểm thì không hiện mục "Căn cứ" nữa (#201).
- Câu hỏi tìm nguyên nhân so hai nhóm (như hai thị trường) cũng do mô hình viết insight và qua hai cổng kiểm, thay cho câu khuôn (#201).
- Câu hỏi về một số đo trong một kỳ, như "doanh thu tháng 6 năm 2017 như thế nào", trả tổng của kỳ đó thay vì hỏi lại có chia theo ngày không (#201).
- Câu khuôn "Lý do chính" chỉ nói khi có số lượng mang nghĩa kinh doanh (như số đơn) và so phần do số lượng với phần do giá trị trung bình mỗi đơn; bỏ ý "số dòng dữ liệu hai bên khác nhau", vốn gọi sai lý do khi cả chênh lệch nhỏ và không có nghĩa với người đọc; không còn nêu phần của chênh lệch quá 100% (#201).
- Câu hỏi tìm nguyên nhân gọi tên bằng tên tiếng Việt của hồ sơ cột: "Doanh thu" thay cho "Tổng Sales", "Số đơn hàng" và "Doanh thu trên mỗi đơn hàng" thay cho "Số Order Id" và "... trên số Order Id"; insight được viết số tròn kèm chữ "khoảng" (tối đa ba chữ số có nghĩa), số khai để kiểm vẫn phải chính xác (#201).
- Hồ sơ cột: mọi cột mã số nguyên (mã đơn, mã khách, mã sản phẩm, mã danh mục) được đề xuất vai `key` thay vì số đo; câu "tại sao" lấy số đơn từ cột mã lặp lại có nhiều giá trị nhất, nên bộ có nhiều cột mã vẫn tách được phần do số đơn và phần do giá trị mỗi đơn (#201).
- Lời giải thích câu hỏi tìm nguyên nhân: mô hình khai từng số và nhóm nó dùng, hệ thống so mỗi khai báo với đúng ô của bảng phân rã, nên bắt được khi mô hình khai một số của bảng cho nhầm nhóm; rồi một lần gọi mô hình riêng so nghĩa của lời với các khai báo và tên hai kỳ; sai hay có chỗ đọc được nhiều nghĩa thì mô hình viết lại một lần kèm các lỗi đó, vẫn sai thì dùng câu do hệ thống viết; chỗ chưa rõ mà không chắc sai thì giữ lời, các chỗ đó hiện ngay dưới insight; tab Các bước ghi số khẳng định đã đối soát (#201).
- `GEMINI_BACKUP_API_KEY`: khóa Gemini dự phòng; khóa chính hết hạn mức thì backend tự gọi lại bằng khóa dự phòng thay vì báo lỗi.
- `make eval-judge`: bộ đo mô hình chấm trên 67 ca có nhãn (32 lời đúng, 35 lời sai): 55 ca lời giải thích câu tìm nguyên nhân (7 ca lấy từ lỗi thật khi chạy Gemini; 16 ca có số lời tự tính từ các số đã khai) và 12 ca lời phần giải thích câu thường; in số ca đúng trên tổng, số lời sai bị bỏ lọt hay chỉ bị cảnh báo, số lời đúng bị chặn nhầm hay bị cảnh báo thừa (#201).
- Lời phần giải thích của câu thường (cũng là đoạn trả lời ngắn) qua hai cổng như câu tìm nguyên nhân: sau bước kiểm số, chiều và từ chỉ nguyên nhân, một lần gọi mô hình riêng so nghĩa của lời với các câu code đã tính; trượt hay có chỗ chưa rõ thì viết lại một lần. Mô hình chấm chỉ nhận lời và các câu đó, không nhận dòng kết quả nào. Có chỗ chưa rõ thì hiện khối cảnh báo nền vàng dưới đoạn trả lời; tab Các bước có cờ match, warning, mismatch. API thêm `insight.summary_verdict`, `insight.summary_warnings` (#201).
- Lời của mô hình nói mỗi chênh lệch một lần, phần trăm trong ngoặc, không còn câu như "cao hơn 11.205,79 hoặc cao hơn 16,7%" (#201).
- Mô hình chấm thấy chỗ chưa rõ (warning) thì lời cũng được viết lại một lần như khi sai, kèm các chỗ chưa rõ đó; lần viết lại không khớp hẳn thì giữ lời cũ kèm cảnh báo. Khối cảnh báo dưới insight có nền vàng (#201).
- Câu SQL mô hình viết đặt tên tiếng Việt cho mọi cột tính ra theo đúng thứ đang tính (như tổng lợi nhuận), không dùng lại tên cột gốc: tổng của cột "Benefit per order" từng mang tên cột đó và bị đọc thành lợi nhuận trên mỗi đơn (#201).
- Câu do hệ thống viết cho câu hỏi tìm nguyên nhân (khi lời của mô hình không qua kiểm hay mô hình lỗi) dễ đọc hơn: gọi cột bằng nghĩa tiếng Việt ("thị trường" thay cho "Market (thị trường)"), "Không nhóm nào khác thường" thay cho "Không có nơi nào đặc biệt" (#201).
- Mô hình chấm nhận số không có sẵn trong bảng khi một phép tính đơn giản trên các số đã khai ra đúng số đó và lời nói đúng phép tính ấy; lời có câu ra lệnh cho mô hình chấm bị coi là sai (#201).
- Tab Các bước có cờ màu: loại câu ở bước phân loại (Quantitative, Causal) và kết luận của mô hình chấm ở bước viết lời (match, warning, mismatch), thay cho nhãn "có mô hình" ở hai bước này; bước phân loại đạt thì không hiện câu lý do của mô hình nữa; bước đạt chỉ còn dấu tick xanh lá, bỏ chữ "đạt". API thêm `question_type` và `explanation.summary_verdict` (#201).
- Trang "Tổng quan dữ liệu": số dòng, khoảng thời gian, các cột theo nhóm kèm nhãn tiếng Việt, biểu đồ dựng sẵn và chỉ số; bấm một cột hay biểu đồ để điền câu hỏi.
- Màn **Chỉ số** của mỗi dataset (nút Chỉ số ở trang Dữ liệu): admin thêm, sửa, xóa chỉ số bằng cách chọn phép tính và cột hoặc viết SQL; công thức được chạy thử trước khi lưu, sai thì báo ngay dưới ô sai; câu hỏi sau dùng đúng công thức đó (#185).
- Cột số đo có cách gộp mặc định, tổng hay trung bình: mô hình đề xuất lúc nạp, admin đổi ở màn Chỉ số; câu hỏi trên cột trung bình dùng AVG, câu "vì sao" so trung bình mỗi dòng (#185).
- Postgres mẫu nhiều bảng có khóa chính, khóa ngoại (DataCo và OULAD) để thử kết nối cơ sở dữ liệu công ty: `data/postgres-demo/`.
- Postgres mẫu có thêm nguyên nhân cài sẵn ở dữ liệu học tập (bỏ học kỳ 2014J), sơ đồ ER và tài liệu bảng sinh bằng tbls, Adminer để xem bảng khi trình diễn (#203).
- Nguồn Postgres (#165): admin kết nối một database Postgres ở trang Dữ liệu, chụp một bảng thành dataset và chụp lại thành phiên bản mới.
- Chụp bảng Postgres kèm các bảng liên quan theo khóa ngoại, theo cả hai chiều và tới hai bước (tối đa 10 bảng): mỗi bảng một dataset, quan hệ giữa chúng được ghi lại; danh sách dataset ghi các dataset liên quan.
- Hỏi trên bảng chụp từ database thì mô hình nối được các bảng liên quan bằng `JOIN`; khung Minh chứng ghi các bảng đã đọc (#213).
- Câu SQL cộng hay lấy trung bình cột của bảng phía một sau khi nối bảng phía nhiều không chạy mà được sửa một lần, tránh số bị cộng lặp (#214).
- Bộ lọc dòng theo vai dữ liệu lan theo khóa ngoại: lọc khách hàng theo vùng thì đơn hàng và dòng hàng của khách vùng khác cũng bị ẩn, cả khi hỏi riêng bảng đơn hàng (#215).
- Mọi câu trả lời có phần giải thích kết quả nói lên điều gì, chọn theo hình dạng kết quả: so với kỳ trước, kỳ đổi mạnh nhất kèm phân rã, tỷ trọng và mức tập trung của các nhóm, hay chỉ số dòng; chênh lệch nhỏ hay ít dòng thì ghi không có thay đổi đáng kể. Trên bảng chụp từ database, phân rã xét cả cột của bảng liên quan (#227).
- Màn **Bài học** cho admin: nhật ký lỗi ghi tự động (guard từ chối, SQL lỗi, cộng lặp bị chặn, form sai, lời bị bộ kiểm chặn, câu bị đánh giá Chưa đạt); admin viết bài học tay hay từ một dòng nhật ký, duyệt và bật; tối đa 3 bài liên quan vào prompt; `make eval-lessons` chạy bộ đánh giá không và có bài học (#228).
- Hai nút **Tốt** và **Chưa đạt** dưới mỗi câu trả lời (Chưa đạt kèm lý do và ghi chú); màn **Câu mẫu** cho admin duyệt câu Tốt thành ví dụ trong prompt của câu hỏi sau có từ chung, và xuất câu đã duyệt thành ca ứng viên cho bộ đánh giá (#212).
- Database mẫu nhiều bảng (`data/postgres-demo`) chạy được ngay trong compose gốc bằng `--profile demo`, chung mạng với backend: thiết lập lần đầu kết nối và chụp 12 bảng kèm quan hệ, có câu mẫu của `mock` và bộ câu của `make eval` cho các bảng đó; không có database mẫu thì nạp DataCo từ file như trước (#230).
- `SOURCE_SECRET_KEY` để trống thì backend tự sinh khóa lần đầu và lưu ở `DATA_DIR/source_secret.key`; nguồn database không còn tắt khi chưa đặt khóa (#230).
- TLS tới nguồn Postgres: `sslmode` `require` và `verify-full`, kèm ô chứng chỉ CA cho CA riêng của tổ chức (#230).
- Màn **Khái niệm ngành** (thanh bên): gói ngành lưu riêng cho từng workspace, admin thêm, sửa, tắt khái niệm, câu hỏi sau dùng bản vừa sửa; thêm gói giáo dục cho dữ liệu OULAD (#229).
- Tab **Các bước** của khung Minh chứng ghi từng lượt gọi mô hình (thời gian, token); lệnh `xdi-backend llm-stats` tổng hợp theo loại câu; lập hồ sơ cột sau khi nạp chạy ba lượt gọi song song và hai bảng cùng lúc (`SEMANTIC_JOBS`); số đo ở `docs/PERFORMANCE.md` (#184).

### Changed

- Trang Cuộc trò chuyện: khung trò chuyện cao hết màn hình, ô hỏi luôn nằm sát đáy dù cuộc trò chuyện chưa có câu nào, mới có một câu hay đã dài; trước đây ô hỏi nổi giữa trang rồi tụt xuống khi có thêm câu. Trên điện thoại lề trong nhỏ hơn để ô hỏi rộng hơn.
- Các trang bung hết bề ngang màn hình, không còn bị giới hạn một cột hẹp chừa trống bên phải: Tổng quan dữ liệu hai biểu đồ một hàng từ cỡ máy tính bảng, ba biểu đồ ở màn rất rộng; Dữ liệu, Thành viên, Quản lý tài khoản chia hai cột ở màn rộng; Chỉ số, Câu mẫu, Bài học, Khái niệm ngành, Mọi workspace dùng hết bề ngang. Ngày ở trang Tổng quan ghi dạng 01/01/2015 thay cho "1 thg 1, 2015".
- Trang Dữ liệu: kết nối database là luồng chính, nạp file CSV, Parquet, Excel chuyển xuống một nút phụ (#230).
- Prompt phân loại và prompt viết SQL có metadata đủ (kiểu, vai trò, đơn vị, cách gộp, mô tả, bí danh) của mọi cột của mọi bảng câu hỏi đọc, không bỏ cột khi dài; lượt phân loại nêu thêm các cột câu hỏi cần (#230).
- Danh sách giá trị ở lượt viết SQL: tổng tới 1000 giá trị thì gửi đủ mọi cột phân loại, vượt thì chỉ cột liên quan (kể cả cột lượt phân loại nêu), tới 200 giá trị mỗi cột (#230).
- Câu SQL lọc theo giá trị cột không có mà danh sách của cột chưa được gửi đủ thì hệ thống viết lại SQL một lần kèm đủ giá trị của cột đó, ghi trong tab Các bước (#230).
- Đoạn trả lời ngắn của câu hỏi thường không còn gửi dòng kết quả nào cho mô hình: nó dùng lời của phần giải thích (viết từ số đã tính), danh sách dòng chỉ ghi số dòng và điều kiện lọc; mỗi câu bớt một lần gọi mô hình.
- Một từ khớp nhiều cột (như "khu vực" là Market hay Order Region) không còn bị hỏi lại trước khi trả lời: hệ thống trả lời ngay bằng cột khớp nhất, các cột còn lại hiện trong thẻ "Hệ thống đã hiểu là" để đổi. Cột người hỏi chọn khi mô hình hỏi lại luôn được dùng, kể cả khi mô hình hỏi bằng chữ khác chữ trong câu, không còn báo "không áp dụng được" sau khi đã chọn (#201).
- Câu hỏi không có số nào để trả lời chỉ hiện câu trả lời và thẻ "Hệ thống đã hiểu là", không còn hộp cảnh báo kỹ thuật, bảng trống hay dòng "câu viết theo khuôn".
- Thanh bên bỏ mục "Hỏi dữ liệu": vào workspace là mở trang **Cuộc trò chuyện**, chưa hỏi gì thì có một câu mời hỏi ngắn; nút **Lịch sử trò chuyện** đứng đầu thanh bên. Trang Cuộc trò chuyện bỏ nút "Tổng quan dữ liệu" cạnh ô chọn dataset, tổng quan chỉ còn ở trang riêng; bỏ công tắc "Dữ liệu mẫu" của chế độ dev.
- Danh sách cuộc trò chuyện rời trang hỏi, sang nút **Lịch sử trò chuyện** trên thanh bên: một khung nổi đè lên trang, liệt kê cuộc trò chuyện trên mọi dataset (mỗi cuộc ghi ngày giờ và dataset của nó), hiện tối đa 8 cuộc một lúc (nhiều hơn thì cuộn), có nút dấu cộng mở cuộc mới, đóng khi chọn, bấm ra ngoài hoặc nhấn Esc. "Tổng quan dữ liệu" thành mục riêng trên thanh bên, chọn dataset ở đầu trang; nút Tổng quan ở trang Dữ liệu mở trang này thay cho khung dưới danh sách.
- Thẻ "Đã hiểu là" đổi tên thành "Hệ thống đã hiểu là", ở câu trả lời, lượt hỏi đã lưu và màn Chỉ số.
- Dòng "Đo" của thẻ "Đã hiểu là" ghi cách tính bằng lời khi chỉ số không có mô tả, ví dụ "tổng cột Order Item Total" (#185).
- Mô hình nhận ra câu hỏi tìm nguyên nhân thay cho so từ khóa, kể cả cách hỏi như "Doanh thu giảm do gì?"; câu đếm theo cột "lý do" vẫn là câu thường. Mô hình lỗi thì dùng luật từ khóa như trước; tab Các bước có thêm bước Phân loại câu hỏi.
- Câu hỏi tìm nguyên nhân có lời tóm tắt do code viết: câu trả lời chính rồi lý do chính (số dòng, lượng và giá, cơ cấu), nhóm khác thường hay "không có nơi nào đặc biệt", và cột lời tóm tắt bỏ qua vì chỉ là tên khác của hai nhóm đang so (như hình thức thanh toán khi so trạng thái đơn) hay trùng cột khác; tên cột kèm nghĩa tiếng Việt.
- Thẻ trả lời câu hỏi tìm nguyên nhân hiện lời tóm tắt (câu chính in đậm và các ý ngắn), số dòng của hai bên, phần trăm thay đổi cạnh chênh lệch, và tên cột kèm nghĩa tiếng Việt ở tiêu đề biểu đồ chênh lệch.
- Tab Phân rã của câu "tại sao" dễ đọc hơn với người dùng nghiệp vụ: thêm cột % Tác động (phần của chênh lệch do nhóm tạo ra) và % Thay đổi (bên so sánh so với bên gốc trong nhóm), in đậm nhóm khác thường so với mức chung, dòng Tổng cuối mỗi bảng và một thẻ xanh dương giải thích các cột tính thêm; số giảm màu đỏ, tăng màu xanh lá ở bảng phân rã, thanh đóng góp và số chênh lệch.
- Tab Phân rã không còn cắt ở 8 bảng: các cột chia nhóm không nhạy cảm có từ 2 tới 50 giá trị (tối đa 30 cột) đều được tính và hiện, xếp theo mức giải thích chênh lệch, bảng giải thích rõ nhất trước, thay cho 8 cột đứng đầu file; bỏ bảng chia y hệt bảng khác (Department Id trùng Department Name, giữ bảng có tên), bảng chỉ một nhóm, và khi so hai nhóm thì bỏ bảng mà mỗi nhóm chỉ có ở một bên; cột bị bỏ vì trùng hay chỉ có ở một bên vẫn được nhắc trong ý "Bỏ qua" của lời tóm tắt. Trên DataCo, câu so phân khúc Consumer với Home Office giờ có bảng theo Market, Order Status, Shipping Mode.
- Tab SQL của pane Minh chứng gọn hơn: bỏ ba câu chú thích dưới bản sau guard (câu giải thích vạch đánh dấu, "Thêm LIMIT 100000", lời nhắc về phân quyền dữ liệu). Dòng guard thêm vẫn có vạch đánh dấu; chi tiết bước guard vẫn ở tab Các bước.
- Câu hỏi tìm nguyên nhân: lời giải thích nói "giảm chủ yếu do" thay cho "nhờ", cột kỳ của bảng kết quả và biểu đồ tên "Tháng", "Quý", "Năm".
- Bảng kết quả: cột thời gian mà mọi ô đều là nửa đêm chỉ hiện ngày, không kèm " 00:00:00".
- Xóa dataset thì xóa luôn lịch sử câu hỏi của nó, kể cả các lượt trò chuyện; khi cập nhật, lịch sử còn sót từ các lần xóa dataset trước bị xóa (#187).

### Fixed

- Câu hỏi ra kết quả toàn ô trống (ví dụ hỏi doanh thu của một năm không có dữ liệu): câu trả lời không còn cụt kiểu "Tổng Sales là ." mà nói rõ không có dữ liệu khớp; cảnh báo kèm khoảng thời gian dữ liệu có khi câu lọc theo thời gian; lời trả lời của mô hình được nhắc lại số có trong câu hỏi (năm, top N) mà không bị bước kiểm số loại.

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
