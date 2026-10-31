# Changelog

Theo [Keep a Changelog](https://keepachangelog.com/en/1.1.0/). Số phiên bản theo [Semantic Versioning](https://semver.org/lang/vi/).

## [Unreleased]

### Added

- Trang "Tổng quan dữ liệu": số dòng, khoảng thời gian, các cột theo nhóm kèm nhãn tiếng Việt, biểu đồ dựng sẵn và chỉ số; bấm một cột hay biểu đồ để điền câu hỏi.
- Màn **Chỉ số** của mỗi dataset (nút Chỉ số ở trang Dữ liệu): admin thêm, sửa, xóa chỉ số bằng cách chọn phép tính và cột hoặc viết SQL; công thức được chạy thử trước khi lưu, sai thì báo ngay dưới ô sai; câu hỏi sau dùng đúng công thức đó (#185).
- Cột số đo có cách gộp mặc định, tổng hay trung bình: mô hình đề xuất lúc nạp, admin đổi ở màn Chỉ số; câu hỏi trên cột trung bình dùng AVG, câu "vì sao" so trung bình mỗi dòng (#185).
- Postgres mẫu nhiều bảng có khóa chính, khóa ngoại (DataCo và OULAD) để thử kết nối cơ sở dữ liệu công ty: `data/postgres-demo/`.
- Nguồn Postgres (#165): admin kết nối một database Postgres ở trang Dữ liệu, chụp một bảng thành dataset và chụp lại thành phiên bản mới.
- Chụp bảng Postgres kèm các bảng liên quan theo khóa ngoại, theo cả hai chiều và tới hai bước (tối đa 10 bảng): mỗi bảng một dataset, quan hệ giữa chúng được ghi lại; danh sách dataset ghi các dataset liên quan.
- Hỏi trên bảng chụp từ database thì mô hình nối được các bảng liên quan bằng `JOIN`; khung Minh chứng ghi các bảng đã đọc (#213).
- Câu SQL cộng hay lấy trung bình cột của bảng phía một sau khi nối bảng phía nhiều không chạy mà được sửa một lần, tránh số bị cộng lặp (#214).
- Bộ lọc dòng theo vai dữ liệu lan theo khóa ngoại: lọc khách hàng theo vùng thì đơn hàng và dòng hàng của khách vùng khác cũng bị ẩn, cả khi hỏi riêng bảng đơn hàng (#215).

### Changed

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
