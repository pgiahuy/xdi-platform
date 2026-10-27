# Dữ liệu mẫu

`samples/` dành cho vài file nhỏ (dưới 1 MB) để chạy ngay và để test.
Compose gắn thư mục này vào container backend ở `/samples`, chỉ đọc. `domains/` chứa gói khái niệm ngành
([docs/DOMAINS.md](../docs/DOMAINS.md)): được commit và backend nhúng vào binary lúc biên dịch, thêm một ngành chỉ cần
thêm một file JSON.

| File trong `samples/` | Nguồn | Giấy phép | Dùng cho |
|---|---|---|---|
| `dataco.csv` (1500 dòng lấy ngẫu nhiên) | Mendeley Data, https://doi.org/10.17632/8gx2fvg2k6.5 | CC BY 4.0 | Bộ đánh giá câu hỏi mẫu (`make eval`, #37) và thử nhanh với mock |

Dataset thật không commit vào repo. `download.sh` là chỗ để thêm lệnh tải, hiện chưa tải dataset nào (chỉ in một dòng
báo). Dataset mà các câu hỏi mẫu của `LLM_PROVIDER=mock` viết cho:

| Dataset | Nguồn | Giấy phép | Dùng cho |
|---|---|---|---|
| DataCo Smart Supply Chain, file `DataCoSupplyChainDataset.csv` | Mendeley Data, https://doi.org/10.17632/8gx2fvg2k6.5 | CC BY 4.0 | Câu hỏi mẫu và hồ sơ viết sẵn của mock (`backend/src/engine/llm/mock_answers.json`), bảng logic `dataco` |

Tải tay từ trang Mendeley Data, nạp ở trang **Dữ liệu** với tên dataset `DataCo` để tên bảng là `dataco`: mục 7 của
[docs/INSTALL.md](../docs/INSTALL.md).

Khi thêm dataset: ghi nguồn, giấy phép, và kiểm tra giấy phép cho phép phân phối lại trước khi đưa vào `samples/`.
