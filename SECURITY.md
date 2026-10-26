# Chính sách bảo mật

## Báo lỗ hổng

Đừng mở issue, Discussion hay PR công khai cho lỗ hổng bảo mật. Báo riêng qua GitHub Private vulnerability reporting:
tab **Security** của repo, chọn **Report a vulnerability**. Chỉ người báo và người duy trì dự án đọc được báo cáo.

Nên ghi: phiên bản hoặc commit, cách chạy (Docker hay build từ nguồn, `LLM_PROVIDER`), các bước tái hiện, điều kẻ tấn
công làm được (đọc dữ liệu của workspace khác, chạy câu SQL guard lẽ ra phải chặn, vượt đăng nhập...). Không đính kèm dữ
liệu thật của tổ chức nào.

Nếu không thấy nút Report a vulnerability, mở một issue chỉ ghi là cần kênh báo riêng, không kèm chi tiết lỗ hổng.

## Phiên bản được hỗ trợ

Bản vá bảo mật làm trên nhánh `main` và trên bản phát hành mới nhất. Bản phát hành cũ hơn không được vá; cập nhật theo
mục Dừng, cập nhật, gỡ của [docs/INSTALL.md](docs/INSTALL.md).

## Sau khi báo

Dự án chưa cam kết thời gian phản hồi cố định. Người duy trì xác nhận đã nhận báo cáo, trao đổi
trong chính báo cáo đó, và công bố lỗ hổng kèm bản vá sau khi bản vá đã phát hành.

## Mô hình an toàn

Dữ liệu nằm ở đâu, gửi gì cho mô hình ngôn ngữ: [docs/PRIVACY.md](docs/PRIVACY.md). Câu SQL nào được chạy, câu nào bị
từ chối, các hạn chế đã biết: [docs/GUARD.md](docs/GUARD.md). Workspace, vai và phân quyền: mục 4b của
[docs/ARCHITECTURE.md](docs/ARCHITECTURE.md). Cấu hình khi đưa lên Internet: mục 3 của [docs/INSTALL.md](docs/INSTALL.md).
